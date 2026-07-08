// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Safe composite binding graph for one-way and two-way bindings.
//!
//! This module owns the host-facing lifecycle for combined one-way and two-way
//! bindings; it explicitly does not own property storage, application
//! invalidation, or property-domain validation.

use core::fmt;
use core::hash::Hash;

use invalidation::Channel;

use crate::endpoint::{BindingId, EndpointKey, PropertyEndpoint};
use crate::error::BindingError;
use crate::host::BindingHost;
use crate::report::BindingStats;
use crate::set::BindingSet;
use crate::two_way::TwoWayBindings;
use crate::{TwoWayBindingId, TwoWayDrainError, TwoWayReport};

/// One owner for one-way bindings, two-way policy, dirty state, and teardown.
///
/// Use `BindingGraph` for application/framework integrations that may register
/// two-way bindings, or for hosts that want one teardown path for both one-way
/// and two-way declarations. Create one with [`BindingGraph::new`]. Register
/// ordinary one-way bindings with [`BindingGraph::bind`] or
/// [`BindingGraph::bind_map`], and register two-way bindings with
/// [`BindingGraph::bind_two_way`], [`BindingGraph::bind_two_way_map`], or
/// [`BindingGraph::try_bind_two_way_map`].
///
/// Host/control writes to endpoints should be routed through
/// [`BindingGraph::report_endpoint_write`] when the endpoint may be a two-way
/// target. Binding-driven writes delivered through [`BindingHost::set_erased`]
/// during [`BindingGraph::drain`] are already accounted for and must not be
/// reported back to the graph.
///
/// `BindingGraph` wraps a [`BindingSet`] plus internal two-way policy so hosts
/// do not have to keep those two structures synchronized by hand. If an
/// integration only needs one-way propagation, use [`BindingSet`] directly.
///
/// `K` is the host owner key and `C` is binding-owned write context. Both match
/// the wrapped [`BindingSet`].
///
/// ```rust,ignore
/// let mut graph = BindingGraph::new(BINDING);
/// let binding = graph.bind_two_way(model_value, slider_value, (), ())?;
///
/// // A user gesture writes the control value into host storage, then reports
/// // the endpoint. The next drain writes the value back to the source.
/// graph.report_endpoint_write(slider_value.key());
/// let report = graph.drain(&mut host)?;
///
/// assert!(graph.forward_binding(binding).is_some());
/// assert_eq!(report.reverse_writes(), 1);
/// ```
pub struct BindingGraph<K, C = ()>
where
    K: Copy + Eq + Hash + 'static,
    C: Copy + 'static,
{
    bindings: BindingSet<K, C>,
    two_way: TwoWayBindings<K, C>,
}

