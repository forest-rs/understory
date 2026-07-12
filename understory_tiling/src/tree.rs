// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;
use alloc::vec::Vec;

use crate::frame::{HIT_Z_PANE, HIT_Z_SPLIT_HANDLE, HIT_Z_TAB, HIT_Z_TAB_BAR};
use crate::snapshot::{RepairAction, RepairReport};
use crate::util::{
    is_valid_split_fraction, repaired_shares, solve_lengths_with_min, split_min_major,
    split_tab_bar, tab_rects,
};
use crate::{
    Axis, DockTarget, FrameItemId, HitKind, HitRegion, LayoutFrame, LayoutInput, PaneFrame, PaneId,
    Placement, Rect, Revision, SplitChildFrame, SplitConstraints, SplitHandleFrame, SplitNode,
    TabBarFrame, TabBarPlacement, TabFrame, TabNode, TileError, TileId, TileNode, TileOp, TileSpec,
    TileSpecError,
};

/// Persistent semantic tree of splits, tab groups, and pane leaves.
///
/// Create one with [`TileTree::from_spec`] or [`TileTree::single_pane`], mutate
/// it with [`TileTree::apply`], and call [`TileTree::layout`] whenever the host
/// needs a fresh [`LayoutFrame`] for rendering, hit testing, or interactions.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct TileTree {
    root: TileId,
    revision: Revision,
    nodes: Vec<NodeSlot>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct NodeSlot {
    node: Option<TileNode>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct RawTileTree {
    pub(crate) root: TileId,
    pub(crate) revision: Revision,
    nodes: Vec<NodeSlot>,
}

impl From<TileTree> for RawTileTree {
    fn from(tree: TileTree) -> Self {
        Self {
            root: tree.root,
            revision: tree.revision,
            nodes: tree.nodes,
        }
    }
}

impl RawTileTree {
    pub(crate) fn into_tree(self) -> TileTree {
        TileTree {
            root: self.root,
            revision: self.revision,
            nodes: self.nodes,
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TileTree {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut tree = RawTileTree::deserialize(deserializer)?.into_tree();
        let _ = tree.repair();
        if tree.panes().next().is_none() {
            return Err(serde::de::Error::custom(
                "tile tree contains no semantic panes",
            ));
        }
        Ok(tree)
    }
}

struct RepairSink<'a> {
    actions: Option<&'a mut Vec<RepairAction>>,
}

#[derive(Clone, Copy, Debug)]
enum PaneLocation {
    Tile { tile: TileId },
    InTabs { group: TileId, index: usize },
}

/// Iterator over every semantic pane in a [`TileTree`].
///
/// Split children are visited depth first in stored order, and tab groups yield
/// panes in tab order, including inactive tabs. Public construction,
/// restoration, and mutation preserve the valid-tree invariant that each pane
/// and tile has exactly one semantic occurrence.
#[derive(Clone, Debug)]
pub struct Panes<'a> {
    tree: &'a TileTree,
    pending: Vec<PaneTraversal>,
}

#[derive(Clone, Copy, Debug)]
enum PaneTraversal {
    Tile(TileId),
    Pane(PaneId),
}

impl Iterator for Panes<'_> {
    type Item = PaneId;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(item) = self.pending.pop() {
            match item {
                PaneTraversal::Pane(pane) => return Some(pane),
                PaneTraversal::Tile(tile) => match self.tree.get_node(tile) {
                    Some(TileNode::Pane(pane)) => return Some(pane.pane),
                    Some(TileNode::Tabs(tabs)) => {
                        self.pending
                            .extend(tabs.panes.iter().rev().copied().map(PaneTraversal::Pane));
                    }
                    Some(TileNode::Split(split)) => {
                        self.pending.extend(
                            split
                                .children
                                .iter()
                                .rev()
                                .copied()
                                .map(PaneTraversal::Tile),
                        );
                    }
                    None => {}
                },
            }
        }
        None
    }
}

impl<'a> RepairSink<'a> {
    fn silent() -> Self {
        Self { actions: None }
    }

    fn recording(actions: &'a mut Vec<RepairAction>) -> Self {
        Self {
            actions: Some(actions),
        }
    }

    fn record(&mut self, action: RepairAction) {
        if let Some(actions) = &mut self.actions
            && !actions.contains(&action)
        {
            actions.push(action);
        }
    }
}

impl TileTree {
    /// Creates a tree containing one pane.
    #[must_use]
    pub fn single_pane(pane: PaneId) -> Self {
        Self {
            root: TileId(0),
            revision: Revision(0),
            nodes: vec![NodeSlot {
                node: Some(TileNode::pane(pane)),
            }],
        }
    }

