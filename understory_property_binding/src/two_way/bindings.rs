// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::TypeId;
use core::fmt;
use core::hash::Hash;

use hashbrown::HashMap;
use understory_property::ErasedValue;

use crate::endpoint::{BindingId, EndpointKey, PropertyEndpoint};
use crate::error::BindingError;
use crate::host::BindingHost;
use crate::set::BindingSet;

use super::{
    ReverseRejection, ReverseRejectionReason, TwoWayBindingId, TwoWayDrainError, TwoWayReport,
};

/// Result of an erased reverse evaluator.
enum ReverseMapResult {
    /// A source value should be written back.
    Write(ErasedValue),
    /// The target value had the declared type, but the reverse converter
    /// rejected it.
    Rejected,
    /// The host value did not match the declared target endpoint type.
    TargetTypeMismatch { actual: TypeId },
}

/// Erased reverse evaluator: maps a target value to the source value to write
/// back, or reports why the value could not be written.
type ReverseMap = Box<dyn Fn(&ErasedValue) -> ReverseMapResult>;

struct ReverseEntry<K, C> {
    forward: BindingId,
    source: EndpointKey<K>,
    target: EndpointKey<K>,
    reverse_context: C,
    expected_target_type: TypeId,
    reverse_map: ReverseMap,
}

/// Registered two-way bindings and their pending reverse propagation.
///
/// This is policy over a host-owned [`BindingSet`]: each two-way binding's
/// forward direction lives in the binding set as an ordinary one-way binding,
/// while the reverse direction and echo/conflict rules live here. See the
/// [module docs](self) for the host write contract and the propagation rules.
///
/// `K` and `C` match the wrapped [`BindingSet`]: `K` is the host owner key and
/// `C` is binding-owned write context. Each two-way binding stores two
/// contexts, one per direction, so a host with layered local values can route
/// forward writes and reverse write-back into different layers.
pub(crate) struct TwoWayBindings<K, C = ()>
where
    K: Copy + Eq + Hash + 'static,
    C: Copy + 'static,
{
    entries: Vec<Option<ReverseEntry<K, C>>>,
    active_bindings: usize,
    by_target: HashMap<EndpointKey<K>, TwoWayBindingId>,
    pending_reverse: Vec<u32>,
}