impl<K, C> BindingGraph<K, C>
where
    K: Copy + Eq + Hash + 'static,
    C: Copy + 'static,
{
    /// Creates an empty binding graph.
    ///
    /// The channel is used only inside the wrapped binding set's tracker. It
    /// does not reserve or define an application-level invalidation channel.
    #[must_use]
    pub fn new(binding_channel: Channel) -> Self {
        Self {
            bindings: BindingSet::new(binding_channel),
            two_way: TwoWayBindings::new(),
        }
    }

    /// Returns the binding-local invalidation channel.
    #[must_use]
    pub const fn binding_channel(&self) -> Channel {
        self.bindings.binding_channel()
    }

    /// Returns the number of active forward graph edges.
    ///
    /// This includes ordinary one-way bindings and the forward direction of
    /// each two-way binding.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Returns the number of active two-way binding declarations.
    #[must_use]
    pub const fn two_way_len(&self) -> usize {
        self.two_way.len()
    }

    /// Returns `true` when no bindings are active.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Returns a snapshot of the underlying forward binding graph.
    #[must_use]
    pub fn stats(&self) -> BindingStats {
        self.bindings.stats()
    }

    /// Returns `true` when dirty forward bindings are waiting to drain.
    #[must_use]
    pub fn has_dirty_bindings(&self) -> bool {
        self.bindings.has_dirty_bindings()
    }

    /// Returns `true` when reported target writes are waiting for reverse
    /// propagation.
    #[must_use]
    pub fn has_pending_reverse(&self) -> bool {
        self.two_way.has_pending_reverse()
    }

    /// Registers a one-way identity binding.
    ///
    /// Use this for ordinary one-way propagation inside a graph-owned host
    /// integration. The returned [`BindingId`] can be used with
    /// [`Self::mark_binding_dirty`] or [`Self::unbind`].
    pub fn bind<T>(
        &mut self,
        source: PropertyEndpoint<K, T>,
        target: PropertyEndpoint<K, T>,
        write_context: C,
    ) -> Result<BindingId, BindingError<K>>
    where
        T: Clone + 'static,
    {
        self.bindings.bind(source, target, write_context)
    }

    /// Registers a one-way mapped binding.
    ///
    /// Use this when the source and target endpoint value types differ, or
    /// when the target needs a derived value. The returned [`BindingId`] can be
    /// used with [`Self::mark_binding_dirty`] or [`Self::unbind`].
    pub fn bind_map<S, T, F>(
        &mut self,
        source: PropertyEndpoint<K, S>,
        target: PropertyEndpoint<K, T>,
        write_context: C,
        map: F,
    ) -> Result<BindingId, BindingError<K>>
    where
        S: Clone + 'static,
        T: Clone + 'static,
        F: Fn(&S) -> T + 'static,
    {
        self.bindings.bind_map(source, target, write_context, map)
    }

    /// Registers a two-way identity binding.
    ///
    /// The returned [`TwoWayBindingId`] identifies the pair for diagnostics,
    /// [`Self::forward_binding`], and [`Self::unbind_two_way`]. The forward
    /// direction is also a normal binding in the wrapped set, but callers do
    /// not need to manage that binding separately.
    pub fn bind_two_way<T>(
        &mut self,
        source: PropertyEndpoint<K, T>,
        target: PropertyEndpoint<K, T>,
        forward_context: C,
        reverse_context: C,
    ) -> Result<TwoWayBindingId, BindingError<K>>
    where
        T: Clone + 'static,
    {
        self.two_way.bind(
            &mut self.bindings,
            source,
            target,
            forward_context,
            reverse_context,
        )
    }

    /// Registers a two-way mapped binding with infallible reverse conversion.
    ///
    /// Use this when both conversion directions are total for the endpoint
    /// types. If user input can be invalid for the source type, prefer
    /// [`Self::try_bind_two_way_map`] so the reverse conversion can reject it
    /// without coercing it into a misleading value.
    pub fn bind_two_way_map<S, T, F, R>(
        &mut self,
        source: PropertyEndpoint<K, S>,
        target: PropertyEndpoint<K, T>,
        forward_context: C,
        reverse_context: C,
        forward: F,
        reverse: R,
    ) -> Result<TwoWayBindingId, BindingError<K>>
    where
        S: Clone + 'static,
        T: Clone + 'static,
        F: Fn(&S) -> T + 'static,
        R: Fn(&T) -> S + 'static,
    {
        self.two_way.bind_map(
            &mut self.bindings,
            source,
            target,
            forward_context,
            reverse_context,
            forward,
            reverse,
        )
    }

    /// Registers a two-way mapped binding with fallible reverse conversion.
    ///
    /// Returning `None` from `reverse` skips that reverse write-back and records
    /// a [`crate::ReverseRejection`] in the next [`TwoWayReport`]. Rejections
    /// do not stop unrelated reverse writes or forward propagation.
    pub fn try_bind_two_way_map<S, T, F, R>(
        &mut self,
        source: PropertyEndpoint<K, S>,
        target: PropertyEndpoint<K, T>,
        forward_context: C,
        reverse_context: C,
        forward: F,
        reverse: R,
    ) -> Result<TwoWayBindingId, BindingError<K>>
    where
        S: Clone + 'static,
        T: Clone + 'static,
        F: Fn(&S) -> T + 'static,
        R: Fn(&T) -> Option<S> + 'static,
    {
        self.two_way.try_bind_map(
            &mut self.bindings,
            source,
            target,
            forward_context,
            reverse_context,
            forward,
            reverse,
        )
    }

    /// Removes one binding declaration.
    ///
    /// If `binding` is the forward direction of a two-way binding, both
    /// directions are removed. Returns `false` when the id is unknown or already
    /// removed.
    pub fn unbind(&mut self, binding: BindingId) -> bool {
        if let Some(two_way) = self.two_way.binding_for_forward(binding) {
            self.two_way.unbind(&mut self.bindings, two_way)
        } else {
            self.bindings.unbind(binding)
        }
    }

    /// Removes one two-way binding declaration.
    ///
    /// The pair's forward binding is removed from the graph and pending reverse
    /// propagation for the pair is dropped.
    pub fn unbind_two_way(&mut self, binding: TwoWayBindingId) -> bool {
        self.two_way.unbind(&mut self.bindings, binding)
    }

    /// Removes bindings that read from or write to `endpoint`.
    ///
    /// Two-way bindings are removed as pairs, so this cannot orphan reverse
    /// write-back state. Returns the number of binding declarations removed.
    pub fn clear_endpoint(&mut self, endpoint: EndpointKey<K>) -> usize {
        let removed_two_way = self.two_way.clear_endpoint(&mut self.bindings, endpoint);
        removed_two_way + self.bindings.clear_endpoint(endpoint)
    }

    /// Removes bindings owned by `owner`.
    ///
    /// Two-way bindings are removed as pairs, so this cannot orphan reverse
    /// write-back state. Returns the number of binding declarations removed.
    pub fn clear_owner(&mut self, owner: K) -> usize {
        let removed_two_way = self.two_way.clear_owner(&mut self.bindings, owner);
        removed_two_way + self.bindings.clear_owner(owner)
    }

    /// Reports that an endpoint's observable value changed.
    ///
    /// This marks all bindings that read `source` dirty. If `source` is also
    /// the target of a two-way binding, reverse write-back is scheduled. Use
    /// this for typed host/application change notifications. Binding-driven
    /// writes delivered through [`BindingHost::set_erased`] must not be
    /// reported here.
    pub fn mark_source_changed<T>(&mut self, source: PropertyEndpoint<K, T>) -> bool {
        self.mark_endpoint_changed(source.key())
    }

    /// Reports that an endpoint's observable value changed using an untyped key.
    ///
    /// This marks all bindings that read `source` dirty. If `source` is also
    /// the target of a two-way binding, reverse write-back is scheduled. Use
    /// [`Self::report_endpoint_write`] instead when the caller needs the
    /// scheduled [`TwoWayBindingId`].
    pub fn mark_endpoint_changed(&mut self, source: EndpointKey<K>) -> bool {
        let (dirtied, two_way) = self
            .two_way
            .report_target_write_state(&mut self.bindings, source);
        dirtied || two_way.is_some()
    }

    /// Marks a specific forward binding as dirty.
    ///
    /// For a two-way binding, pass the id returned by [`Self::forward_binding`].
    pub fn mark_binding_dirty(&mut self, binding: BindingId) -> bool {
        self.bindings.mark_binding_dirty(binding)
    }

    /// Reports a host/control-initiated endpoint write.
    ///
    /// This marks ordinary bindings that read `endpoint` dirty. If `endpoint`
    /// is the target of a two-way binding, reverse write-back is also scheduled
    /// and the two-way id is returned. Binding-driven writes delivered through
    /// [`BindingHost::set_erased`] must not be reported here.
    pub fn report_endpoint_write(&mut self, endpoint: EndpointKey<K>) -> Option<TwoWayBindingId> {
        self.two_way
            .report_target_write(&mut self.bindings, endpoint)
    }

    /// Drains pending reverse write-back and dirty forward bindings.
    ///
    /// This is the graph equivalent of [`BindingSet::drain`]. It returns a
    /// [`TwoWayReport`] so callers can inspect reverse write-back separately
    /// from ordinary forward binding propagation.
    pub fn drain(
        &mut self,
        host: &mut dyn BindingHost<K, C>,
    ) -> Result<TwoWayReport<K>, TwoWayDrainError<K>> {
        self.two_way.drain(&mut self.bindings, host)
    }

    /// Returns the forward binding id for a two-way binding.
    #[must_use]
    pub fn forward_binding(&self, binding: TwoWayBindingId) -> Option<BindingId> {
        self.two_way.forward_binding(binding)
    }

    /// Returns the two-way binding whose target is `target`, if any.
    #[must_use]
    pub fn two_way_for_target(&self, target: EndpointKey<K>) -> Option<TwoWayBindingId> {
        self.two_way.binding_for_target(target)
    }

    /// Returns the two-way binding whose forward direction is `forward`, if any.
    #[must_use]
    pub fn two_way_for_forward(&self, forward: BindingId) -> Option<TwoWayBindingId> {
        self.two_way.binding_for_forward(forward)
    }
}

