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
    /// Split child item.
    ///
    /// This is non-rendering geometry from [`LayoutFrame::split_children`].
    /// Hosts can use it for resize previews, transition planning, and debugging
    /// solved split layout.
    SplitChild {
        /// Split tile.
        split: TileId,
        /// Child tile.
        child: TileId,
    },
    /// Split handle item.
    SplitHandle {
        /// Split tile.
        split: TileId,
        /// Handle index.
        handle: usize,
    },
}

/// Layout difference between two frames.
///
/// Returned by [`diff_frames`]. Renderers can use this to decide which stable
/// frame items were added, removed, moved, or resized between two layout solves.
/// Animation timing and interpolation remain the host's responsibility.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FrameDiff {
    /// Changed items in deterministic order.
    pub items: Vec<FrameItemDiff>,
}

/// Difference for one stable frame item.
///
/// Produced inside [`FrameDiff::items`]. `before` is `None` for added items;
/// `after` is `None` for removed items.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FrameItemDiff {
    /// Stable frame item id.
    pub item: FrameItemId,
    /// Previous rectangle, if the item existed before.
    pub before: Option<Rect>,
    /// New rectangle, if the item exists after.
    pub after: Option<Rect>,
    /// Geometry change classification.
    pub change: FrameChange,
    /// Optional transition hint for animation planning.
    pub transition: Option<FrameTransitionHint>,
}

/// Geometry change classification for one frame item.
///
/// Returned in [`FrameItemDiff::change`] so hosts can choose animation behavior
/// without re-deriving whether an item was added, removed, moved, or resized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FrameChange {
    /// Item appears in the new frame only.
    Added,
    /// Item appears in the old frame only.
    Removed,
    /// Item keeps its size but changes origin.
    Moved,
    /// Item keeps its origin but changes size.
    Resized,
    /// Item changes both origin and size.
    MovedAndResized,
}

/// Transition hint for one frame item.
///
/// Returned in [`FrameItemDiff::transition`]. These hints are intentionally
/// descriptive rather than prescriptive: they tell a host where an item appears
/// to come from or go to, but animation timing and visual interpolation remain
/// outside this crate.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FrameTransitionHint {
    /// Item entered from a previous rectangle.
    EnteredFrom {
        /// Related previous item, if one was identified.
        item: Option<FrameItemId>,
        /// Previous rectangle to animate from.
        rect: Rect,
    },
    /// Item exited toward a new rectangle.
    ExitedTo {
        /// Related new item, if one was identified.
        item: Option<FrameItemId>,
        /// New rectangle to animate toward.
        rect: Rect,
    },
    /// Stable item should animate from its own previous rectangle.
    SharedOrigin(FrameItemId),
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

/// Computes geometry changes between two layout frames.
///
/// The returned diff contains changed items only. Items are matched by stable
/// [`FrameItemId`], so a pane that changes rectangle is reported as moved or
/// resized instead of as a remove/add pair.
#[must_use]
pub fn diff_frames(before: &LayoutFrame, after: &LayoutFrame) -> FrameDiff {
    let before_items = frame_item_rects(before);
    let after_items = frame_item_rects(after);
    let mut items = Vec::new();

    for (item, after_rect) in &after_items {
        match find_item_rect(&before_items, *item) {
            Some(before_rect) => {
                if let Some(change) = classify_rect_change(before_rect, *after_rect) {
                    items.push(FrameItemDiff {
                        item: *item,
                        before: Some(before_rect),
                        after: Some(*after_rect),
                        change,
                        transition: Some(FrameTransitionHint::SharedOrigin(*item)),
                    });
                }
            }
            None => items.push(FrameItemDiff {
                item: *item,
                before: None,
                after: Some(*after_rect),
                change: FrameChange::Added,
                transition: transition_for_added(&before_items, *after_rect),
            }),
        }
    }

    for (item, before_rect) in &before_items {
        if find_item_rect(&after_items, *item).is_none() {
            items.push(FrameItemDiff {
                item: *item,
                before: Some(*before_rect),
                after: None,
                change: FrameChange::Removed,
                transition: transition_for_removed(&after_items, *before_rect),
            });
        }
    }

    FrameDiff { items }
}

