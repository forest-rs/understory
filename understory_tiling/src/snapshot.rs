// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use crate::tree::RawTileTree;
use crate::{PaneId, TileError, TileId, TileTree};

/// Saved layout snapshot.
///
/// Construct this when persisting a layout, then pass it to
/// [`restore_snapshot`] or [`TileOp::RestoreLayout`](crate::TileOp::RestoreLayout)
/// when loading saved state. The tree is repaired before restoration returns.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct LayoutSnapshot {
    payload: SnapshotPayload,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "schema_version", content = "layout"))]
enum SnapshotPayload {
    #[cfg_attr(feature = "serde", serde(rename = "1"))]
    V1(RawTileTree),
    #[cfg_attr(feature = "serde", serde(other))]
    #[cfg_attr(not(any(feature = "serde", test)), allow(dead_code))]
    Unsupported,
}

impl LayoutSnapshot {
    /// Captures a valid semantic tree for persistence or later restoration.
    #[must_use]
    pub fn new(tree: TileTree) -> Self {
        Self {
            payload: SnapshotPayload::V1(tree.into()),
        }
    }

    /// Returns the supported persisted schema version, or `None` for an
    /// unsupported version decoded from the wire.
    #[must_use]
    pub const fn schema_version(&self) -> Option<u16> {
        match self.payload {
            SnapshotPayload::V1(_) => Some(1),
            SnapshotPayload::Unsupported => None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn unsupported_for_test() -> Self {
        Self {
            payload: SnapshotPayload::Unsupported,
        }
    }
}

pub(crate) type RepairReport = Vec<RepairAction>;

/// One repair performed while restoring tree invariants.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RepairAction {
    /// Removed an invalid node.
    RemovedInvalidNode(TileId),
    /// Repaired a tab group's active index.
    RepairedActiveTab(TileId),
    /// Repaired invalid split shares.
    RepairedShares(TileId),
    /// Repaired or realigned per-child split constraints.
    RepairedConstraints(TileId),
    /// Collapsed a split.
    CollapsedSplit(TileId),
    /// Removed a pane after its first semantic occurrence.
    RemovedDuplicatePane {
        /// Duplicate pane id.
        pane: PaneId,
        /// Tile from which the duplicate occurrence was removed.
        tile: TileId,
    },
}

/// Result of restoring persisted layout state.
///
/// `tree` always satisfies live-tree invariants. `repairs` records every
/// deterministic change made while earning those invariants.
#[derive(Clone, Debug)]
pub struct RestoreOutcome {
    /// Valid restored semantic tree.
    pub tree: TileTree,
    /// Repairs applied to persisted structure, in deterministic order.
    pub repairs: Vec<RepairAction>,
}

/// Restores a layout snapshot.
///
/// Pass saved data here to recover a live [`TileTree`]. Restoration always
/// repairs persisted structure before returning it and reports those repairs;
/// an irreparable tree with no semantic panes is rejected.
pub fn restore_snapshot(snapshot: LayoutSnapshot) -> Result<RestoreOutcome, TileError> {
    let SnapshotPayload::V1(raw_tree) = snapshot.payload else {
        return Err(TileError::UnsupportedSnapshotVersion);
    };
    let mut tree = raw_tree.into_tree();
    let repairs = tree.repair();
    if tree.panes().next().is_none() {
        return Err(TileError::InvalidTree);
    }
    Ok(RestoreOutcome { tree, repairs })
}
