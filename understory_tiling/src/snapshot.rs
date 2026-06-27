// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use crate::{PaneId, TileError, TileId, TileTree};

/// Saved layout snapshot.
///
/// Construct this when persisting a layout, then pass it to
/// [`restore_snapshot`] or [`TileOp::RestoreLayout`](crate::TileOp::RestoreLayout)
/// when loading saved state. The tree is normalized during restore when
/// requested by [`RestoreOptions`].
#[derive(Clone, Debug)]
pub struct LayoutSnapshot {
    /// Snapshot schema version.
    pub schema_version: u16,
    /// Saved tree.
    pub tree: TileTree,
    /// Active pane known by the embedding layer.
    pub active_pane: Option<PaneId>,
    /// Pane ids that were closed by the layout layer.
    pub closed_panes: Vec<PaneId>,
}

/// Restore behavior for saved snapshots.
///
/// Construct this and pass it to [`restore_snapshot`] to choose how aggressively
/// persisted data should be normalized or repaired before it becomes a live
/// [`TileTree`].
#[derive(Clone, Copy, Debug)]
pub struct RestoreOptions {
    /// Repair references to missing panes when possible.
    pub repair_missing_panes: bool,
    /// Drop unknown panes when the embedding layer reports them.
    pub drop_unknown_panes: bool,
    /// Normalize the restored tree.
    pub normalize: bool,
}

/// Report returned by repair operations.
///
/// Returned by [`TileTree::repair`]. The MVP currently normalizes the tree and
/// leaves detailed actions mostly reserved for future repair reporting.
#[derive(Clone, Debug, Default)]
pub struct RepairReport {
    /// Repair actions performed.
    pub actions: Vec<RepairAction>,
}

/// A single repair action.
///
/// Values of this enum appear in [`RepairReport::actions`] to explain what was
/// changed while making a persisted or host-edited tree usable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepairAction {
    /// Removed an invalid node.
    RemovedInvalidNode(TileId),
    /// Removed a missing pane.
    RemovedMissingPane(PaneId),
    /// Repaired a tab group's active index.
    RepairedActiveTab(TileId),
    /// Repaired invalid split shares.
    RepairedShares(TileId),
    /// Collapsed a split.
    CollapsedSplit(TileId),
}

/// Restores a layout snapshot.
pub fn restore_snapshot(
    snapshot: LayoutSnapshot,
    options: RestoreOptions,
) -> Result<TileTree, TileError> {
    let mut tree = snapshot.tree;
    if options.normalize || options.repair_missing_panes || options.drop_unknown_panes {
        tree.normalize();
    }
    Ok(tree)
}
