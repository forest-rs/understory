// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use crate::{PaneId, Rect, Size, TileId};

// TODO: Re-export `kurbo::Axis` instead once it supports serde under
// `kurbo/serde`.
/// Major axis for split layout.
///
/// Pass this to [`TileOp::SplitPane`](crate::TileOp::SplitPane) or
/// [`DockTarget::Split`](crate::DockTarget::Split) to choose whether children
/// are arranged horizontally or vertically. Solved frames echo the axis on
/// [`SplitHandleFrame`](crate::SplitHandleFrame) values so renderers know which
/// direction a handle moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Axis {
    /// Children are laid out left-to-right.
    Horizontal,
    /// Children are laid out top-to-bottom.
    Vertical,
}

/// Placement relative to an existing tile or tab.
///
/// Used in split operations and dock targets to say whether a new pane or group
/// should be inserted before or after the target tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Placement {
    /// Place before the target in the relevant axis or order.
    Before,
    /// Place after the target in the relevant axis or order.
    After,
}

/// Placement of a tab bar around a tab group.
///
/// Store this on a [`TabNode`] before layout.
/// [`TileTree::layout`](crate::TileTree::layout) returns the chosen placement on
/// [`TabBarFrame`](crate::TabBarFrame) so the renderer can draw the tab strip in
/// the same place that the solver reserved geometry for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TabBarPlacement {
    /// Place tabs along the top edge.
    Top,
    /// Place tabs along the bottom edge.
    Bottom,
    /// Place tabs along the left edge.
    Left,
    /// Place tabs along the right edge.
    Right,
    /// Do not emit tab bar geometry.
    Hidden,
}

/// Per-child constraints for a split node.
///
/// Store this in [`TileSpec::Split`](crate::TileSpec::Split) when a child needs
/// a stronger major-axis minimum than [`LayoutInput::min_pane_size`]. Missing
/// entries inherit the layout input minimum.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SplitConstraints {
    /// Minimum major-axis length per child.
    pub min_major: Vec<f64>,
}

/// A semantic tile node.
///
/// Returned by [`TileTree::node`](crate::TileTree::node) for semantic
/// inspection. Construct initial trees with [`TileSpec`](crate::TileSpec), and
/// use these node values directly only when building crate-internal tests. Query existing nodes with
/// [`TileTree::node`](crate::TileTree::node); renderers normally use the
/// flattened [`LayoutFrame`](crate::LayoutFrame) instead of walking `TileNode`
/// recursively.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TileNode {
    /// N-ary split node.
    Split(SplitNode),
    /// Tab group node.
    Tabs(TabNode),
    /// Single pane leaf node.
    Pane(PaneNode),
}

/// N-ary split node.
///
/// Use this inside [`TileNode::Split`] when constructing a custom tree. Most
/// callers create splits through [`TileOp::SplitPane`](crate::TileOp::SplitPane),
/// then read the solved child and handle geometry from
/// [`LayoutFrame`](crate::LayoutFrame).
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SplitNode {
    /// Major axis used to place children.
    pub axis: Axis,
    /// Child tile ids.
    pub children: Vec<TileId>,
    /// Relative child shares.
    ///
    /// Shares loaded from snapshots are repaired by normalization. Callers that
    /// construct valid trees directly should use finite positive shares.
    pub shares: Vec<f64>,
    /// Optional per-child split constraints.
    pub constraints: SplitConstraints,
}

/// Tab group node.
///
/// Use this inside [`TileNode::Tabs`] to group multiple application panes under
/// one tab strip. Layout returns one [`TabBarFrame`](crate::TabBarFrame), one
/// [`TabFrame`](crate::TabFrame) per pane, and one active
/// [`PaneFrame`](crate::PaneFrame) for the group.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TabNode {
    /// Panes in tab order.
    pub panes: Vec<PaneId>,
    /// Active tab index.
    pub active: usize,
    /// Tab bar placement.
    pub placement: TabBarPlacement,
}

/// Single pane leaf node.
///
/// Use this inside [`TileNode::Pane`] when a tile directly hosts one
/// application pane. Layout lowers it into a [`PaneFrame`](crate::PaneFrame).
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaneNode {
    /// Application-owned pane id.
    pub pane: PaneId,
}

/// Geometry-affecting input for layout solving.
///
/// Construct this at render time and pass it to
/// [`TileTree::layout`](crate::TileTree::layout). It is not stored in the tree;
/// changing it produces a new [`LayoutFrame`](crate::LayoutFrame) without
/// mutating semantic layout. Apply presentation-only transforms such as zoom to
/// the returned frame with
/// [`LayoutFrame::apply_projection`](crate::LayoutFrame::apply_projection).
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LayoutInput {
    /// Bounds available to the root tile.
    ///
    /// Coordinates are expected to be finite. Bounds are normalized before
    /// layout, so reversed edges are accepted.
    pub bounds: Rect,
    /// Thickness reserved for visible tab bars.
    ///
    /// Expected to be finite and non-negative.
    pub tab_bar_thickness: f64,
    /// Thickness reserved for split handles.
    ///
    /// Expected to be finite and non-negative.
    pub split_handle_thickness: f64,
    /// Minimum pane size used by the split solver.
    ///
    /// Expected to be finite and non-negative.
    pub min_pane_size: Size,
}

impl TileNode {
    /// Creates a pane leaf node.
    #[must_use]
    pub(crate) const fn pane(pane: PaneId) -> Self {
        Self::Pane(PaneNode { pane })
    }

    /// Creates a tab group with the first pane active.
    #[must_use]
    pub(crate) fn tabs(panes: Vec<PaneId>) -> Self {
        Self::Tabs(TabNode {
            panes,
            active: 0,
            placement: TabBarPlacement::Top,
        })
    }
}
