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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
/// persisted data should be normalized before it becomes a live [`TileTree`].
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RestoreOptions {
    /// Request repair of references to missing panes.
    ///
    /// The current API does not yet accept a known-pane inventory, so this
    /// option currently triggers structural repair only.
    pub repair_missing_panes: bool,
    /// Request removal of panes unknown to the embedding layer.
    ///
    /// The current API does not yet accept a known-pane inventory, so this
    /// option currently triggers structural repair only.
    pub drop_unknown_panes: bool,
    /// Normalize the restored tree.
    pub normalize: bool,
}

/// Report returned by repair operations.
///
/// Returned by [`TileTree::repair`]. Use this after loading host-edited or
/// persisted layout data when callers need to know which nodes, tab indices, or
/// split shares were repaired.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RepairReport {
    /// Repair actions performed.
    pub actions: Vec<RepairAction>,
}

/// A single repair action.
///
/// Values of this enum appear in [`RepairReport::actions`] to explain what was
/// changed while making a persisted or host-edited tree usable.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RepairAction {
    /// Removed an invalid node.
    RemovedInvalidNode(TileId),
    /// Removed a missing pane.
    ///
    /// Reserved for restore paths that can compare snapshots against a
    /// caller-provided pane inventory.
    RemovedMissingPane(PaneId),
    /// Repaired a tab group's active index.
    RepairedActiveTab(TileId),
    /// Repaired invalid split shares.
    RepairedShares(TileId),
    /// Collapsed a split.
    CollapsedSplit(TileId),
}

/// Restores a layout snapshot.
///
/// Pass saved data here to recover a live [`TileTree`]. When any
/// [`RestoreOptions`] repair or normalization flag is enabled, restore uses the
/// same structural repair path as [`TileTree::repair`]. This function returns
/// only the repaired tree, so call [`TileTree::repair`] on
/// [`LayoutSnapshot::tree`] before restore if the caller also needs a
/// [`RepairReport`].
pub fn restore_snapshot(
    snapshot: LayoutSnapshot,
    options: RestoreOptions,
) -> Result<TileTree, TileError> {
    let mut tree = snapshot.tree;
    if options.normalize || options.repair_missing_panes || options.drop_unknown_panes {
        let _ = tree.repair();
    }
    Ok(tree)
}
