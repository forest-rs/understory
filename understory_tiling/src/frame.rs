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
    /// Presentation projection metadata, when this frame has been collapsed onto
    /// one pane (for example by zoom).
    pub projection: Option<FrameProjection>,
    /// Hit-test regions in frame coordinates.
    pub hit_regions: Vec<HitRegion>,
    /// Pane focus order in semantic traversal order.
    pub focus_order: Vec<PaneId>,
    /// Paint order hints for renderers.
    pub paint_order: Vec<FrameItemId>,
}

/// Kind of presentation projection that collapsed a frame onto one pane.
///
/// Stored on [`FrameProjection::kind`] and echoed on
/// [`FrameCause::Projection`] so hosts can pick projection-specific animation
/// styling. Zoom is the only kind today; future modes (floating, auto-hide,
/// maximize) add a variant here without changing [`diff_frames`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ProjectionKind {
    /// The focus pane was expanded to fill the frame, hiding the rest.
    Zoom,
}

/// Presentation projection metadata for a solved frame.
///
/// Produced in [`LayoutFrame::projection`] when a projection (such as
/// [`LayoutInput::zoom`](crate::LayoutInput::zoom)) names a visible pane. Hosts
/// use `source_rect` as the focus pane's normal tiling rectangle and
/// `projected_rect` as its collapsed rectangle for projection/restore animation
/// planning.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FrameProjection {
    /// Projection kind that produced this frame.
    pub kind: ProjectionKind,
    /// Focus pane the frame collapsed onto.
    pub focus: PaneId,
    /// Tile that produced the focus pane.
    pub focus_tile: TileId,
    /// Focus pane rectangle before projection.
    pub source_rect: Rect,
    /// Focus pane rectangle after projection.
    pub projected_rect: Rect,
    /// Normal-frame items hidden by the projection.
    ///
    /// Diffing uses these records to distinguish items hidden or revealed by
    /// the projection from unrelated items added or removed while a frame is
    /// projected.
    pub hidden_items: Vec<ProjectionHiddenItem>,
}

/// Normal-frame item hidden by a projected [`LayoutFrame`].
///
/// Produced in [`FrameProjection::hidden_items`]. Hosts usually consume this
/// indirectly through [`diff_frames`], which uses it to emit precise projection
/// and restore transitions.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProjectionHiddenItem {
    /// Stable frame item hidden by the projection.
    pub item: FrameItemId,
    /// Normal tiling rectangle for the hidden item.
    pub rect: Rect,
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
/// Produced in [`LayoutFrame::split_handles`] between split children.
/// [`begin_interaction`](crate::begin_interaction) uses these rectangles to
/// start resize interactions before considering drag gestures.
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
/// Returned by [`diff_frames`] after a host solves an old and new
/// [`LayoutFrame`]. Renderers can use this to decide which stable pane, tab, tab
/// bar, split child, and split handle items were added, removed, moved, or
/// resized. Animation timing and interpolation remain the host's
/// responsibility.
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
    /// Where the item animates from or to (kinematics).
    pub motion: FrameMotion,
    /// Why the item changed (provenance).
    pub cause: FrameCause,
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

/// Kinematics for one frame item: where it animates from or to.
///
/// Returned in [`FrameItemDiff::motion`]. This is purely geometric and
/// independent of *why* the item changed (see [`FrameCause`]); the same three
/// shapes describe ordinary relayout and every presentation projection. Hints
/// are descriptive rather than prescriptive: visual interpolation remains the
/// host's responsibility.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FrameMotion {
    /// Item is present before and after; animate from its own
    /// [`before`](FrameItemDiff::before) rectangle.
    Stable,
    /// Item appears; animate from `from`, if an origin was identified.
    Enter {
        /// Rectangle to animate from, when one is known.
        from: Option<Rect>,
        /// Related item the origin came from, if any.
        anchor: Option<FrameItemId>,
    },
    /// Item disappears; animate toward `to`, if a target was identified.
    Exit {
        /// Rectangle to animate toward, when one is known.
        to: Option<Rect>,
        /// Related item the target came from, if any.
        anchor: Option<FrameItemId>,
    },
}

/// Provenance for one frame item: why it changed.
///
/// Returned in [`FrameItemDiff::cause`], orthogonal to [`FrameMotion`]. Ordinary
/// relayout is [`FrameCause::Relayout`]; a presentation projection (such as
/// zoom) is [`FrameCause::Projection`], carrying the projection kind so hosts
/// can pick animation styling without the diff growing a variant per kind.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FrameCause {
    /// Ordinary relayout (split resize, tab reorder, bounds change, …).
    Relayout,
    /// A presentation projection drove this item's change.
    Projection {
        /// Projection kind that drove the change.
        kind: ProjectionKind,
        /// Focus pane of the projection event.
        focus: PaneId,
        /// Role this item plays in the projection event.
        event: ProjectionEvent,
    },
}