impl<K, C> TwoWayBindings<K, C>
where
    K: Copy + Eq + Hash + 'static,
    C: Copy + 'static,
{
    /// Creates an empty two-way binding collection.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
            active_bindings: 0,
            by_target: HashMap::new(),
            pending_reverse: Vec::new(),
        }
    }

    /// Returns the number of active two-way bindings.
    #[must_use]
    pub(crate) const fn len(&self) -> usize {
        self.active_bindings
    }

    /// Returns `true` when reported target writes are waiting for
    /// [`Self::drain`] to propagate back to their sources.
    #[must_use]
    pub(crate) fn has_pending_reverse(&self) -> bool {
        !self.pending_reverse.is_empty()
    }

    /// Returns the [`BindingId`] of a two-way binding's forward direction.
    ///
    /// Returns `None` when the id is unknown or the binding was removed.
    #[must_use]
    pub(crate) fn forward_binding(&self, binding: TwoWayBindingId) -> Option<BindingId> {
        Some(self.entry(binding)?.forward)
    }

    /// Returns the two-way binding whose target is `target`, if any.
    #[must_use]
    pub(crate) fn binding_for_target(&self, target: EndpointKey<K>) -> Option<TwoWayBindingId> {
        let id = self.by_target.get(&target).copied()?;
        self.entry(id).map(|_| id)
    }

    /// Returns the two-way binding whose forward direction is `forward`, if any.
    #[must_use]
    pub(crate) fn binding_for_forward(&self, forward: BindingId) -> Option<TwoWayBindingId> {
        self.entries.iter().enumerate().find_map(|(index, slot)| {
            let entry = slot.as_ref()?;
            let raw = u32::try_from(index).ok()?;
            (entry.forward == forward).then_some(TwoWayBindingId(raw))
        })
    }

    /// Registers a two-way identity binding.
    ///
    /// The source and target endpoint value types must match. Use
    /// [`Self::bind_map`] when the value types differ. `forward_context` is
    /// passed to [`BindingHost::set_erased`] for forward writes to `target`;
    /// `reverse_context` is passed for reverse write-back into `source`.
    ///
    /// # Errors
    ///
    /// Fails with the same [`BindingError`] cases as [`BindingSet::bind`]:
    /// the forward direction is registered in `bindings` and must satisfy the
    /// one-way rules (no self binding, single writer per target, no cycle
    /// through the forward edge).
    pub(crate) fn bind<T>(
        &mut self,
        bindings: &mut BindingSet<K, C>,
        source: PropertyEndpoint<K, T>,
        target: PropertyEndpoint<K, T>,
        forward_context: C,
        reverse_context: C,
    ) -> Result<TwoWayBindingId, BindingError<K>>
    where
        T: Clone + 'static,
    {
        self.bind_map(
            bindings,
            source,
            target,
            forward_context,
            reverse_context,
            T::clone,
            T::clone,
        )
    }

    /// Registers a two-way mapped binding.
    ///
    /// `forward` converts source values for forward writes to `target`;
    /// `reverse` converts target values for reverse write-back into `source`.
    /// The two evaluators should be inverses of each other where the host
    /// expects a control write to round-trip losslessly, but the module does
    /// not check that. Use [`Self::try_bind_map`] when reverse conversion can
    /// reject normal user input.
    ///
    /// The forward direction is registered in `bindings` as an ordinary
    /// one-way binding. The reverse direction is stored here and is not an
    /// edge in the binding dependency graph, so the pair's intentional
    /// back-edge is exempt from cycle rejection while real cycles through the
    /// forward edge are still caught.
    ///
    /// # Errors
    ///
    /// Fails with the same [`BindingError`] cases as [`BindingSet::bind_map`].
    /// On failure nothing is registered.
    #[expect(
        clippy::too_many_arguments,
        reason = "two directions each carry a context and an evaluator"
    )]
    pub(crate) fn bind_map<S, T, F, R>(
        &mut self,
        bindings: &mut BindingSet<K, C>,
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
        self.try_bind_map(
            bindings,
            source,
            target,
            forward_context,
            reverse_context,
            forward,
            move |target| Some(reverse(target)),
        )
    }

    /// Registers a two-way mapped binding with fallible reverse conversion.
    ///
    /// `forward` converts source values for forward writes to `target`;
    /// `reverse` converts target values for reverse write-back into `source`.
    /// Returning `None` rejects that target value for reverse write-back. A
    /// rejected value is counted in [`TwoWayReport::reverse_rejected`] and
    /// described by [`TwoWayReport::reverse_rejections`], but it does not stop
    /// the drain or keep later reverse writes from running.
    ///
    /// The forward direction is registered in `bindings` as an ordinary
    /// one-way binding. The reverse direction is stored here and is not an edge
    /// in the binding dependency graph.
    ///
    /// # Errors
    ///
    /// Fails with the same [`BindingError`] cases as [`BindingSet::bind_map`].
    /// On failure nothing is registered.
    #[expect(
        clippy::too_many_arguments,
        reason = "two directions each carry a context and an evaluator"
    )]
    pub(crate) fn try_bind_map<S, T, F, R>(
        &mut self,
        bindings: &mut BindingSet<K, C>,
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
        let raw = u32::try_from(self.entries.len()).map_err(|_| BindingError::TooManyBindings)?;
        let forward_id = bindings.bind_map(source, target, forward_context, forward)?;
        let id = TwoWayBindingId(raw);

        self.entries.push(Some(ReverseEntry {
            forward: forward_id,
            source: source.key(),
            target: target.key(),
            reverse_context,
            expected_target_type: TypeId::of::<T>(),
            reverse_map: Box::new(move |value| match value.downcast_ref::<T>() {
                Some(target_value) => reverse(target_value)
                    .map_or(ReverseMapResult::Rejected, |source_value| {
                        ReverseMapResult::Write(ErasedValue::new(source_value))
                    }),
                None => ReverseMapResult::TargetTypeMismatch {
                    actual: value.type_id(),
                },
            }),
        }));
        self.active_bindings += 1;
        self.by_target.insert(target.key(), id);

        Ok(id)
    }

    /// Removes one two-way binding, cleaning up both directions.
    ///
    /// The forward direction is unbound from `bindings` and any pending
    /// reverse propagation for the pair is dropped. Returns `false` when the
    /// id is unknown or was already removed. Ids are stable and not reused.
    pub(crate) fn unbind(
        &mut self,
        bindings: &mut BindingSet<K, C>,
        binding: TwoWayBindingId,
    ) -> bool {
        let Some(index) = binding.index() else {
            return false;
        };
        let Some(slot) = self.entries.get_mut(index) else {
            return false;
        };
        let Some(entry) = slot.take() else {
            return false;
        };

        self.active_bindings -= 1;
        self.by_target.remove(&entry.target);
        self.pending_reverse.retain(|raw| *raw != binding.get());
        bindings.unbind(entry.forward);
        true
    }

    /// Removes active two-way bindings owned by `owner`.
    ///
    /// A binding is removed when either its source owner or target owner
    /// matches `owner`, mirroring [`BindingSet::clear_owner`]. Returns the
    /// number of two-way bindings removed.
    pub(crate) fn clear_owner(&mut self, bindings: &mut BindingSet<K, C>, owner: K) -> usize {
        let matches: Vec<TwoWayBindingId> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                let entry = slot.as_ref()?;
                let raw = u32::try_from(index).ok()?;
                (entry.source.owner() == owner || entry.target.owner() == owner)
                    .then_some(TwoWayBindingId(raw))
            })
            .collect();
        matches
            .into_iter()
            .filter(|binding| self.unbind(bindings, *binding))
            .count()
    }

    /// Removes active two-way bindings that read from or write to `endpoint`.
    ///
    /// This mirrors [`BindingSet::clear_endpoint`] for the two-way sidecar:
    /// a binding is removed when either its source endpoint or target endpoint
    /// matches `endpoint`. Returns the number of two-way bindings removed.
    pub(crate) fn clear_endpoint(
        &mut self,
        bindings: &mut BindingSet<K, C>,
        endpoint: EndpointKey<K>,
    ) -> usize {
        let matches: Vec<TwoWayBindingId> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                let entry = slot.as_ref()?;
                let raw = u32::try_from(index).ok()?;
                (entry.source == endpoint || entry.target == endpoint)
                    .then_some(TwoWayBindingId(raw))
            })
            .collect();
        matches
            .into_iter()
            .filter(|binding| self.unbind(bindings, *binding))
            .count()
    }

    /// Reports a control-initiated write to an endpoint.
    ///
    /// This is the write-routing half of the two-way contract (see the
    /// [module docs](self)): the host writes the new value into its own store
    /// first, then reports the endpoint here instead of calling
    /// [`BindingSet::mark_endpoint_changed`] directly.
    ///
    /// The report always marks ordinary bindings reading `target` dirty. When
    /// `target` is the target endpoint of an active two-way binding, reverse
    /// propagation to that binding's source is additionally scheduled for the
    /// next [`Self::drain`], and the two-way id is returned. The reverse write
    /// reads the endpoint's value at drain time, so reporting the same
    /// endpoint several times before a drain performs one write-back with the
    /// latest value, and moves that write-back to the end of the pending batch.
    ///
    /// Do not report binding-driven writes (values arriving through
    /// [`BindingHost::set_erased`]); provenance-based echo suppression relies
    /// on only control-initiated writes being reported.
    pub(crate) fn report_target_write(
        &mut self,
        bindings: &mut BindingSet<K, C>,
        target: EndpointKey<K>,
    ) -> Option<TwoWayBindingId> {
        self.report_target_write_state(bindings, target).1
    }

    /// Reports a control-initiated write and returns both dirtying effects.
    pub(crate) fn report_target_write_state(
        &mut self,
        bindings: &mut BindingSet<K, C>,
        target: EndpointKey<K>,
    ) -> (bool, Option<TwoWayBindingId>) {
        let dirtied = bindings.mark_endpoint_changed(target);
        let Some(id) = self.by_target.get(&target).copied() else {
            return (dirtied, None);
        };
        if self.entry(id).is_none() {
            return (dirtied, None);
        }
        let raw = id.get();
        self.pending_reverse.retain(|pending| *pending != raw);
        self.pending_reverse.push(raw);
        (dirtied, Some(id))
    }

    /// Applies pending reverse write-back, then drains dirty forward bindings.
    ///
    /// Reverse writes are applied first, in host report order: each
    /// pending pair reads its target endpoint's current value from the host,
    /// maps it through the reverse evaluator, and writes the source endpoint
    /// with the pair's reverse context. A changed reverse write marks other
    /// bindings reading the source dirty, but never the pair's own forward
    /// binding (provenance-based echo suppression). [`BindingSet::drain`]
    /// then runs as usual.
    ///
    /// Applying reverse writes first is the conflict rule: when a source
    /// change and a reported target write race into the same drain, the
    /// forward binding re-reads the source after write-back, so the
    /// target-initiated value wins (see the [module docs](self)).
    ///
    /// A pending pair whose target endpoint has no value is skipped, stays
    /// unscheduled, and is counted in
    /// [`TwoWayReport::reverse_skipped_missing_target`]; the host re-reports
    /// once the value exists. Reverse conversion rejection and reverse target
    /// type mismatch are recorded in [`TwoWayReport::reverse_rejections`] and
    /// do not stop unrelated reverse writes.
    ///
    /// # Errors
    ///
    /// Returns [`TwoWayDrainError`] when the forward drain fails; the partial
    /// [`TwoWayReport`] still carries completed reverse writes and rejections.
    pub(crate) fn drain(
        &mut self,
        bindings: &mut BindingSet<K, C>,
        host: &mut dyn BindingHost<K, C>,
    ) -> Result<TwoWayReport<K>, TwoWayDrainError<K>> {
        let mut report = TwoWayReport::default();

        let pending = core::mem::take(&mut self.pending_reverse);

        for raw in pending {
            let id = TwoWayBindingId(raw);
            let Some(entry) = self.entry(id) else {
                continue;
            };

            let source = entry.source;
            let target = entry.target;
            let forward = entry.forward;
            let expected = entry.expected_target_type;
            let reverse_context = entry.reverse_context;
            let Some(erased) = host.get_erased(entry.target) else {
                report.reverse_skipped_missing_target += 1;
                continue;
            };

            let mapped = match (entry.reverse_map)(&erased) {
                ReverseMapResult::Write(mapped) => mapped,
                ReverseMapResult::Rejected => {
                    report.record_reverse_rejection(ReverseRejection::new(
                        id,
                        forward,
                        source,
                        target,
                        ReverseRejectionReason::ConversionRejected,
                    ));
                    continue;
                }
                ReverseMapResult::TargetTypeMismatch { actual } => {
                    report.record_reverse_rejection(ReverseRejection::new(
                        id,
                        forward,
                        source,
                        target,
                        ReverseRejectionReason::TargetTypeMismatch { expected, actual },
                    ));
                    continue;
                }
            };

            let write = host.set_erased(source, mapped, reverse_context);
            report.record_reverse(write);
            if write.did_change() {
                bindings.mark_endpoint_changed_except(source, forward);
            }
        }

        match bindings.drain(host) {
            Ok(forward) => {
                report.forward = forward;
                Ok(report)
            }
            Err(error) => {
                let (error, forward) = error.into_parts();
                report.forward = forward;
                Err(TwoWayDrainError::new(error, report))
            }
        }
    }

    fn entry(&self, binding: TwoWayBindingId) -> Option<&ReverseEntry<K, C>> {
        self.entries.get(binding.index()?)?.as_ref()
    }
}

impl<K, C> Default for TwoWayBindings<K, C>
where
    K: Copy + Eq + Hash + 'static,
    C: Copy + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, C> fmt::Debug for TwoWayBindings<K, C>
where
    K: Copy + Eq + Hash + 'static,
    C: Copy + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TwoWayBindings")
            .field("len", &self.len())
            .field("pending_reverse", &self.pending_reverse.len())
            .finish_non_exhaustive()
    }
}
