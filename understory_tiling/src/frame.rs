// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use crate::{Axis, PaneId, Point, Rect, Revision, TabBarPlacement, TileId};

/// Flattened output from a layout pass.
///
/// Returned by [`TileTree::layout`](crate::TileTree::layout). Renderers and hit
/// testing should consume this flattened data instead of walking
/// [`TileNode`](crate::TileNode) directly.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LayoutFrame {
    /// Tree revision used for this frame.
    pub revision: Revision,
    /// Active pane rectangles.
    pub panes: Vec<PaneFrame>,
    /// Visible tab bar rectangles.
    pub tab_bars: Vec<TabBarFrame>,
    /// Individual tab rectangles.
    pub tabs: Vec<TabFrame>,
    /// Solved split child rectangles.
    pub split_children: Vec<SplitChildFrame>,
    /// Split handle rectangles.
    pub split_handles: Vec<SplitHandleFrame>,
    /// Hit-test regions in frame coordinates.
    pub hit_regions: Vec<HitRegion>,
    /// Pane focus order in semantic traversal order.
    pub focus_order: Vec<PaneId>,
    /// Paint order hints for renderers.
    pub paint_order: Vec<FrameItemId>,
}

/// Flattened pane geometry.
///
/// Produced in [`LayoutFrame::panes`] for every visible pane body. Use `pane` to
/// look up application content and `rect`/`clip` to place it.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaneFrame {
    /// Pane id.
    pub pane: PaneId,
    /// Tile that produced the pane.
    pub tile: TileId,
    /// Pane rectangle.
    pub rect: Rect,
    /// Pane clip rectangle.
    pub clip: Rect,
    /// Whether this pane is active in its group.
    pub active: bool,
}

/// Flattened tab bar geometry.
///
/// Produced in [`LayoutFrame::tab_bars`] for tab groups whose
/// [`TabBarPlacement`] is not hidden. Renderers use it for tab strip chrome and
/// hit regions use it to start tab-group drags.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TabBarFrame {
    /// Tab group tile id.
    pub group: TileId,
    /// Tab bar rectangle.
    pub rect: Rect,
    /// Tab bar placement.
    pub placement: TabBarPlacement,
    /// Active pane in this group, if any.
    pub active_pane: Option<PaneId>,
}

/// Flattened tab geometry.
///
/// Produced in [`LayoutFrame::tabs`] for each tab in a visible tab group. Use it
/// to render tab labels/chrome and to start tab drags or reorder gestures.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TabFrame {
    /// Tab group tile id.
    pub group: TileId,
    /// Pane represented by this tab.
    pub pane: PaneId,
    /// Tab rectangle.
    pub rect: Rect,
    /// Tab index in the group.
    pub index: usize,
    /// Whether this tab is active.
    pub active: bool,
}

/// Solved geometry for one split child.
///
/// Produced in [`LayoutFrame::split_children`] for every child of every solved
/// split. Interaction code uses these records to compute resize proposals from
/// the same geometry that renderers see.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SplitChildFrame {
    /// Split tile id.
    pub split: TileId,
    /// Child tile id.
    pub child: TileId,
    /// Child index in the split.
    pub index: usize,
    /// Solved child rectangle.
    pub rect: Rect,
}

/// Flattened split handle geometry.
///
/// Produced in [`LayoutFrame::split_handles`] between split children. Pass the
/// hit result from these rectangles to [`begin_resize`](crate::begin_resize) to
/// start a resize interaction.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SplitHandleFrame {
    /// Split tile id.
    pub split: TileId,
    /// Handle index between child `handle` and `handle + 1`.
    pub handle: usize,
    /// Split axis.
    pub axis: Axis,
    /// Handle rectangle.
    pub rect: Rect,
}

/// Identifier for a flattened frame item.
///
/// Returned in [`LayoutFrame::paint_order`] as an ordering hint for renderers
/// that want a deterministic sequence matching the layout solver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FrameItemId {
    /// Pane item.
    Pane(PaneId),
    /// Tab item.
    Tab {
        /// Tab group.
        group: TileId,
        /// Pane represented by the tab.
        pane: PaneId,
    },
    /// Tab bar item.
    TabBar(TileId),
    /// Split handle item.
    SplitHandle {
        /// Split tile.
        split: TileId,
        /// Handle index.
        handle: usize,
    },
}

/// Hit-test region.
///
/// Produced in [`LayoutFrame::hit_regions`] and consumed by [`hit_test`]. Most
/// callers do not construct these manually unless they are building custom
/// frame data for tests.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HitRegion {
    /// Region rectangle.
    pub rect: Rect,
    /// Region z-order. Higher values win.
    pub z: i16,
    /// Region kind.
    pub kind: HitKind,
}

/// Semantic hit-test result.
///
/// Returned by [`hit_test`] and used by interaction helpers such as
/// [`begin_drag`](crate::begin_drag) and [`begin_resize`](crate::begin_resize)
/// to decide which session to create.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HitKind {
    /// A pane body.
    Pane {
        /// Hit pane.
        pane: PaneId,
    },
    /// A tab.
    Tab {
        /// Hit group.
        group: TileId,
        /// Hit pane tab.
        pane: PaneId,
    },
    /// A tab bar background.
    TabBar {
        /// Hit group.
        group: TileId,
    },
    /// A split handle.
    SplitHandle {
        /// Hit split.
        split: TileId,
        /// Hit handle index.
        handle: usize,
    },
    /// Empty layout space.
    Empty,
}

/// Performs flattened-frame hit testing.
#[must_use]
pub fn hit_test(frame: &LayoutFrame, point: Point) -> Option<HitKind> {
    let mut best: Option<(usize, HitRegion)> = None;
    for (index, region) in frame.hit_regions.iter().copied().enumerate() {
        if !region.rect.contains(point) {
            continue;
        }
        match best {
            Some((best_index, best_region))
                if region.z < best_region.z
                    || (region.z == best_region.z && index >= best_index) => {}
            _ => best = Some((index, region)),
        }
    }
    best.map(|(_, region)| region.kind)
}