impl<K, C> fmt::Debug for BindingGraph<K, C>
where
    K: Copy + Eq + Hash + 'static,
    C: Copy + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingGraph")
            .field("binding_channel", &self.binding_channel())
            .field("len", &self.len())
            .field("two_way_len", &self.two_way_len())
            .field("has_dirty_bindings", &self.has_dirty_bindings())
            .field("has_pending_reverse", &self.has_pending_reverse())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;
    use alloc::string::{String, ToString};

    use invalidation::{Channel, ChannelSet};
    use understory_property::{ErasedValue, Property, PropertyMetadataBuilder, PropertyRegistry};

    use super::BindingGraph;
    use crate::{BindingHost, BindingWrite, EndpointKey, PropertyEndpoint};

    const BINDING: Channel = Channel::new(0);

    #[derive(Default)]
    struct TestHost {
        values: BTreeMap<EndpointKey<u32>, ErasedValue>,
    }

    impl TestHost {
        fn set_initial<T: Clone + 'static>(
            &mut self,
            endpoint: PropertyEndpoint<u32, T>,
            value: T,
        ) {
            self.values.insert(endpoint.key(), ErasedValue::new(value));
        }

        fn value<T: 'static>(&self, endpoint: PropertyEndpoint<u32, T>) -> Option<&T> {
            self.values
                .get(&endpoint.key())
                .and_then(ErasedValue::downcast_ref)
        }

        fn erased_equal(left: &ErasedValue, right: &ErasedValue) -> bool {
            if left.type_id() != right.type_id() {
                return false;
            }
            if let (Some(left), Some(right)) =
                (left.downcast_ref::<u32>(), right.downcast_ref::<u32>())
            {
                return left == right;
            }
            if let (Some(left), Some(right)) = (
                left.downcast_ref::<String>(),
                right.downcast_ref::<String>(),
            ) {
                return left == right;
            }
            false
        }
    }

    impl BindingHost<u32> for TestHost {
        fn get_erased(&self, endpoint: EndpointKey<u32>) -> Option<ErasedValue> {
            self.values.get(&endpoint).cloned()
        }

        fn set_erased(
            &mut self,
            endpoint: EndpointKey<u32>,
            value: ErasedValue,
            (): (),
        ) -> BindingWrite {
            let changed = self
                .values
                .get(&endpoint)
                .is_none_or(|old| !Self::erased_equal(old, &value));
            self.values.insert(endpoint, value);
            BindingWrite::new(changed, ChannelSet::empty())
        }
    }

    fn registry() -> (PropertyRegistry, Property<u32>) {
        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0_u32).build());
        (registry, width)
    }

    #[test]
    fn one_way_binding_drains_through_graph() {
        let (_registry, width) = registry();
        let source = PropertyEndpoint::new(1, width);
        let target = PropertyEndpoint::new(2, width);

        let mut graph = BindingGraph::new(BINDING);
        graph.bind(source, target, ()).unwrap();

        let mut host = TestHost::default();
        host.set_initial(source, 42_u32);
        graph.mark_source_changed(source);

        let report = graph.drain(&mut host).unwrap();

        assert_eq!(report.forward().evaluated_bindings(), 1);
        assert_eq!(host.value(target), Some(&42));
    }

    #[test]
    fn mark_endpoint_changed_schedules_two_way_reverse() {
        let (_registry, width) = registry();
        let model = PropertyEndpoint::new(1, width);
        let control = PropertyEndpoint::new(2, width);

        let mut graph = BindingGraph::new(BINDING);
        graph.bind_two_way(model, control, (), ()).unwrap();

        let mut host = TestHost::default();
        host.set_initial(model, 10_u32);
        host.set_initial(control, 70_u32);

        assert!(graph.mark_endpoint_changed(control.key()));
        assert!(graph.has_pending_reverse());

        let report = graph.drain(&mut host).unwrap();

        assert_eq!(report.reverse_writes(), 1);
        assert_eq!(host.value(model), Some(&70));
    }

    #[test]
    fn clear_owner_drops_pending_two_way_reverse() {
        let (_registry, width) = registry();
        let model = PropertyEndpoint::new(1, width);
        let control = PropertyEndpoint::new(2, width);

        let mut graph = BindingGraph::new(BINDING);
        graph.bind_two_way(model, control, (), ()).unwrap();

        let mut host = TestHost::default();
        host.set_initial(model, 10_u32);
        host.set_initial(control, 70_u32);
        graph.report_endpoint_write(control.key());

        assert_eq!(graph.clear_owner(2), 1);
        assert!(graph.is_empty());
        assert!(!graph.has_pending_reverse());

        let report = graph.drain(&mut host).unwrap();
        assert_eq!(report.reverse_writes(), 0);
        assert_eq!(host.value(model), Some(&10));
    }

    #[test]
    fn clear_endpoint_drops_pending_two_way_reverse() {
        let (_registry, width) = registry();
        let model = PropertyEndpoint::new(1, width);
        let control = PropertyEndpoint::new(2, width);

        let mut graph = BindingGraph::new(BINDING);
        graph.bind_two_way(model, control, (), ()).unwrap();

        let mut host = TestHost::default();
        host.set_initial(model, 10_u32);
        host.set_initial(control, 70_u32);
        graph.report_endpoint_write(control.key());

        assert_eq!(graph.clear_endpoint(control.key()), 1);
        assert!(graph.is_empty());
        assert!(!graph.has_pending_reverse());

        let report = graph.drain(&mut host).unwrap();
        assert_eq!(report.reverse_writes(), 0);
        assert_eq!(host.value(model), Some(&10));
    }

    #[test]
    fn unbind_forward_id_removes_two_way_pair() {
        let (_registry, width) = registry();
        let model = PropertyEndpoint::new(1, width);
        let control = PropertyEndpoint::new(2, width);

        let mut graph = BindingGraph::new(BINDING);
        let two_way = graph.bind_two_way(model, control, (), ()).unwrap();
        let forward = graph.forward_binding(two_way).unwrap();

        assert!(graph.unbind(forward));
        assert!(graph.is_empty());
        assert_eq!(graph.two_way_len(), 0);
        assert_eq!(graph.two_way_for_target(control.key()), None);
    }

    #[test]
    fn try_two_way_map_reports_rejected_reverse_conversion() {
        let mut registry = PropertyRegistry::new();
        let count = registry.register("Count", PropertyMetadataBuilder::new(0_u32).build());
        let text = registry.register("Text", PropertyMetadataBuilder::new(String::new()).build());
        let model = PropertyEndpoint::new(1, count);
        let editor = PropertyEndpoint::new(2, text);

        let mut graph = BindingGraph::new(BINDING);
        graph
            .try_bind_two_way_map(
                model,
                editor,
                (),
                (),
                |count: &u32| count.to_string(),
                |text: &String| text.parse::<u32>().ok(),
            )
            .unwrap();

        let mut host = TestHost::default();
        host.set_initial(model, 5_u32);
        host.set_initial(editor, String::from("nope"));
        graph.report_endpoint_write(editor.key());

        let report = graph.drain(&mut host).unwrap();

        assert_eq!(report.reverse_writes(), 0);
        assert_eq!(report.reverse_rejected(), 1);
        assert_eq!(host.value(model), Some(&5));
    }
}