/// Role a frame item plays in a [`FrameCause::Projection`] transition.
///
/// `Project`/`Restore` describe the focus pane growing into or shrinking back
/// from its projected rectangle; `Hide`/`Reveal` describe bystanders collapsing
/// toward or emerging from the focus pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ProjectionEvent {
    /// Focus pane is being projected (growing to fill the frame).
    Project,
    /// Focus pane is being restored (shrinking back to its normal rectangle).
    Restore,
    /// Bystander is hidden because the focus pane was projected.
    Hide,
    /// Bystander is revealed because a projection was restored.
    Reveal,
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
/// Returned by [`hit_test`] and used by
/// [`begin_interaction`](crate::begin_interaction) to decide which interaction
/// state to create.
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
/// resized instead of as a remove/add pair. Call this after relayout when a
/// host wants transition metadata for panes, tabs, tab bars, split children, and
/// split handles.
#[must_use]
pub fn diff_frames(before: &LayoutFrame, after: &LayoutFrame) -> FrameDiff {
    let before_items = frame_item_rects(before);
    let after_items = frame_item_rects(after);
    let mut items = Vec::new();

    for (item, after_rect) in &after_items {
        match find_item_rect(&before_items, *item) {
            Some(before_rect) => {
                if let Some(change) = classify_rect_change(before_rect, *after_rect) {
                    let cause = cause_for_changed(*item, before, after);
                    items.push(FrameItemDiff {
                        item: *item,
                        before: Some(before_rect),
                        after: Some(*after_rect),
                        change,
                        motion: FrameMotion::Stable,
                        cause,
                    });
                }
            }
            None => {
                let (motion, cause) =
                    transition_for_added(before, after, &before_items, *item, *after_rect);
                items.push(FrameItemDiff {
                    item: *item,
                    before: None,
                    after: Some(*after_rect),
                    change: FrameChange::Added,
                    motion,
                    cause,
                });
            }
        }
    }

    for (item, before_rect) in &before_items {
        if find_item_rect(&after_items, *item).is_none() {
            let (motion, cause) =
                transition_for_removed(before, after, &after_items, *item, *before_rect);
            items.push(FrameItemDiff {
                item: *item,
                before: Some(*before_rect),
                after: None,
                change: FrameChange::Removed,
                motion,
                cause,
            });
        }
    }

    FrameDiff { items }
}

pub(crate) fn frame_item_rects(frame: &LayoutFrame) -> Vec<(FrameItemId, Rect)> {
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

/// Provenance for a stable (present-before-and-after) item.
///
/// A stable item always animates from its own previous rectangle
/// ([`FrameMotion::Stable`]); only the cause varies, so this returns the cause
/// alone. The focus pane growing into or shrinking back from its projected
/// rectangle is a stable item whose rectangle change carries the kinematics.
fn cause_for_changed(item: FrameItemId, before: &LayoutFrame, after: &LayoutFrame) -> FrameCause {
    if let Some(projection) = &after.projection
        && item == FrameItemId::Pane(projection.focus)
        && before.projection.is_none()
    {
        return projection.cause(ProjectionEvent::Project);
    }
    if let Some(projection) = &before.projection
        && item == FrameItemId::Pane(projection.focus)
        && after.projection.is_none()
    {
        return projection.cause(ProjectionEvent::Restore);
    }
    FrameCause::Relayout
}

fn transition_for_added(
    before: &LayoutFrame,
    after: &LayoutFrame,
    before_items: &[(FrameItemId, Rect)],
    item: FrameItemId,
    after_rect: Rect,
) -> (FrameMotion, FrameCause) {
    // Focus pane appearing as the projected pane (e.g. a direct zoom switch):
    // animate from its own normal rectangle.
    if let Some(projection) = &after.projection
        && item == FrameItemId::Pane(projection.focus)
    {
        return (
            FrameMotion::Enter {
                from: Some(projection.source_rect),
                anchor: None,
            },
            projection.cause(ProjectionEvent::Project),
        );
    }
    // Bystander revealed because a projection was restored: emerge from where
    // the focus pane sat.
    if let Some(projection) = &before.projection
        && after.projection.is_none()
        && projection.hidden_rect(item).is_some()
    {
        return (
            FrameMotion::Enter {
                from: Some(projection.source_rect),
                anchor: Some(FrameItemId::Pane(projection.focus)),
            },
            projection.cause(ProjectionEvent::Reveal),
        );
    }
    let (from, anchor) = match related_rect(before_items, after_rect) {
        Some((anchor, rect)) => (Some(rect), Some(anchor)),
        None => (None, None),
    };
    (FrameMotion::Enter { from, anchor }, FrameCause::Relayout)
}

fn transition_for_removed(
    before: &LayoutFrame,
    after: &LayoutFrame,
    after_items: &[(FrameItemId, Rect)],
    item: FrameItemId,
    before_rect: Rect,
) -> (FrameMotion, FrameCause) {
    if let Some(projection) = &after.projection
        && let Some(hidden_rect) = projection.hidden_rect(item)
    {
        // The previously-projected focus pane being replaced by a new one
        // (direct switch): restore it toward its own normal rectangle.
        if let Some(before_projection) = &before.projection
            && item == FrameItemId::Pane(before_projection.focus)
        {
            return (
                FrameMotion::Exit {
                    to: Some(hidden_rect),
                    anchor: None,
                },
                before_projection.cause(ProjectionEvent::Restore),
            );
        }
        // Ordinary bystander hidden by the projection: collapse toward the
        // focus pane.
        return (
            FrameMotion::Exit {
                to: Some(projection.source_rect),
                anchor: Some(FrameItemId::Pane(projection.focus)),
            },
            projection.cause(ProjectionEvent::Hide),
        );
    }
    let (to, anchor) = match related_rect(after_items, before_rect) {
        Some((anchor, rect)) => (Some(rect), Some(anchor)),
        None => (None, None),
    };
    (FrameMotion::Exit { to, anchor }, FrameCause::Relayout)
}

impl FrameProjection {
    /// Builds a [`FrameCause::Projection`] for this projection and `event`.
    fn cause(&self, event: ProjectionEvent) -> FrameCause {
        FrameCause::Projection {
            kind: self.kind,
            focus: self.focus,
            event,
        }
    }

    /// Returns the normal rectangle recorded for a hidden `item`, if any.
    fn hidden_rect(&self, item: FrameItemId) -> Option<Rect> {
        self.hidden_items
            .iter()
            .find(|hidden| hidden.item == item)
            .map(|hidden| hidden.rect)
    }
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
