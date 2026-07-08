// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::any::TypeId;

use invalidation::ChannelSet;

use crate::endpoint::{BindingId, EndpointKey};
use crate::error::BindingError;
use crate::report::{BindingReport, BindingWrite};

/// Identifier for a two-way binding registered in a [`crate::BindingGraph`].
///
/// [`crate::BindingGraph::bind_two_way`],
/// [`crate::BindingGraph::bind_two_way_map`], and
/// [`crate::BindingGraph::try_bind_two_way_map`] return this id. Pass it to
/// [`crate::BindingGraph::forward_binding`] to inspect the pair's ordinary
/// forward [`BindingId`], or to [`crate::BindingGraph::unbind_two_way`] to
/// remove both directions. Two-way ids are stable and are not reused.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TwoWayBindingId(pub(super) u32);

impl TwoWayBindingId {
    /// Returns the raw integer id.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    pub(super) fn index(self) -> Option<usize> {
        usize::try_from(self.0).ok()
    }
}

/// Reason a pending reverse write-back was rejected.
///
/// Values of this enum are observed through [`ReverseRejection::reason`] on
/// the [`ReverseRejection`] entries returned by [`TwoWayReport::reverse_rejections`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReverseRejectionReason {
    /// The host value matched the target endpoint type, but the binding's
    /// reverse converter rejected the value.
    ConversionRejected,
    /// The host returned a target value with a different runtime type than the
    /// two-way binding declaration expects.
    TargetTypeMismatch {
        /// The type expected by the target endpoint declaration.
        expected: TypeId,
        /// The type returned by the host.
        actual: TypeId,
    },
}

/// A pending reverse write-back that was rejected instead of written.
///
/// Callers get these from [`TwoWayReport::reverse_rejections`] after
/// [`crate::BindingGraph::drain`]. Rejections are non-poisoning: they do not
/// stop the drain and they do not stay scheduled. The host can inspect the
/// rejected binding and endpoint pair, repair the value if appropriate, and
/// report the target write again with
/// [`crate::BindingGraph::report_endpoint_write`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReverseRejection<K> {
    binding: TwoWayBindingId,
    forward: BindingId,
    source: EndpointKey<K>,
    target: EndpointKey<K>,
    reason: ReverseRejectionReason,
}

impl<K> ReverseRejection<K> {
    pub(super) const fn new(
        binding: TwoWayBindingId,
        forward: BindingId,
        source: EndpointKey<K>,
        target: EndpointKey<K>,
        reason: ReverseRejectionReason,
    ) -> Self {
        Self {
            binding,
            forward,
            source,
            target,
            reason,
        }
    }

    /// Returns the rejected two-way binding id.
    #[must_use]
    pub const fn binding(&self) -> TwoWayBindingId {
        self.binding
    }

    /// Returns the forward binding id for the rejected pair.
    #[must_use]
    pub const fn forward_binding(&self) -> BindingId {
        self.forward
    }

    /// Returns why reverse write-back was rejected.
    #[must_use]
    pub const fn reason(&self) -> ReverseRejectionReason {
        self.reason
    }
}

impl<K: Copy> ReverseRejection<K> {
    /// Returns the source endpoint that would have been written.
    #[must_use]
    pub const fn source(&self) -> EndpointKey<K> {
        self.source
    }

    /// Returns the target endpoint whose value could not be mapped.
    #[must_use]
    pub const fn target(&self) -> EndpointKey<K> {
        self.target
    }
}
/// Summary returned by [`crate::BindingGraph::drain`].
///
/// Most callers get this from [`crate::BindingGraph::drain`] after the graph
/// drains pending reverse write-back and dirty forward bindings. Reverse
/// write-back is reported separately from the wrapped forward [`BindingReport`],
/// so hosts and inspectors can tell control-initiated write-back apart from
/// ordinary forward propagation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TwoWayReport<K> {
    reverse_writes: usize,
    reverse_changed: usize,
    pub(super) reverse_skipped_missing_target: usize,
    reverse_rejections: Vec<ReverseRejection<K>>,
    reverse_channels: ChannelSet,
    pub(super) forward: BindingReport,
}

