// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::fmt;

use crate::{Axis, PaneId, Placement, TileId};

/// Target for a dock or move operation.
///
/// Construct this when building [`TileOp::MovePane`] directly, or read it from
/// [`DropTargetFrame`](crate::DropTargetFrame) and
/// [`DockProposal`](crate::DockProposal) during drag/drop. It describes the
/// semantic destination; layout code later turns the committed tree into
/// geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DockTarget {
    /// Dock after the root in a horizontal split with an equal share.
    Root,
    /// Split relative to an existing tile.
    Split {
        /// Target tile.
        tile: TileId,
        /// Split axis.
        axis: Axis,
        /// Placement relative to the target tile.
        placement: Placement,
        /// Share assigned to the moved or inserted pane.
        ///
        /// Expected to be finite and strictly between `0.0` and `1.0`.
        ratio: f64,
    },
    /// Insert as a tab in an existing group.
    TabInto {
        /// Target tab group.
        group: TileId,
        /// Optional insertion index.
        index: Option<usize>,
    },
}

impl DockTarget {
    pub(crate) const ROOT_AXIS: Axis = Axis::Horizontal;
    pub(crate) const ROOT_PLACEMENT: Placement = Placement::After;
    pub(crate) const ROOT_RATIO: f64 = 0.5;
}

/// Semantic mutation applied to a [`TileTree`](crate::TileTree).
///
/// Construct one and pass it to [`TileTree::apply`](crate::TileTree::apply) for
/// command-driven changes. The interaction path validates proposals with
/// [`validate_interaction_update`](crate::validate_interaction_update) and
/// commits them with [`commit_proposal`](crate::commit_proposal), which returns
/// the operation applied so callers can log, undo, or mirror the semantic
/// change.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TileOp {
    /// Activate a pane within its tab group.
    ActivatePane {
        /// Pane to activate.
        pane: PaneId,
    },
    /// Close a pane.
    ClosePane {
        /// Pane to close.
        pane: PaneId,
    },
    /// Split around a pane or its containing tab group.
    SplitPane {
        /// Existing pane used as the split target.
        pane: PaneId,
        /// Split axis.
        axis: Axis,
        /// New pane to insert.
        new_pane: PaneId,
        /// Placement relative to the target.
        placement: Placement,
        /// Share assigned to the new pane.
        ///
        /// Expected to be finite and strictly between `0.0` and `1.0`.
        share: f64,
    },
    /// Move a pane to a dock target.
    MovePane {
        /// Pane to move.
        pane: PaneId,
        /// Move target.
        target: DockTarget,
    },
    /// Move a tab group to a dock target.
    MoveTabGroup {
        /// Tab group to move.
        group: TileId,
        /// Move target.
        target: DockTarget,
    },
    /// Reorder a tab inside a group.
    ReorderTab {
        /// Target tab group.
        group: TileId,
        /// Pane tab to move.
        pane: PaneId,
        /// Final tab index, after removing the source tab.
        index: usize,
    },
    /// Set split shares directly.
    SetSplitShares {
        /// Split tile.
        split: TileId,
        /// Replacement shares.
        shares: Vec<f64>,
    },
    /// Collapse or expand a pane or tab group in its existing split slot.
    ///
    /// The expanded share and active tab remain in the tree. Pointer resize
    /// is suspended on the containing split while any child is collapsed.
    /// A tile promoted to the root by another edit expands automatically.
    SetCollapsed {
        /// Pane or tab group tile. The root cannot be collapsed.
        tile: TileId,
        /// Finite non-negative major-axis strip extent, or `None` to expand.
        extent: Option<f64>,
    },
}

/// Error returned by mutation and commit APIs.
///
/// Returned from [`TileTree::apply`](crate::TileTree::apply), interaction
/// commits, proposal validation, and snapshot restore when an id, target,
/// policy, or interaction revision is invalid.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TileError {
    /// The operation referenced a missing tile.
    InvalidTileId,
    /// The operation referenced a missing pane.
    InvalidPaneId,
    /// The operation tried to introduce an existing pane id again.
    DuplicatePane(PaneId),
    /// The operation is structurally invalid.
    InvalidOperation,
    /// The target cannot accept the requested operation.
    InvalidTarget,
    /// The interaction was based on an old tree revision.
    StaleInteraction,
    /// Policy data rejected the operation.
    PolicyRejected,
    /// The operation tried to close the last pane.
    CannotCloseLastPane,
    /// A persisted or host-edited tree could not be made valid.
    InvalidTree,
    /// The monotonic tree revision cannot represent another mutation.
    RevisionExhausted,
    /// A snapshot uses an unsupported schema version.
    UnsupportedSnapshotVersion(u64),
}

impl fmt::Display for TileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTileId => f.write_str("invalid tile id"),
            Self::InvalidPaneId => f.write_str("invalid pane id"),
            Self::DuplicatePane(pane) => write!(f, "duplicate {pane}"),
            Self::InvalidOperation => f.write_str("invalid operation"),
            Self::InvalidTarget => f.write_str("invalid target"),
            Self::StaleInteraction => f.write_str("stale interaction"),
            Self::PolicyRejected => f.write_str("policy rejected operation"),
            Self::CannotCloseLastPane => f.write_str("cannot close last pane"),
            Self::InvalidTree => f.write_str("invalid tile tree"),
            Self::RevisionExhausted => f.write_str("tile tree revision exhausted"),
            Self::UnsupportedSnapshotVersion(version) => {
                write!(f, "unsupported layout snapshot version {version}")
            }
        }
    }
}