    /// Constructs a normalized tree from an ID-free recursive specification.
    ///
    /// Validation completes before arena construction. Pane ids must be unique,
    /// tab groups must be non-empty with a valid active index, and splits must
    /// have at least two children with one finite positive share per child.
    /// Tile ids are assigned deterministically in preorder.
    pub fn from_spec(spec: TileSpec) -> Result<Self, TileSpecError> {
        spec.validate()?;
        let mut tree = Self {
            root: TileId(0),
            revision: Revision(0),
            nodes: Vec::new(),
        };
        tree.root = tree.push_spec(spec);
        tree.normalize();
        Ok(tree)
    }

    /// Iterates over every semantic pane in deterministic depth-first order.
    ///
    /// This includes inactive tabs and ordinary pane leaves. The iterator uses
    /// a traversal stack but does not collect the yielded panes.
    #[must_use]
    pub fn panes(&self) -> Panes<'_> {
        Panes {
            tree: self,
            pending: alloc::vec![PaneTraversal::Tile(self.root)],
        }
    }

    /// Returns the root tile id.
    #[must_use]
    pub const fn root(&self) -> TileId {
        self.root
    }

    /// Returns the current tree revision.
    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }

    /// Returns a tile node by id.
    #[must_use]
    pub fn node(&self, id: TileId) -> Option<&TileNode> {
        self.get_node(id)
    }

    /// Applies a semantic operation atomically.
    pub fn apply(&mut self, op: TileOp) -> Result<(), TileError> {
        let mut next = self.clone();
        let changed = next.apply_inner(op)?;
        if changed {
            next.revision.0 = next
                .revision
                .0
                .checked_add(1)
                .ok_or(TileError::RevisionExhausted)?;
            next.normalize();
            *self = next;
        }
        Ok(())
    }

    /// Normalizes the tree in place.
    pub(crate) fn normalize(&mut self) {
        let mut sink = RepairSink::silent();
        self.normalize_with_report(&mut sink);
    }

    fn normalize_with_report(&mut self, sink: &mut RepairSink<'_>) {
        if self.nodes.is_empty() {
            sink.record(RepairAction::RemovedInvalidNode(self.root));
            self.root = self.push_node(TileNode::Tabs(TabNode {
                panes: Vec::new(),
                active: 0,
                placement: TabBarPlacement::Hidden,
            }));
            return;
        }

        let mut visiting = vec![false; self.nodes.len()];
        let mut claimed = vec![false; self.nodes.len()];
        let mut panes = Vec::new();
        let root = self.normalize_node(self.root, &mut visiting, &mut claimed, &mut panes, sink);
        self.root = root.unwrap_or_else(|| {
            self.push_node(TileNode::Tabs(TabNode {
                panes: Vec::new(),
                active: 0,
                placement: TabBarPlacement::Hidden,
            }))
        });

        let mut reachable = vec![false; self.nodes.len()];
        self.mark_reachable(self.root, &mut reachable);
        for (index, slot) in self.nodes.iter_mut().enumerate() {
            if !reachable.get(index).copied().unwrap_or(false) {
                if slot.node.is_some() {
                    sink.record(RepairAction::RemovedInvalidNode(TileId(
                        u32::try_from(index).expect("tile arena exhausted"),
                    )));
                }
                slot.node = None;
            }
        }
    }

    /// Repairs a tree and reports high-level repair actions.
    pub(crate) fn repair(&mut self) -> RepairReport {
        let mut actions = Vec::new();
        let mut sink = RepairSink::recording(&mut actions);
        self.normalize_with_report(&mut sink);
        actions
    }

    /// Solves layout and returns a flattened frame.
    ///
    /// `input` scalar geometry is expected to be finite. Thicknesses and minimum
    /// pane sizes are expected to be non-negative.
    #[must_use]
    pub fn layout(&self, input: LayoutInput) -> LayoutFrame {
        debug_assert!(
            input.bounds.is_finite(),
            "LayoutInput::bounds must be finite",
        );
        debug_assert!(
            input.tab_bar_thickness.is_finite() && input.tab_bar_thickness >= 0.0,
            "LayoutInput::tab_bar_thickness must be finite and non-negative",
        );
        debug_assert!(
            input.split_handle_thickness.is_finite() && input.split_handle_thickness >= 0.0,
            "LayoutInput::split_handle_thickness must be finite and non-negative",
        );
        debug_assert!(
            input.min_pane_size.is_finite()
                && input.min_pane_size.width >= 0.0
                && input.min_pane_size.height >= 0.0,
            "LayoutInput::min_pane_size must be finite and non-negative",
        );
        let mut frame = LayoutFrame::new(self.revision);
        let mut visiting = vec![false; self.nodes.len()];
        self.layout_tile(
            self.root,
            input.bounds.abs(),
            &input,
            &mut frame,
            &mut visiting,
        );
        frame
    }

    fn apply_inner(&mut self, op: TileOp) -> Result<bool, TileError> {
        match op {
            TileOp::ActivatePane { pane } => self.activate_pane(pane),
            TileOp::ClosePane { pane } => self.close_pane(pane),
            TileOp::SplitPane {
                pane,
                axis,
                new_pane,
                placement,
                share,
            } => self.split_pane(pane, axis, new_pane, placement, share),
            TileOp::MovePane { pane, target } => self.move_pane(pane, target),
            TileOp::MoveTabGroup { group, target } => self.move_tab_group(group, target),
            TileOp::ReorderTab { group, pane, index } => self.reorder_tab(group, pane, index),
            TileOp::SetSplitShares { split, shares } => self.set_split_shares(split, shares),
            TileOp::RestoreLayout { snapshot } => {
                let revision = self.revision;
                let mut restored = crate::restore_snapshot(snapshot)?.tree;
                restored.revision = revision;
                *self = restored;
                Ok(true)
            }
        }
    }

    fn push_node(&mut self, node: TileNode) -> TileId {
        let id = TileId(u32::try_from(self.nodes.len()).expect("tile arena exhausted"));
        self.nodes.push(NodeSlot { node: Some(node) });
        id
    }

    fn push_spec(&mut self, spec: TileSpec) -> TileId {
        let id = self.push_node(TileNode::Tabs(TabNode {
            panes: Vec::new(),
            active: 0,
            placement: TabBarPlacement::Hidden,
        }));
        let node = match spec {
            TileSpec::Pane(pane) => TileNode::pane(pane),
            TileSpec::Tabs {
                panes,
                active,
                placement,
            } => TileNode::Tabs(TabNode {
                panes,
                active,
                placement,
            }),
            TileSpec::Split {
                axis,
                children,
                shares,
                constraints,
            } => {
                let children = children
                    .into_iter()
                    .map(|child| self.push_spec(child))
                    .collect();
                TileNode::Split(SplitNode {
                    axis,
                    children,
                    shares,
                    constraints,
                })
            }
        };
        self.set_node(id, node)
            .expect("newly allocated tile id must be valid");
        id
    }

    fn get_node(&self, id: TileId) -> Option<&TileNode> {
        self.nodes.get(id.0 as usize)?.node.as_ref()
    }

    fn get_node_mut(&mut self, id: TileId) -> Option<&mut TileNode> {
        self.nodes.get_mut(id.0 as usize)?.node.as_mut()
    }

    fn set_node(&mut self, id: TileId, node: TileNode) -> Result<(), TileError> {
        let slot = self
            .nodes
            .get_mut(id.0 as usize)
            .ok_or(TileError::InvalidTileId)?;
        slot.node = Some(node);
        Ok(())
    }

    fn clear_node(&mut self, id: TileId) {
        if let Some(slot) = self.nodes.get_mut(id.0 as usize) {
            slot.node = None;
        }
    }

    /// Creates a tree from raw arena parts for malformed-layout tests.
    #[cfg(test)]
    pub(crate) fn from_raw_parts_for_test(
        root: TileId,
        revision: Revision,
        nodes: Vec<Option<TileNode>>,
    ) -> Self {
        Self {
            root,
            revision,
            nodes: nodes.into_iter().map(|node| NodeSlot { node }).collect(),
        }
    }

    fn activate_pane(&mut self, pane: PaneId) -> Result<bool, TileError> {
        let location = self.find_pane(pane).ok_or(TileError::InvalidPaneId)?;
        match location {
            PaneLocation::InTabs { group, index } => {
                let Some(TileNode::Tabs(tabs)) = self.get_node_mut(group) else {
                    return Err(TileError::InvalidTileId);
                };
                if tabs.active == index {
                    return Ok(false);
                }
                tabs.active = index;
                Ok(true)
            }
            PaneLocation::Tile { .. } => Ok(false),
        }
    }

    fn close_pane(&mut self, pane: PaneId) -> Result<bool, TileError> {
        if self.count_panes() <= 1 {
            return Err(TileError::CannotCloseLastPane);
        }
        self.remove_pane(pane)?;
        Ok(true)
    }

    fn split_pane(
        &mut self,
        pane: PaneId,
        axis: Axis,
        new_pane: PaneId,
        placement: Placement,
        share: f64,
    ) -> Result<bool, TileError> {
        if !is_valid_split_fraction(share) {
            return Err(TileError::InvalidOperation);
        }
        if self.contains_pane(new_pane) {
            return Err(TileError::DuplicatePane(new_pane));
        }
        let location = self.find_pane(pane).ok_or(TileError::InvalidPaneId)?;
        let target = match location {
            PaneLocation::Tile { tile } => tile,
            PaneLocation::InTabs { group, .. } => group,
        };
        let new_tile = self.push_node(TileNode::pane(new_pane));
        self.insert_tile_near(target, axis, new_tile, placement, share)?;
        Ok(true)
    }

    fn move_pane(&mut self, pane: PaneId, target: DockTarget) -> Result<bool, TileError> {
        if !self.contains_pane(pane) {
            return Err(TileError::InvalidPaneId);
        }
        self.validate_target(target)?;
        self.remove_pane(pane)?;
        let tile = self.push_node(TileNode::pane(pane));
        self.insert_tile_at_target(tile, pane, target)?;
        Ok(true)
    }

    fn move_tab_group(&mut self, group: TileId, target: DockTarget) -> Result<bool, TileError> {
        match self.get_node(group) {
            Some(TileNode::Tabs(_)) => {}
            Some(_) => return Err(TileError::InvalidTarget),
            None => return Err(TileError::InvalidTileId),
        }
        self.validate_group_target(group, target)?;
        if group == self.root && matches!(target, DockTarget::Root) {
            return Ok(false);
        }
        self.detach_tile(group)?;
        self.insert_group_at_target(group, target)?;
        Ok(true)
    }

    fn reorder_tab(
        &mut self,
        group: TileId,
        pane: PaneId,
        index: usize,
    ) -> Result<bool, TileError> {
        let Some(TileNode::Tabs(tabs)) = self.get_node_mut(group) else {
            return Err(TileError::InvalidTileId);
        };
        let Some(from) = tabs.panes.iter().position(|candidate| *candidate == pane) else {
            return Err(TileError::InvalidPaneId);
        };
        let active_pane = tabs.panes.get(tabs.active).copied();
        let pane = tabs.panes.remove(from);
        let to = index.min(tabs.panes.len());
        tabs.panes.insert(to, pane);
        if let Some(active) = active_pane {
            tabs.active = tabs
                .panes
                .iter()
                .position(|candidate| *candidate == active)
                .unwrap_or(0);
        }
        Ok(from != to)
    }

    fn set_split_shares(&mut self, split: TileId, shares: Vec<f64>) -> Result<bool, TileError> {
        let Some(TileNode::Split(node)) = self.get_node_mut(split) else {
            return Err(TileError::InvalidTileId);
        };
        if shares.len() != node.children.len()
            || shares
                .iter()
                .any(|share| !share.is_finite() || *share <= 0.0)
        {
            return Err(TileError::InvalidOperation);
        }
        if node.shares == shares {
            return Ok(false);
        }
        node.shares = shares;
        Ok(true)
    }

    fn validate_target(&self, target: DockTarget) -> Result<(), TileError> {
        match target {
            DockTarget::Root => Ok(()),
            DockTarget::Split { tile, ratio, .. } => {
                if !is_valid_split_fraction(ratio) {
                    return Err(TileError::InvalidTarget);
                }
                self.get_node(tile).ok_or(TileError::InvalidTileId)?;
                Ok(())
            }
            DockTarget::TabInto { group, .. } => match self.get_node(group) {
                Some(TileNode::Tabs(_)) => Ok(()),
                Some(_) => Err(TileError::InvalidTarget),
                None => Err(TileError::InvalidTileId),
            },
        }
    }

    fn validate_group_target(&self, group: TileId, target: DockTarget) -> Result<(), TileError> {
        match target {
            DockTarget::Root => Ok(()),
            DockTarget::Split { tile, ratio, .. } => {
                if tile == group || !is_valid_split_fraction(ratio) {
                    return Err(TileError::InvalidTarget);
                }
                self.get_node(tile).ok_or(TileError::InvalidTileId)?;
                Ok(())
            }
            DockTarget::TabInto { .. } => Err(TileError::InvalidTarget),
        }
    }

    fn insert_tile_at_target(
        &mut self,
        tile: TileId,
        pane: PaneId,
        target: DockTarget,
    ) -> Result<(), TileError> {
        match target {
            DockTarget::Root => {
                let group = self.push_node(TileNode::tabs(vec![pane]));
                self.insert_tile_near(self.root, Axis::Horizontal, group, Placement::After, 0.5)
            }
            DockTarget::Split {
                tile: target,
                axis,
                placement,
                ratio,
            } => self.insert_tile_near(target, axis, tile, placement, ratio),
            DockTarget::TabInto { group, index } => {
                let Some(TileNode::Tabs(tabs)) = self.get_node_mut(group) else {
                    return Err(TileError::InvalidTarget);
                };
                let index = index.unwrap_or(tabs.panes.len()).min(tabs.panes.len());
                tabs.panes.insert(index, pane);
                tabs.active = index;
                self.clear_node(tile);
                Ok(())
            }
        }
    }

    fn insert_group_at_target(
        &mut self,
        group: TileId,
        target: DockTarget,
    ) -> Result<(), TileError> {
        match target {
            DockTarget::Root => {
                if group == self.root {
                    Ok(())
                } else {
                    self.insert_tile_near(self.root, Axis::Horizontal, group, Placement::After, 0.5)
                }
            }
            DockTarget::Split {
                tile,
                axis,
                placement,
                ratio,
            } => self.insert_tile_near(tile, axis, group, placement, ratio),
            DockTarget::TabInto { .. } => Err(TileError::InvalidTarget),
        }
    }

    fn insert_tile_near(
        &mut self,
        target: TileId,
        axis: Axis,
        inserted: TileId,
        placement: Placement,
        share: f64,
    ) -> Result<(), TileError> {
        debug_assert!(
            is_valid_split_fraction(share),
            "internal split insertion requires a finite fraction in 0.0..1.0",
        );
        self.get_node(target).ok_or(TileError::InvalidTileId)?;
        let parent = self.find_parent(target);
        if let Some((parent_id, child_index)) = parent {
            let Some(TileNode::Split(parent_split)) = self.get_node_mut(parent_id) else {
                return Err(TileError::InvalidOperation);
            };
            if parent_split.axis == axis {
                let old_shares = repaired_shares(parent_split.children.len(), &parent_split.shares);
                let mut constraints =
                    aligned_constraints(parent_split.children.len(), &parent_split.constraints);
                parent_split.shares = old_shares;
                let old_share = parent_split.shares[child_index];
                let inserted_share = (old_share * share).max(0.01);
                let target_share = (old_share - inserted_share).max(0.01);
                parent_split.shares[child_index] = target_share;
                let insert_index = match placement {
                    Placement::Before => child_index,
                    Placement::After => child_index + 1,
                };
                parent_split.children.insert(insert_index, inserted);
                parent_split.shares.insert(insert_index, inserted_share);
                constraints.insert(insert_index, 0.0);
                parent_split.constraints.min_major = compact_constraints(constraints);
                return Ok(());
            }
        }

        let (children, shares) = match placement {
            Placement::Before => (
                vec![inserted, target],
                vec![share.max(0.01), (1.0 - share).max(0.01)],
            ),
            Placement::After => (
                vec![target, inserted],
                vec![(1.0 - share).max(0.01), share.max(0.01)],
            ),
        };
        let split = self.push_node(TileNode::Split(SplitNode {
            axis,
            children,
            shares,
            constraints: SplitConstraints::default(),
        }));

        if target == self.root {
            self.root = split;
            Ok(())
        } else if let Some((parent_id, child_index)) = parent {
            let Some(TileNode::Split(parent_split)) = self.get_node_mut(parent_id) else {
                return Err(TileError::InvalidOperation);
            };
            parent_split.children[child_index] = split;
            Ok(())
        } else {
            Err(TileError::InvalidOperation)
        }
    }

    fn remove_pane(&mut self, pane: PaneId) -> Result<(), TileError> {
        match self.find_pane(pane).ok_or(TileError::InvalidPaneId)? {
            PaneLocation::Tile { tile } => {
                self.clear_node(tile);
            }
            PaneLocation::InTabs { group, index } => {
                let Some(TileNode::Tabs(tabs)) = self.get_node_mut(group) else {
                    return Err(TileError::InvalidTileId);
                };
                tabs.panes.remove(index);
                if tabs.panes.is_empty() {
                    tabs.active = 0;
                } else if tabs.active >= tabs.panes.len() {
                    tabs.active = tabs.panes.len() - 1;
                } else if index <= tabs.active && tabs.active > 0 {
                    tabs.active -= 1;
                }
            }
        }
        Ok(())
    }

    fn detach_tile(&mut self, tile: TileId) -> Result<(), TileError> {
        if tile == self.root {
            return Ok(());
        }
        let Some((parent_id, child_index)) = self.find_parent(tile) else {
            return Err(TileError::InvalidOperation);
        };
        let Some(TileNode::Split(parent)) = self.get_node_mut(parent_id) else {
            return Err(TileError::InvalidOperation);
        };
        let mut shares = repaired_shares(parent.children.len(), &parent.shares);
        let mut constraints = aligned_constraints(parent.children.len(), &parent.constraints);
        parent.children.remove(child_index);
        shares.remove(child_index);
        constraints.remove(child_index);
        parent.shares = shares;
        parent.constraints.min_major = compact_constraints(constraints);
        Ok(())
    }

    fn normalize_node(
        &mut self,
        id: TileId,
        visiting: &mut [bool],
        claimed: &mut [bool],
        panes: &mut Vec<PaneId>,
        sink: &mut RepairSink<'_>,
    ) -> Option<TileId> {
        let index = id.0 as usize;
        if index >= self.nodes.len()
            || visiting.get(index).copied().unwrap_or(false)
            || claimed.get(index).copied().unwrap_or(false)
        {
            sink.record(RepairAction::RemovedInvalidNode(id));
            return None;
        }
        if self.get_node(id).is_none() {
            sink.record(RepairAction::RemovedInvalidNode(id));
            return None;
        }

        claimed[index] = true;
        visiting[index] = true;
        let node = self.get_node(id).cloned()?;
        let result = match node {
            TileNode::Pane(pane) => {
                if panes.contains(&pane.pane) {
                    sink.record(RepairAction::RemovedDuplicatePane {
                        pane: pane.pane,
                        tile: id,
                    });
                    self.clear_node(id);
                    None
                } else {
                    panes.push(pane.pane);
                    Some(id)
                }
            }
            TileNode::Tabs(mut tabs) => {
                let active_pane = tabs.panes.get(tabs.active).copied();
                tabs.panes.retain(|pane| {
                    if panes.contains(pane) {
                        sink.record(RepairAction::RemovedDuplicatePane {
                            pane: *pane,
                            tile: id,
                        });
                        false
                    } else {
                        panes.push(*pane);
                        true
                    }
                });
                if tabs.panes.is_empty() {
                    sink.record(RepairAction::RemovedInvalidNode(id));
                    self.clear_node(id);
                    None
                } else {
                    let repaired_active = active_pane
                        .and_then(|active| tabs.panes.iter().position(|pane| *pane == active))
                        .unwrap_or(0);
                    if tabs.active != repaired_active {
                        tabs.active = repaired_active;
                        sink.record(RepairAction::RepairedActiveTab(id));
                    }
                    let _ = self.set_node(id, TileNode::Tabs(tabs));
                    Some(id)
                }
            }
            TileNode::Split(split) => {
                self.normalize_split(id, split, visiting, claimed, panes, sink)
            }
        };
        visiting[index] = false;
        result
    }

    fn normalize_split(
        &mut self,
        id: TileId,
        split: SplitNode,
        visiting: &mut [bool],
        claimed: &mut [bool],
        panes: &mut Vec<PaneId>,
        sink: &mut RepairSink<'_>,
    ) -> Option<TileId> {
        let old_shares = repaired_shares(split.children.len(), &split.shares);
        let old_constraints = aligned_constraints(split.children.len(), &split.constraints);
        if compact_constraints(old_constraints.clone()) != split.constraints.min_major {
            sink.record(RepairAction::RepairedConstraints(id));
        }
        if old_shares != split.shares {
            sink.record(RepairAction::RepairedShares(id));
        }
        let mut children = Vec::new();
        let mut shares = Vec::new();
        let mut constraints = Vec::new();
        let mut structure_changed = false;

        for (child_index, child) in split.children.iter().copied().enumerate() {
            let Some(child) = self.normalize_node(child, visiting, claimed, panes, sink) else {
                structure_changed = true;
                continue;
            };
            let share = old_shares.get(child_index).copied().unwrap_or(1.0);
            let minimum = old_constraints.get(child_index).copied().unwrap_or(0.0);
            if let Some(TileNode::Split(child_split)) = self.get_node(child).cloned()
                && child_split.axis == split.axis
                && minimum == 0.0
            {
                let child_shares = repaired_shares(child_split.children.len(), &child_split.shares);
                let child_constraints =
                    aligned_constraints(child_split.children.len(), &child_split.constraints);
                for (grand_index, grandchild) in child_split.children.iter().copied().enumerate() {
                    children.push(grandchild);
                    shares.push(share * child_shares.get(grand_index).copied().unwrap_or(1.0));
                    constraints.push(child_constraints.get(grand_index).copied().unwrap_or(0.0));
                }
                structure_changed = true;
                sink.record(RepairAction::CollapsedSplit(child));
                self.clear_node(child);
                continue;
            }
            children.push(child);
            shares.push(share);
            constraints.push(minimum);
        }

        match children.len() {
            0 => {
                sink.record(RepairAction::RemovedInvalidNode(id));
                self.clear_node(id);
                None
            }
            1 => {
                sink.record(RepairAction::CollapsedSplit(id));
                self.clear_node(id);
                children.first().copied()
            }
            _ => {
                let shares = repaired_shares(children.len(), &shares);
                if structure_changed || shares != split.shares {
                    sink.record(RepairAction::RepairedShares(id));
                }
                let _ = self.set_node(
                    id,
                    TileNode::Split(SplitNode {
                        axis: split.axis,
                        children,
                        shares,
                        constraints: SplitConstraints {
                            min_major: compact_constraints(constraints),
                        },
                    }),
                );
                Some(id)
            }
        }
    }

    fn mark_reachable(&self, id: TileId, reachable: &mut [bool]) {
        let index = id.0 as usize;
        if index >= reachable.len() || reachable[index] {
            return;
        }
        reachable[index] = true;
        if let Some(TileNode::Split(split)) = self.get_node(id) {
            for child in &split.children {
                self.mark_reachable(*child, reachable);
            }
        }
    }

    fn layout_tile(
        &self,
        id: TileId,
        rect: Rect,
        input: &LayoutInput,
        frame: &mut LayoutFrame,
        visiting: &mut [bool],
    ) {
        let index = id.0 as usize;
        if index >= self.nodes.len() || visiting.get(index).copied().unwrap_or(false) {
            return;
        }
        let Some(node) = self.get_node(id) else {
            return;
        };
        visiting[index] = true;
        match node {
            TileNode::Pane(pane) => {
                frame.focus_order.push(pane.pane);
                frame.panes.push(PaneFrame {
                    pane: pane.pane,
                    tile: id,
                    rect,
                    clip: rect,
                    active: true,
                });
                frame.hit_regions.push(HitRegion {
                    rect,
                    z: HIT_Z_PANE,
                    kind: HitKind::Pane { pane: pane.pane },
                });
                frame.paint_order.push(FrameItemId::Pane(pane.pane));
            }
            TileNode::Tabs(tabs) => self.layout_tabs(id, tabs, rect, input, frame),
            TileNode::Split(split) => self.layout_split(id, split, rect, input, frame, visiting),
        }
        visiting[index] = false;
    }

    fn layout_tabs(
        &self,
        id: TileId,
        tabs: &TabNode,
        rect: Rect,
        input: &LayoutInput,
        frame: &mut LayoutFrame,
    ) {
        for pane in &tabs.panes {
            frame.focus_order.push(*pane);
        }
        if tabs.panes.is_empty() {
            return;
        }
        let active_index = tabs.active.min(tabs.panes.len() - 1);
        let active_pane = tabs.panes[active_index];
        let (bar_rect, pane_rect) = split_tab_bar(rect, tabs.placement, input.tab_bar_thickness);

        if tabs.placement != TabBarPlacement::Hidden {
            let bar = TabBarFrame {
                group: id,
                rect: bar_rect,
                viewport: bar_rect,
                placement: tabs.placement,
                active_pane: Some(active_pane),
            };
            frame.tab_bars.push(bar);
            frame.hit_regions.push(HitRegion {
                rect: bar_rect,
                z: HIT_Z_TAB_BAR,
                kind: HitKind::TabBar { group: id },
            });
            frame.paint_order.push(FrameItemId::TabBar(id));

            let tab_rects = tab_rects(bar_rect, tabs.placement, tabs.panes.len());
            for (index, pane) in tabs.panes.iter().copied().enumerate() {
                let rect = tab_rects[index];
                frame.tabs.push(TabFrame {
                    group: id,
                    pane,
                    rect,
                    index,
                    active: index == active_index,
                });
                frame.hit_regions.push(HitRegion {
                    rect,
                    z: HIT_Z_TAB,
                    kind: HitKind::Tab { group: id, pane },
                });
                frame.paint_order.push(FrameItemId::Tab { group: id, pane });
            }
        }

        frame.panes.push(PaneFrame {
            pane: active_pane,
            tile: id,
            rect: pane_rect,
            clip: pane_rect,
            active: true,
        });
        frame.hit_regions.push(HitRegion {
            rect: pane_rect,
            z: HIT_Z_PANE,
            kind: HitKind::Pane { pane: active_pane },
        });
        frame.paint_order.push(FrameItemId::Pane(active_pane));
    }

    fn layout_split(
        &self,
        id: TileId,
        split: &SplitNode,
        rect: Rect,
        input: &LayoutInput,
        frame: &mut LayoutFrame,
        visiting: &mut [bool],
    ) {
        let children: Vec<_> = split
            .children
            .iter()
            .copied()
            .filter(|child| self.get_node(*child).is_some())
            .collect();
        if children.is_empty() {
            return;
        }
        if children.len() == 1 {
            self.layout_tile(children[0], rect, input, frame, visiting);
            return;
        }

        let handle = input.split_handle_thickness;
        let major = match split.axis {
            Axis::Horizontal => rect.width(),
            Axis::Vertical => rect.height(),
        };
        let handle_total = handle * (children.len() - 1) as f64;
        let child_total = (major - handle_total).max(0.0);
        let shares = repaired_shares(children.len(), &split.shares);
        let default_min_major = match split.axis {
            Axis::Horizontal => input.min_pane_size.width,
            Axis::Vertical => input.min_pane_size.height,
        };
        let min_major = split_min_major(children.len(), &split.constraints, default_min_major);
        let lengths = solve_lengths_with_min(child_total, &shares, &min_major);
        let mut cursor = match split.axis {
            Axis::Horizontal => rect.x0,
            Axis::Vertical => rect.y0,
        };

        for (index, child) in children.iter().copied().enumerate() {
            let length = lengths[index];
            let child_rect = match split.axis {
                Axis::Horizontal => Rect::new(cursor, rect.y0, cursor + length, rect.y1),
                Axis::Vertical => Rect::new(rect.x0, cursor, rect.x1, cursor + length),
            };
            frame.split_children.push(SplitChildFrame {
                split: id,
                child,
                index,
                rect: child_rect,
            });
            self.layout_tile(child, child_rect, input, frame, visiting);
            cursor += length;
            if index + 1 < children.len() {
                let handle_rect = match split.axis {
                    Axis::Horizontal => Rect::new(cursor, rect.y0, cursor + handle, rect.y1),
                    Axis::Vertical => Rect::new(rect.x0, cursor, rect.x1, cursor + handle),
                };
                frame.split_handles.push(SplitHandleFrame {
                    split: id,
                    handle: index,
                    axis: split.axis,
                    rect: handle_rect,
                });
                frame.hit_regions.push(HitRegion {
                    rect: handle_rect,
                    z: HIT_Z_SPLIT_HANDLE,
                    kind: HitKind::SplitHandle {
                        split: id,
                        handle: index,
                    },
                });
                frame.paint_order.push(FrameItemId::SplitHandle {
                    split: id,
                    handle: index,
                });
                cursor += handle;
            }
        }
    }

    fn find_pane(&self, pane: PaneId) -> Option<PaneLocation> {
        let mut pending = vec![self.root];
        let mut visited = Vec::new();
        while let Some(tile) = pending.pop() {
            if visited.contains(&tile) {
                continue;
            }
            visited.push(tile);
            match self.get_node(tile) {
                Some(TileNode::Pane(node)) if node.pane == pane => {
                    return Some(PaneLocation::Tile { tile });
                }
                Some(TileNode::Tabs(tabs)) => {
                    if let Some(index) = tabs.panes.iter().position(|candidate| *candidate == pane)
                    {
                        return Some(PaneLocation::InTabs { group: tile, index });
                    }
                }
                Some(TileNode::Split(split)) => {
                    pending.extend(split.children.iter().rev().copied());
                }
                _ => {}
            }
        }
        None
    }

    fn find_parent(&self, target: TileId) -> Option<(TileId, usize)> {
        let mut pending = vec![self.root];
        let mut visited = Vec::new();
        while let Some(tile) = pending.pop() {
            if visited.contains(&tile) {
                continue;
            }
            visited.push(tile);
            let Some(TileNode::Split(split)) = self.get_node(tile) else {
                continue;
            };
            for (index, child) in split.children.iter().copied().enumerate() {
                if child == target {
                    return Some((tile, index));
                }
            }
            pending.extend(split.children.iter().rev().copied());
        }
        None
    }

    fn contains_pane(&self, pane: PaneId) -> bool {
        self.find_pane(pane).is_some()
    }

    fn count_panes(&self) -> usize {
        self.panes().count()
    }
}

fn aligned_constraints(count: usize, constraints: &SplitConstraints) -> Vec<f64> {
    (0..count)
        .map(|index| match constraints.min_major.get(index).copied() {
            Some(minimum) if minimum.is_finite() && minimum >= 0.0 => minimum,
            _ => 0.0,
        })
        .collect()
}

fn compact_constraints(mut constraints: Vec<f64>) -> Vec<f64> {
    while constraints.last() == Some(&0.0) {
        constraints.pop();
    }
    constraints
}
