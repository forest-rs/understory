// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#[cfg(feature = "serde")]
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::tree::RawTileTree;
use crate::{PaneId, TileError, TileId, TileTree};

/// Saved layout snapshot.
///
/// Construct this when persisting a layout, then pass it to
/// [`restore_snapshot`] or [`TileTree::restore`] when loading saved state. The
/// tree is repaired before restoration returns.
#[derive(Clone, Debug)]
pub struct LayoutSnapshot {
    payload: SnapshotPayload,
}

#[derive(Clone, Debug)]
enum SnapshotPayload {
    V1(RawTileTree),
    #[cfg_attr(
        not(any(feature = "serde", test)),
        allow(
            dead_code,
            reason = "unsupported versions are constructed only by the serde wire decoder"
        )
    )]
    Unsupported(u64),
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
            SnapshotPayload::Unsupported(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn unsupported_for_test(version: u64) -> Self {
        Self {
            payload: SnapshotPayload::Unsupported(version),
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for LayoutSnapshot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;

        let mut state = serializer.serialize_map(Some(1))?;
        match &self.payload {
            SnapshotPayload::V1(tree) => {
                state.serialize_entry("1", tree)?;
            }
            SnapshotPayload::Unsupported(version) => {
                state.serialize_entry(&version.to_string(), &())?;
            }
        }
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for LayoutSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use core::fmt;
        use serde::de::{IgnoredAny, MapAccess, Visitor};

        struct SnapshotVisitor;

        impl<'de> Visitor<'de> for SnapshotVisitor {
            type Value = LayoutSnapshot;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a versioned layout snapshot")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let version = map
                    .next_key::<String>()?
                    .ok_or_else(|| serde::de::Error::custom("snapshot must contain one version"))?;
                let payload = if version == "1" {
                    SnapshotPayload::V1(map.next_value::<RawTileTree>()?)
                } else {
                    let version = version.parse::<u64>().map_err(|_| {
                        serde::de::Error::custom("snapshot version must be an unsigned integer")
                    })?;
                    let _ = map.next_value::<IgnoredAny>()?;
                    SnapshotPayload::Unsupported(version)
                };
                if map.next_key::<IgnoredAny>()?.is_some() {
                    return Err(serde::de::Error::custom(
                        "snapshot must contain exactly one version",
                    ));
                }
                Ok(LayoutSnapshot { payload })
            }
        }

        deserializer.deserialize_map(SnapshotVisitor)
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
    let raw_tree = match snapshot.payload {
        SnapshotPayload::V1(raw_tree) => raw_tree,
        SnapshotPayload::Unsupported(version) => {
            return Err(TileError::UnsupportedSnapshotVersion(version));
        }
    };
    let mut tree = raw_tree.into_tree();
    let repairs = tree.repair();
    if tree.panes().next().is_none() {
        return Err(TileError::InvalidTree);
    }
    Ok(RestoreOutcome { tree, repairs })
}
