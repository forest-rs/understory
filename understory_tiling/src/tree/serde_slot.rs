// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::borrow::Cow;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{PaneNode, SplitNode, TabNode, TileNode};

use super::NodeSlot;

#[derive(Serialize, Deserialize)]
#[serde(rename = "NodeSlot")]
struct StoredSlot<'a> {
    node: Option<StoredTile<'a>>,
}

// Keep the original TileNode variant order and payloads for expanded tiles.
// Additional variants keep the slot shape fixed for sequence-based formats.
#[derive(Serialize, Deserialize)]
#[serde(rename = "TileNode")]
enum StoredTile<'a> {
    Split(Cow<'a, SplitNode>),
    Tabs(Cow<'a, TabNode>),
    Pane(Cow<'a, PaneNode>),
    CollapsedTabs {
        tabs: Cow<'a, TabNode>,
        extent: f64,
    },
    CollapsedPane {
        pane: Cow<'a, PaneNode>,
        extent: f64,
    },
}

impl Serialize for NodeSlot {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let node = self
            .node
            .as_ref()
            .map(|node| match (node, self.collapsed_extent) {
                (TileNode::Split(split), _) => StoredTile::Split(Cow::Borrowed(split)),
                (TileNode::Tabs(tabs), Some(extent)) => StoredTile::CollapsedTabs {
                    tabs: Cow::Borrowed(tabs),
                    extent,
                },
                (TileNode::Tabs(tabs), None) => StoredTile::Tabs(Cow::Borrowed(tabs)),
                (TileNode::Pane(pane), Some(extent)) => StoredTile::CollapsedPane {
                    pane: Cow::Borrowed(pane),
                    extent,
                },
                (TileNode::Pane(pane), None) => StoredTile::Pane(Cow::Borrowed(pane)),
            });
        StoredSlot { node }.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NodeSlot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let StoredSlot { node } = StoredSlot::deserialize(deserializer)?;
        let (node, collapsed_extent) = match node {
            None => (None, None),
            Some(StoredTile::Split(split)) => (Some(TileNode::Split(split.into_owned())), None),
            Some(StoredTile::Tabs(tabs)) => (Some(TileNode::Tabs(tabs.into_owned())), None),
            Some(StoredTile::Pane(pane)) => (Some(TileNode::Pane(pane.into_owned())), None),
            Some(StoredTile::CollapsedTabs { tabs, extent }) => {
                (Some(TileNode::Tabs(tabs.into_owned())), Some(extent))
            }
            Some(StoredTile::CollapsedPane { pane, extent }) => {
                (Some(TileNode::Pane(pane.into_owned())), Some(extent))
            }
        };
        Ok(Self {
            node,
            collapsed_extent,
        })
    }
}
