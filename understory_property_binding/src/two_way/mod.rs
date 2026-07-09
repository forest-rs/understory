// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Internal two-way binding policy over the one-way binding core.
//!
//! This module owns reverse write-back policy for [`BindingGraph`]; it
//! explicitly does not own the host-facing lifecycle API. Ecosystem callers
//! construct [`BindingGraph`] instead of constructing this sidecar
//! directly.
//!
//! A two-way binding keeps a source endpoint (typically model state) and a
//! target endpoint (typically a control part) in sync in both directions:
//! source changes propagate forward to the target exactly like a one-way
//! binding, and control-initiated target writes (a slider drag, text input)
//! propagate back to the source without destroying the binding and without
//! echoing forever between the two endpoints.
//!
//! ## Model
//!
//! The internal sidecar does not duplicate the one-way machinery. Each two-way
//! binding registers its forward direction (source to target) as an ordinary
//! binding in the host's [`BindingSet`], so forward propagation keeps the
//! existing dirty selection, dependency ordering, and drain behavior. The
//! reverse direction (target back to source) is stored here as policy: it is
//! never registered as a binding, so it is not an edge in the binding
//! dependency graph.
//!
//! That split is what makes the intentional back-edge legal. The cycle check
//! in [`BindingSet`] would reject source-to-target plus target-to-source as a
//! cycle if both were registered bindings. Because only the forward direction
//! is a graph edge, a two-way pair registers cleanly, while a *real* cycle
//! that closes a loop through the forward direction and other bindings (for
//! example two-way `A <-> B`, one-way `B -> C`, one-way `C -> A`) is still
//! rejected at registration time.
//!
//! ## The host write contract
//!
//! **Control-initiated writes must be routed through
//! [`BindingGraph::report_endpoint_write`], not silently applied with a
//! raw local-property write.** The host still owns the property store: it first
//! writes the new value into its own storage (whichever layer it chooses),
//! then reports the endpoint. Reporting does two things: it marks ordinary
//! bindings that read the written endpoint dirty, and it schedules reverse
//! propagation if the endpoint is the target of a two-way binding. The next
//! [`BindingGraph::drain`] performs the write-back.
//!
//! This is the analogue of WPF's rule that `SetValue` on a property with a
//! two-way binding routes the value through the binding instead of replacing
//! it. `understory_property`'s local slots are layered
//! (`LocalValueSource::Local` / `TemplateBinding` / `TemplateDefault`), and a
//! control-initiated write usually lands in the same layer the binding writes
//! (chosen per binding via the `C` write context). If the host wrote that
//! layer with a raw `set_local` and never reported it, the binding would still
//! exist but the source would silently fall out of sync; if the host instead
//! tore the binding down on user input, it would reproduce the WPF bug this
//! module exists to avoid. Route the write through
//! [`BindingGraph::report_endpoint_write`] and neither failure mode can
//! happen. The reverse write into the *source* endpoint is delivered through
//! [`BindingHost::set_erased`] with the binding's reverse `C` context, so the
//! host also controls which layer write-back lands in.
//!
//! [`BindingGraph::report_endpoint_write`] is safe to call for every
//! control-initiated endpoint write: for endpoints that are not two-way
//! targets it degrades to [`BindingSet::mark_endpoint_changed`] and returns
//! `None`.
//!
//! ## Echo suppression
//!
//! Echo suppression is provenance-based, not equality-based:
//!
//! - A reverse write to the source marks other bindings reading that source
//!   dirty, but *skips the pair's own forward binding*. The control's value
//!   therefore does not bounce straight back at the target it came from.
//! - Forward binding writes are delivered through [`BindingHost::set_erased`]
//!   during a drain. They are binding-driven, so the host never reports them
//!   as control writes, and they never schedule reverse propagation.
//!
//! Together with [`BindingSet`]'s single-writer-per-target rule (nothing else
//! may write a two-way target), one control write settles in a single drain:
//! target to source, then forward to any *other* readers of the source, with
//! no path that re-enters the originating target. A host-reported
//! `BindingWrite::did_change() == false` on the reverse write additionally
//! short-circuits downstream dirtying, but that equality check is the host's
//! optimization, not the correctness mechanism.
//!
//! Reverse propagation is deliberately one hop per report: if write-back into
//! the source cascades through other forward bindings onto some *other*
//! two-way target, that binding-driven write does not schedule another
//! reverse. This keeps a drain deterministic and free of hidden echo chains;
//! multiple controls bound two-way to the same source still stay in sync,
//! because the reverse write dirties each sibling's forward binding.
//!
//! ## Conflict policy: target-initiated wins within a drain
//!
//! If the source was changed *and* the control wrote the target before the
//! same drain, the control's value wins: [`BindingGraph::drain`] applies
//! pending reverse writes in host report order before evaluating forward
//! bindings, so a dirty forward binding re-reads the source *after* write-back
//! and re-asserts the control's value. This matches WPF's effective
//! most-recent-write-wins behavior and the interactive expectation that
//! in-flight user input is not clobbered by a same-frame model tick. The
//! reverse write reads the target's *current* value from the host at drain
//! time, so if the same control wrote twice before the drain, the latest value
//! is the one written back; if multiple controls report writes to the same
//! source, the last reported target wins.
//!
//! ## Reports
//!
//! [`BindingGraph::drain`] returns a [`TwoWayReport`] that surfaces reverse
//! writes separately from the wrapped forward [`BindingReport`], so hosts and
//! inspectors can distinguish write-back from ordinary propagation. Reverse
//! writes whose target endpoint has no value yet are skipped and counted,
//! mirroring the one-way missing-source rule; the host re-reports when the
//! value appears. Fallible reverse conversion rejection and reverse target type
//! mismatch are reported as non-poisoning rejections, so one bad target value
//! does not block unrelated reverse writes.
//!
//! ## Minimal example
//!
//! ```rust
//! use invalidation::Channel;
//! use understory_property::{PropertyMetadataBuilder, PropertyRegistry};
//! use understory_property_binding::{BindingGraph, PropertyEndpoint};
//! # use invalidation::ChannelSet;
//! # use understory_property::ErasedValue;
//! # use understory_property_binding::{BindingHost, BindingWrite, EndpointKey};
//! # #[derive(Default)]
//! # struct Host(std::collections::BTreeMap<EndpointKey<u32>, ErasedValue>);
//! # impl BindingHost<u32> for Host {
//! #     fn get_erased(&self, endpoint: EndpointKey<u32>) -> Option<ErasedValue> {
//! #         self.0.get(&endpoint).cloned()
//! #     }
//! #     fn set_erased(&mut self, endpoint: EndpointKey<u32>, value: ErasedValue, (): ()) -> BindingWrite {
//! #         self.0.insert(endpoint, value);
//! #         BindingWrite::changed(ChannelSet::empty())
//! #     }
//! # }
//!
//! const BINDING: Channel = Channel::new(0);
//!
//! let mut registry = PropertyRegistry::new();
//! let value = registry.register("Value", PropertyMetadataBuilder::new(0_u32).build());
//! let model = PropertyEndpoint::new(1, value);
//! let slider = PropertyEndpoint::new(2, value);
//!
//! let mut graph = BindingGraph::new(BINDING);
//! graph.bind_two_way(model, slider, (), ()).unwrap();
//!
//! let mut host = Host::default();
//! # host.0.insert(model.key(), ErasedValue::new(10_u32));
//!
//! // Forward: model changed, slider follows.
//! graph.mark_source_changed(model);
//! graph.drain(&mut host).unwrap();
//!
//! // Reverse: the user drags the slider. The host stores the value itself,
//! // then reports the control-initiated write instead of touching the model.
//! # host.0.insert(slider.key(), ErasedValue::new(70_u32));
//! graph.report_endpoint_write(slider.key());
//! let report = graph.drain(&mut host).unwrap();
//!
//! assert_eq!(report.reverse_writes(), 1);
//! # assert_eq!(host.0.get(&model.key()).and_then(ErasedValue::downcast_ref::<u32>), Some(&70));
//! ```
//!
//! [`BindingGraph`]: crate::BindingGraph
//! [`BindingGraph::drain`]: crate::BindingGraph::drain
//! [`BindingSet::mark_endpoint_changed`]: crate::BindingSet::mark_endpoint_changed
//! [`BindingGraph::report_endpoint_write`]: crate::BindingGraph::report_endpoint_write
//! [`BindingHost::set_erased`]: crate::BindingHost::set_erased
//! [`BindingReport`]: crate::BindingReport
//! [`BindingSet`]: crate::BindingSet

mod bindings;
mod types;

pub(crate) use bindings::TwoWayBindings;
pub use types::{
    ReverseRejection, ReverseRejectionReason, TwoWayBindingId, TwoWayDrainError, TwoWayReport,
};

#[cfg(test)]
mod tests;
