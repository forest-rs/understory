// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;
use alloc::vec::Vec;

use crate::{PaneId, Rect, Size, SurfaceId, TileId};

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

/// Basic min/max layout constraints.
///
/// This is a data shape for caller-authored constraints. The current MVP keeps
/// it in the model for forward compatibility; layout currently uses
/// [`LayoutInput::min_pane_size`] as the global minimum.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LayoutConstraints {
    /// Minimum accepted size.
    pub min_size: Size,
    /// Optional maximum accepted size.
    pub max_size: Option<Size>,
}

/// Per-child constraints for a split node.
///
/// Store this inside [`SplitNode`] when building or restoring a tree. The MVP
/// normalizes the data but mostly uses [`LayoutInput::min_pane_size`] while the
/// per-child constraint solver matures.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SplitConstraints {
    /// Minimum major-axis length per child.
    pub min_major: Vec<f64>,
}

/// A semantic tile node.
///
/// Construct these when creating a tree with [`TileTree::new`](crate::TileTree::new)
/// or when building tests. Query existing nodes with
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

/// Abstract surface kind.
///
/// Used by [`TileSurface`] to describe future root, floating, external-window,
/// or auto-hide surfaces. The MVP layout API only solves the root tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SurfaceKind {
    /// Primary root surface.
    Root,
    /// Future floating surface.
    Floating,
    /// Future external window surface.
    ExternalWindow,
    /// Future auto-hide strip surface.
    AutoHide,
}

/// Abstract surface record.
///
/// Keep this in persisted or host-side state when modelling more than one
/// surface. Current layout entry points take [`LayoutInput`] and solve the root
/// tree directly; they do not open windows or manage floating surfaces.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TileSurface {
    /// Surface id.
    pub id: SurfaceId,
    /// Surface kind.
    pub kind: SurfaceKind,
    /// Root tile for this surface.
    pub root: TileId,
    /// Surface bounds.
    ///
    /// Bounds supplied to layout APIs are expected to be finite.
    pub bounds: Rect,
}

/// Geometry-affecting input for layout solving.
///
/// Construct this at render time and pass it to
/// [`TileTree::layout`](crate::TileTree::layout). It is not stored in the tree;
/// changing it produces a new [`LayoutFrame`](crate::LayoutFrame) without
/// mutating semantic layout.
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
    /// Reserved switch for layout-time drop target generation.
    ///
    /// Current drag/drop target generation happens in
    /// [`drop_targets_for_drag`](crate::drop_targets_for_drag) from a solved
    /// [`LayoutFrame`](crate::LayoutFrame). This field is preserved for callers
    /// that want to keep layout inputs forward-compatible with a future layout
    /// pass that can emit target hints directly.
    pub generate_drop_targets: bool,
}

impl TileNode {
    /// Creates a pane leaf node.
    #[must_use]
    pub const fn pane(pane: PaneId) -> Self {
        Self::Pane(PaneNode { pane })
    }

    /// Creates a tab group with the first pane active.
    #[must_use]
    pub fn tabs(panes: Vec<PaneId>) -> Self {
        Self::Tabs(TabNode {
            panes,
            active: 0,
            placement: TabBarPlacement::Top,
        })
    }

    /// Creates a split node with equal shares.
    #[must_use]
    pub fn split(axis: Axis, children: Vec<TileId>) -> Self {
        let len = children.len();
        Self::Split(SplitNode {
            axis,
            children,
            shares: vec![1.0; len],
            constraints: SplitConstraints::default(),
        })
    }
}
