// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Changes to realized results, separate from source edits and derive work.

use alloc::vec::Vec;

use crate::{EdgeId, NodeId, PortId, Revision};

/// Actual changes to one computed geometry table.
///
/// An updated id is newly realized or has a different value. Read its current
/// value from [`GraphComputed`](crate::GraphComputed). A removed id no longer
/// has a realized value, even if it still exists in the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeometryChanges<I> {
    /// Newly realized or changed geometry, without duplicates.
    pub updated: Vec<I>,
    /// Geometry that ceased to exist, without duplicates.
    pub removed: Vec<I>,
}

impl<I> Default for GeometryChanges<I> {
    fn default() -> Self {
        Self {
            updated: Vec::new(),
            removed: Vec::new(),
        }
    }
}

impl<I> GeometryChanges<I> {
    /// Whether this table's realized values were unchanged.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.updated.is_empty() && self.removed.is_empty()
    }
}

/// Changes in viewport membership, independent of geometry changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibilityChanges<I> {
    /// Entities entering the visible set.
    pub entered: Vec<I>,
    /// Entities leaving the visible set, including deleted entities.
    pub exited: Vec<I>,
}

impl<I> Default for VisibilityChanges<I> {
    fn default() -> Self {
        Self {
            entered: Vec::new(),
            exited: Vec::new(),
        }
    }
}

impl<I> VisibilityChanges<I> {
    /// Whether visible membership was unchanged.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entered.is_empty() && self.exited.is_empty()
    }
}

/// Owned result delta from one computed rebuild.
///
/// Apply reports in order to a host mapping bound to the same
/// [`GraphComputed`](crate::GraphComputed) instance. `before` must match that
/// mapping's last applied revision unless `reset` is set; otherwise resynchronize from the computed
/// snapshot. These revisions are local to that cache, not global identities.
/// Reports contain ids rather than copied geometry and must be consumed before
/// another rebuild replaces the values they describe. No result history is kept.
///
/// A `reset` starts a new baseline: discard the old host mapping, then apply the
/// report. Its updates and visibility entries describe all current results.
/// This happens after cache reset, on the initial rebuild, or when replacing the
/// document instance, or after an interrupted rebuild. It prevents coincidentally equal document-local handles
/// from retaining artwork or controls belonging to a different document.
///
/// Only world-space geometry, visible membership and the edge preview are
/// covered. Hosts own metadata, selection styling and viewport transforms. A
/// camera change can require repainting without changing any world geometry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComputedChanges {
    /// Computed revision before this call.
    pub before: Revision,
    /// Computed revision after this call.
    pub after: Revision,
    /// Whether the host must discard its previous result mapping.
    pub reset: bool,
    /// Changes to realized node bounds.
    pub nodes: GeometryChanges<NodeId>,
    /// Changes to realized world-space socket anchors.
    pub ports: GeometryChanges<PortId>,
    /// Changes to committed edge routes, including hidden or removed routes.
    pub edges: GeometryChanges<EdgeId>,
    /// Visible node membership changes.
    pub visible_nodes: VisibilityChanges<NodeId>,
    /// Visible edge membership changes.
    pub visible_edges: VisibilityChanges<EdgeId>,
    /// Whether preview presence, endpoints, compatibility or route changed.
    pub preview_changed: bool,
}

impl ComputedChanges {
    /// Whether any derive phase ran, even if all realized results stayed equal.
    #[must_use]
    pub fn rebuilt(&self) -> bool {
        self.before != self.after
    }

    /// Whether no host-visible computed results changed.
    ///
    /// A rebuild can return `true` from [`Self::rebuilt`] and still be empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.reset
            && self.nodes.is_empty()
            && self.ports.is_empty()
            && self.edges.is_empty()
            && self.visible_nodes.is_empty()
            && self.visible_edges.is_empty()
            && !self.preview_changed
    }
}
