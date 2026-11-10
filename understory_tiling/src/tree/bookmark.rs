// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;

use crate::placement::{Neighbor, SplitBookmark, TabBookmark};
use crate::util::{is_valid_split_fraction, positive_share, repaired_shares};
use crate::{
    DockTarget, PaneBookmark, PaneId, Placement, ReopenPlacement, TabNode, TileError, TileId,
    TileNode, TileOp,
};

use super::{PaneLocation, TileTree, aligned_constraints, compact_constraints};

impl PaneLocation {
    const fn tile(self) -> TileId {
        match self {
            Self::Tile { tile } | Self::InTabs { group: tile, .. } => tile,
        }
    }
}

impl TileTree {
    /// Closes a pane and captures where to reopen it later.
    ///
    /// The bookmark records semantic neighbors and a relative split share.
    /// Closing the last pane returns [`TileError::CannotCloseLastPane`].
    pub fn close_pane_with_bookmark(&mut self, pane: PaneId) -> Result<PaneBookmark, TileError> {
        let location = self.find_pane(pane).ok_or(TileError::InvalidPaneId)?;
        let tile = location.tile();
        let tabs = match location {
            PaneLocation::InTabs { group, index } => {
                let Some(TileNode::Tabs(tabs)) = self.get_node(group) else {
                    unreachable!()
                };
                Some(TabBookmark {
                    group,
                    index,
                    placement: tabs.placement,
                    next: tabs.panes.get(index + 1).copied(),
                    previous: index.checked_sub(1).map(|index| tabs.panes[index]),
                })
            }
            PaneLocation::Tile { .. } => None,
        };
        let bookmark = PaneBookmark {
            pane,
            tabs,
            split: self.split_bookmark(tile),
            collapsed_extent: self.collapsed_extent(tile),
        };
        self.apply(TileOp::ClosePane { pane })?;
        Ok(bookmark)
    }

    /// Reopens a pane using its bookmark in the current tree.
    ///
    /// Try the original tab group, then a group containing a saved tab
    /// neighbor, then the original split or a surviving split neighbor.
    /// If none survive, dock beside the root. Within a container, prefer
    /// the next neighbor, then the previous one, then the saved index.
    /// Reopening into a tab group activates the pane.
    pub fn reopen_pane(&mut self, bookmark: &PaneBookmark) -> Result<ReopenPlacement, TileError> {
        if self.contains_pane(bookmark.pane) {
            return Err(TileError::DuplicatePane(bookmark.pane));
        }
        let mut next = self.clone();
        let placement = next.reopen_inner(bookmark)?;
        next.finish_edit()?;
        *self = next;
        Ok(placement)
    }

    fn split_bookmark(&self, tile: TileId) -> Option<SplitBookmark> {
        let (parent, index) = self.find_parent(tile)?;
        let TileNode::Split(split) = self.get_node(parent)? else {
            return None;
        };
        let shares = repaired_shares(split.children.len(), &split.shares);
        let ratio = positive_share(shares[index] / shares.iter().sum::<f64>())
            .min(f64::from_bits(1.0_f64.to_bits() - 1));
        Some(SplitBookmark {
            parent,
            index,
            axis: split.axis,
            binary: split.children.len() == 2,
            ratio,
            minimum: split
                .constraints
                .min_major
                .get(index)
                .copied()
                .unwrap_or(0.0),
            next: split
                .children
                .get(index + 1)
                .and_then(|tile| self.bookmark_neighbor(*tile)),
            previous: index
                .checked_sub(1)
                .and_then(|index| self.bookmark_neighbor(split.children[index])),
        })
    }

    fn bookmark_neighbor(&self, tile: TileId) -> Option<Neighbor> {
        let mut id = tile;
        loop {
            let pane = match self.get_node(id)? {
                TileNode::Split(split) => {
                    id = *split.children.first()?;
                    continue;
                }
                TileNode::Pane(pane) => pane.pane,
                TileNode::Tabs(tabs) => *tabs.panes.first()?,
            };
            return Some(Neighbor { tile, pane });
        }
    }

    fn resolve_neighbor(&self, neighbor: Neighbor) -> Option<(TileId, bool)> {
        let current = self.find_pane(neighbor.pane)?.tile();
        let mut ancestor = current;
        loop {
            if ancestor == neighbor.tile {
                return Some((ancestor, true));
            }
            let Some((parent, _)) = self.find_parent(ancestor) else {
                return Some((current, false));
            };
            ancestor = parent;
        }
    }