impl<K> Default for TwoWayReport<K> {
    fn default() -> Self {
        Self {
            reverse_writes: 0,
            reverse_changed: 0,
            reverse_skipped_missing_target: 0,
            reverse_rejections: Vec::new(),
            reverse_channels: ChannelSet::empty(),
            forward: BindingReport::default(),
        }
    }
}

impl<K> TwoWayReport<K> {
    /// Returns the number of reverse write-back evaluations that ran.
    #[must_use]
    pub const fn reverse_writes(&self) -> usize {
        self.reverse_writes
    }

    /// Returns the number of reverse writes that changed observable values.
    #[must_use]
    pub const fn reverse_changed(&self) -> usize {
        self.reverse_changed
    }

    /// Returns the number of pending reverse writes skipped because the host
    /// reported no value for the pair's target endpoint. Those pairs stay
    /// unscheduled; the host re-reports the write once the value exists.
    #[must_use]
    pub const fn reverse_skipped_missing_target(&self) -> usize {
        self.reverse_skipped_missing_target
    }

    /// Returns the number of pending reverse writes skipped by reverse
    /// conversion rejection or reverse target type mismatch.
    #[must_use]
    pub fn reverse_rejected(&self) -> usize {
        self.reverse_rejections.len()
    }

    /// Returns reverse write-back rejections in the order they were observed.
    #[must_use]
    pub fn reverse_rejections(&self) -> &[ReverseRejection<K>] {
        &self.reverse_rejections
    }

    /// Returns the application channels affected by reverse writes only.
    #[must_use]
    pub const fn reverse_channels(&self) -> ChannelSet {
        self.reverse_channels
    }

    /// Returns the forward drain report from the wrapped [`crate::BindingSet`].
    #[must_use]
    pub const fn forward(&self) -> BindingReport {
        self.forward
    }

    /// Returns the union of application channels affected by reverse and
    /// forward writes.
    #[must_use]
    pub fn affected_channels(&self) -> ChannelSet {
        self.reverse_channels | self.forward.affected_channels()
    }

    pub(super) fn record_reverse(&mut self, write: BindingWrite) {
        self.reverse_writes += 1;
        if write.did_change() {
            self.reverse_changed += 1;
        }
        self.reverse_channels |= write.affected_channels();
    }

    pub(super) fn record_reverse_rejection(&mut self, rejection: ReverseRejection<K>) {
        self.reverse_rejections.push(rejection);
    }
}

/// Error returned when [`crate::BindingGraph::drain`] stops before completion.
///
/// This is the two-way analogue of [`crate::BindingDrainError`]. It is produced
/// only by [`crate::BindingGraph::drain`]; reverse conversion rejections are
/// recorded in [`TwoWayReport`] instead of becoming this error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TwoWayDrainError<K> {
    error: BindingError<K>,
    report: TwoWayReport<K>,
}

impl<K> TwoWayDrainError<K> {
    pub(super) const fn new(error: BindingError<K>, report: TwoWayReport<K>) -> Self {
        Self { error, report }
    }

    /// Returns the binding error that stopped the drain.
    #[must_use]
    pub const fn error(&self) -> &BindingError<K> {
        &self.error
    }

    /// Returns the report for writes completed before the error.
    ///
    /// Those writes are not rolled back. Hosts should still apply the affected
    /// channels from this report to their application-level invalidation
    /// state.
    #[must_use]
    pub const fn report(&self) -> &TwoWayReport<K> {
        &self.report
    }

    /// Consumes the error and returns the binding error and partial report.
    #[must_use]
    pub fn into_parts(self) -> (BindingError<K>, TwoWayReport<K>) {
        (self.error, self.report)
    }
}
