// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use invalidation::ChannelSet;

/// Result of writing a binding target endpoint.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BindingWrite {
    changed: bool,
    affected_channels: ChannelSet,
}

impl BindingWrite {
    /// Creates a write result.
    ///
    /// `changed` reports whether the target endpoint's observable value changed.
    /// `affected_channels` reports the host application channels dirtied by that
    /// change.
    #[must_use]
    pub const fn new(changed: bool, affected_channels: ChannelSet) -> Self {
        Self {
            changed,
            affected_channels,
        }
    }

    /// Creates a write result for an unchanged target endpoint.
    #[must_use]
    pub const fn unchanged() -> Self {
        Self::new(false, ChannelSet::empty())
    }

    /// Creates a write result for a changed target endpoint.
    #[must_use]
    pub const fn changed(affected_channels: ChannelSet) -> Self {
        Self::new(true, affected_channels)
    }

    /// Returns whether the target endpoint's observable value changed.
    #[must_use]
    pub const fn did_change(self) -> bool {
        self.changed
    }

    /// Returns the application channels dirtied by the write.
    #[must_use]
    pub const fn affected_channels(self) -> ChannelSet {
        self.affected_channels
    }
}

/// Summary returned by [`BindingSet::drain`].
///
/// [`BindingSet::drain`]: crate::BindingSet::drain
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct BindingReport {
    evaluated_bindings: usize,
    changed_bindings: usize,
    skipped_missing_source: usize,
    affected_channels: ChannelSet,
}

impl BindingReport {
    /// Returns the number of binding evaluators that ran.
    #[must_use]
    pub const fn evaluated_bindings(self) -> usize {
        self.evaluated_bindings
    }

    /// Returns the number of binding target writes that changed observable values.
    #[must_use]
    pub const fn changed_bindings(self) -> usize {
        self.changed_bindings
    }

    /// Returns the number of bindings the drain skipped because the host
    /// reported no value for the source endpoint. Those bindings stay clean
    /// and will be re-dirtied via [`BindingSet::mark_endpoint_changed`]
    /// when the source is written.
    ///
    /// [`BindingSet::mark_endpoint_changed`]: crate::BindingSet::mark_endpoint_changed
    #[must_use]
    pub const fn skipped_missing_source(self) -> usize {
        self.skipped_missing_source
    }

    /// Returns the union of application channels affected by binding target writes.
    #[must_use]
    pub const fn affected_channels(self) -> ChannelSet {
        self.affected_channels
    }

    pub(crate) fn record(&mut self, write: BindingWrite) {
        self.evaluated_bindings += 1;
        if write.did_change() {
            self.changed_bindings += 1;
        }
        self.affected_channels |= write.affected_channels();
    }

    pub(crate) fn record_skipped_missing_source(&mut self) {
        self.skipped_missing_source += 1;
    }
}

/// Snapshot of a binding set's structural state.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct BindingStats {
    active_bindings: usize,
    binding_slots: usize,
    source_endpoints: usize,
    target_endpoints: usize,
    dependency_edges: usize,
    dirty_bindings: usize,
    has_dirty_bindings: bool,
}

impl BindingStats {
    #[must_use]
    pub(crate) const fn new(
        active_bindings: usize,
        binding_slots: usize,
        source_endpoints: usize,
        target_endpoints: usize,
        dependency_edges: usize,
        dirty_bindings: usize,
        has_dirty_bindings: bool,
    ) -> Self {
        Self {
            active_bindings,
            binding_slots,
            source_endpoints,
            target_endpoints,
            dependency_edges,
            dirty_bindings,
            has_dirty_bindings,
        }
    }

    /// Returns the number of active bindings.
    #[must_use]
    pub const fn active_bindings(self) -> usize {
        self.active_bindings
    }

    /// Returns the number of allocated binding id slots.
    ///
    /// This can be larger than [`Self::active_bindings`] after bindings are
    /// removed because ids are stable and are not reused.
    #[must_use]
    pub const fn binding_slots(self) -> usize {
        self.binding_slots
    }

    /// Returns the number of source endpoints with active bindings.
    #[must_use]
    pub const fn source_endpoints(self) -> usize {
        self.source_endpoints
    }

    /// Returns the number of target endpoints with active bindings.
    #[must_use]
    pub const fn target_endpoints(self) -> usize {
        self.target_endpoints
    }

    /// Returns the number of live binding dependency edges.
    #[must_use]
    pub const fn dependency_edges(self) -> usize {
        self.dependency_edges
    }

    /// Returns the number of dirty bindings waiting to drain.
    #[must_use]
    pub const fn dirty_bindings(self) -> usize {
        self.dirty_bindings
    }

    /// Returns `true` when dirty bindings are waiting to be drained.
    #[must_use]
    pub const fn has_dirty_bindings(self) -> bool {
        self.has_dirty_bindings
    }
}