    fn tab_reopen_target(&self, saved: &TabBookmark) -> Option<(TileId, usize, ReopenPlacement)> {
        let neighbors = [
            (saved.next, Placement::Before),
            (saved.previous, Placement::After),
        ];
        let mut gaps = neighbors.into_iter().filter_map(|(pane, side)| {
            let PaneLocation::InTabs { group, index } = self.find_pane(pane?)? else {
                return None;
            };
            Some((group, index + usize::from(side == Placement::After)))
        });
        if matches!(self.get_node(saved.group), Some(TileNode::Tabs(_))) {
            return Some(gaps.find(|(group, _)| *group == saved.group).map_or(
                (saved.group, saved.index, ReopenPlacement::Approximate),
                |(group, index)| (group, index, ReopenPlacement::Exact),
            ));
        }
        gaps.next()
            .map(|(group, index)| (group, index, ReopenPlacement::Approximate))
    }

    fn reopen_inner(&mut self, bookmark: &PaneBookmark) -> Result<ReopenPlacement, TileError> {
        if let Some(tabs) = &bookmark.tabs
            && let Some((group, index, accuracy)) = self.tab_reopen_target(tabs)
        {
            self.insert_tab_pane(group, bookmark.pane, Some(index))?;
            return Ok(accuracy);
        }

        let node = bookmark.tabs.as_ref().map_or_else(
            || TileNode::pane(bookmark.pane),
            |tabs| {
                TileNode::Tabs(TabNode {
                    panes: vec![bookmark.pane],
                    active: 0,
                    placement: tabs.placement,
                })
            },
        );
        let tile = self.push_node(node);
        let accuracy = if let Some(split) = &bookmark.split {
            self.reopen_split_slot(tile, split)?
        } else {
            self.insert_tile_near(
                self.root,
                DockTarget::ROOT_AXIS,
                tile,
                DockTarget::ROOT_PLACEMENT,
                DockTarget::ROOT_RATIO,
            )?;
            ReopenPlacement::Approximate
        };
        self.set_collapsed(tile, bookmark.collapsed_extent)?;
        Ok(accuracy)
    }

    fn reopen_split_slot(
        &mut self,
        tile: TileId,
        saved: &SplitBookmark,
    ) -> Result<ReopenPlacement, TileError> {
        if !is_valid_split_fraction(saved.ratio)
            || !saved.minimum.is_finite()
            || saved.minimum < 0.0
        {
            return Err(TileError::InvalidTarget);
        }
        let neighbors = [
            (saved.next, Placement::Before),
            (saved.previous, Placement::After),
        ];
        let mut anchors = neighbors.into_iter().filter_map(|(neighbor, side)| {
            let (tile, original) = self.resolve_neighbor(neighbor?)?;
            Some((tile, side, original))
        });
        if let Some(TileNode::Split(split)) = self.get_node(saved.parent)
            && split.axis == saved.axis
        {
            let gap = anchors.find_map(|(anchor, side, original)| {
                let index = split.children.iter().position(|child| *child == anchor)?;
                Some((index + usize::from(side == Placement::After), original))
            });
            let (index, original) = gap.unwrap_or((saved.index.min(split.children.len()), false));
            let Some(TileNode::Split(split)) = self.get_node_mut(saved.parent) else {
                unreachable!()
            };
            split.shares = repaired_shares(split.children.len(), &split.shares);
            let total = split.shares.iter().sum::<f64>();
            for share in &mut split.shares {
                *share = positive_share((*share / total) * (1.0 - saved.ratio));
            }
            let mut minimums = aligned_constraints(split.children.len(), &split.constraints);
            minimums.insert(index, saved.minimum);
            split.constraints.min_major = compact_constraints(minimums);
            split.children.insert(index, tile);
            split.shares.insert(index, saved.ratio);
            return Ok(if original {
                ReopenPlacement::Exact
            } else {
                ReopenPlacement::Approximate
            });
        }

        let (anchor, side, original) =
            anchors
                .next()
                .unwrap_or((self.root, DockTarget::ROOT_PLACEMENT, false));
        let recreate_split = self.find_parent(anchor).is_none_or(|(parent, _)| {
            !matches!(self.get_node(parent), Some(TileNode::Split(split)) if split.axis == saved.axis)
        });
        self.insert_tile_near(anchor, saved.axis, tile, side, saved.ratio)?;
        if saved.minimum > 0.0 {
            let (parent, index) = self.find_parent(tile).ok_or(TileError::InvalidOperation)?;
            let Some(TileNode::Split(split)) = self.get_node_mut(parent) else {
                unreachable!()
            };
            let mut minimums = aligned_constraints(split.children.len(), &split.constraints);
            minimums[index] = saved.minimum;
            split.constraints.min_major = compact_constraints(minimums);
        }
        Ok(if original && saved.binary && recreate_split {
            ReopenPlacement::Exact
        } else {
            ReopenPlacement::Approximate
        })
    }
}
