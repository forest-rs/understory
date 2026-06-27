// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use crate::{Axis, LayoutSnapshot, PaneId, Placement, Rect, TileId};

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
    /// Root-level target.
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
    /// Replace a target tile.
    Replace {
        /// Target tile.
        tile: TileId,
    },
    /// Future floating target.
    Float {
        /// Requested floating bounds.
        bounds: Rect,
    },
}

/// Semantic mutation applied to a [`TileTree`](crate::TileTree).
///
/// Construct one and pass it to [`TileTree::apply`](crate::TileTree::apply) for
/// command-driven changes. Interaction commit helpers such as
/// [`commit_drag`](crate::commit_drag) and
/// [`commit_resize`](crate::commit_resize) also return the operation they
/// applied so callers can log, undo, or mirror the semantic change.
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
    /// Reorder a tab inside a group.
    ReorderTab {
        /// Target tab group.
        group: TileId,
        /// Pane tab to move.
        pane: PaneId,
        /// New tab index.
        index: usize,
    },
    /// Resize a split by moving one handle.
    ResizeSplit {
        /// Split tile.
        split: TileId,
        /// Handle index.
        handle: usize,
        /// Pointer delta along the split axis.
        ///
        /// Expected to be finite.
        delta: f64,
    },
    /// Set split shares directly.
    SetSplitShares {
        /// Split tile.
        split: TileId,
        /// Replacement shares.
        shares: Vec<f64>,
    },
    /// Future operation for floating a pane.
    FloatPane {
        /// Pane to float.
        pane: PaneId,
        /// Requested floating bounds.
        bounds: Rect,
    },
    /// Restore a saved layout snapshot.
    RestoreLayout {
        /// Snapshot to restore.
        snapshot: LayoutSnapshot,
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
    /// The operation is structurally invalid.
    InvalidOperation,
    /// The target cannot accept the requested operation.
    InvalidTarget,
    /// The interaction was based on an old tree revision.
    StaleInteraction,
    /// Policy data rejected the operation.
    PolicyRejected,
    /// The operation would leave no panes.
    EmptyTree,
    /// The operation tried to close the last pane.
    CannotCloseLastPane,
    /// The operation is reserved but not implemented in this slice.
    Unsupported,
}