fn frame_item_rects(frame: &LayoutFrame) -> Vec<(FrameItemId, Rect)> {
    let mut items = Vec::new();
    for pane in &frame.panes {
        items.push((FrameItemId::Pane(pane.pane), pane.rect));
    }
    for bar in &frame.tab_bars {
        items.push((FrameItemId::TabBar(bar.group), bar.rect));
    }
    for tab in &frame.tabs {
        items.push((
            FrameItemId::Tab {
                group: tab.group,
                pane: tab.pane,
            },
            tab.rect,
        ));
    }
    for child in &frame.split_children {
        items.push((
            FrameItemId::SplitChild {
                split: child.split,
                child: child.child,
            },
            child.rect,
        ));
    }
    for handle in &frame.split_handles {
        items.push((
            FrameItemId::SplitHandle {
                split: handle.split,
                handle: handle.handle,
            },
            handle.rect,
        ));
    }
    items
}

fn find_item_rect(items: &[(FrameItemId, Rect)], item: FrameItemId) -> Option<Rect> {
    items
        .iter()
        .find(|(candidate, _)| *candidate == item)
        .map(|(_, rect)| *rect)
}

fn classify_rect_change(before: Rect, after: Rect) -> Option<FrameChange> {
    if before == after {
        return None;
    }
    let moved = before.x0 != after.x0 || before.y0 != after.y0;
    let resized = before.width() != after.width() || before.height() != after.height();
    match (moved, resized) {
        (true, true) => Some(FrameChange::MovedAndResized),
        (true, false) => Some(FrameChange::Moved),
        (false, true) => Some(FrameChange::Resized),
        (false, false) => None,
    }
}

fn transition_for_added(
    before_items: &[(FrameItemId, Rect)],
    after_rect: Rect,
) -> Option<FrameTransitionHint> {
    related_rect(before_items, after_rect).map(|(item, rect)| FrameTransitionHint::EnteredFrom {
        item: Some(item),
        rect,
    })
}

fn transition_for_removed(
    after_items: &[(FrameItemId, Rect)],
    before_rect: Rect,
) -> Option<FrameTransitionHint> {
    related_rect(after_items, before_rect).map(|(item, rect)| FrameTransitionHint::ExitedTo {
        item: Some(item),
        rect,
    })
}

fn related_rect(items: &[(FrameItemId, Rect)], rect: Rect) -> Option<(FrameItemId, Rect)> {
    let mut best: Option<(usize, FrameItemId, Rect, f64, f64)> = None;
    for (index, (item, candidate)) in items.iter().copied().enumerate() {
        let overlap = overlap_area(candidate, rect);
        let distance = center_distance_squared(candidate, rect);
        match best {
            Some((best_index, _, _, best_overlap, best_distance))
                if overlap < best_overlap
                    || (overlap == best_overlap && distance > best_distance)
                    || (overlap == best_overlap
                        && distance == best_distance
                        && index >= best_index) => {}
            _ => best = Some((index, item, candidate, overlap, distance)),
        }
    }
    best.map(|(_, item, rect, _, _)| (item, rect))
}

fn overlap_area(a: Rect, b: Rect) -> f64 {
    let width = (a.x1.min(b.x1) - a.x0.max(b.x0)).max(0.0);
    let height = (a.y1.min(b.y1) - a.y0.max(b.y0)).max(0.0);
    width * height
}

fn center_distance_squared(a: Rect, b: Rect) -> f64 {
    let ax = (a.x0 + a.x1) * 0.5;
    let ay = (a.y0 + a.y1) * 0.5;
    let bx = (b.x0 + b.x1) * 0.5;
    let by = (b.y0 + b.y1) * 0.5;
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
}
