// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::Placement;
use crate::frame::hit_test;
use crate::util::{major_length, major_size, rect_distance, split_min_major};
use crate::{
    Axis, DockTarget, HitKind, LayoutFrame, LayoutInput, PaneFrame, PaneId, Point, Rect, Revision,
    Size, SplitChildFrame, SplitHandleFrame, TabBarFrame, TabFrame, TileError, TileId, TileNode,
    TileOp, TileTree,
};

/// High-level drag intent.
///
/// Pass this to [`begin_drag`] with a pointer position in a [`LayoutFrame`] to
/// choose what kind of drag session may start from the hit region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DragIntent {
    /// Move the hit pane, tab, or tab group.
    Move,
}

/// Current interaction state.
///
/// Host applications may store this between pointer events if they want one
/// enum for drag and resize state. The free functions also work if the host
/// stores [`DragSession`] and [`ResizeSession`] separately.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum InteractionState {
    /// No active interaction.
    None,
    /// Active drag session.
    Drag(DragSession),
    /// Active resize session.
    Resize(ResizeSession),
}

/// Active drag session.
///
/// Returned by [`begin_drag`]. Pass it mutably to [`update_drag`] as the pointer
/// moves, then pass it to [`commit_drag`] on pointer-up to apply the current
/// proposal.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DragSession {
    /// Subject being dragged.
    pub subject: DragSubject,
    /// Source that started the drag.
    pub source: DragSource,
    /// Drag origin.
    pub origin: Point,
    /// Current pointer position.
    pub current: Point,
    /// Tree revision at drag start.
    pub base_revision: Revision,
    /// Current proposal.
    pub proposal: Option<DockProposal>,
}

/// Subject of a drag.
///
/// Stored in [`DragSession`] and echoed in overlay frames so renderers know
/// whether they are previewing one pane, one tab, or a whole tab group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DragSubject {
    /// Dragging a pane.
    Pane(PaneId),
    /// Dragging one tab.
    Tab {
        /// Source group.
        group: TileId,
        /// Pane represented by the tab.
        pane: PaneId,
    },
    /// Dragging a whole tab group.
    TabGroup(TileId),
}

/// Source region that started a drag.
///
/// Stored in [`DragSession`] for embedders that need to distinguish a tab drag
/// from pane-chrome or tab-bar gestures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DragSource {
    /// Drag started on a tab.
    Tab {
        /// Source group.
        group: TileId,
        /// Pane represented by the tab.
        pane: PaneId,
    },
    /// Drag started on a tab bar.
    TabBar {
        /// Source group.
        group: TileId,
    },
    /// Drag started on pane chrome.
    PaneChrome {
        /// Source pane.
        pane: PaneId,
    },
}

/// Active resize session.
///
/// Returned by [`begin_resize`]. Pass it mutably to [`update_resize`] as the
/// pointer moves, then pass it to [`commit_resize`] to apply the proposed split
/// shares.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResizeSession {
    /// Split being resized.
    pub split: TileId,
    /// Handle being moved.
    pub handle: usize,
    /// Split axis.
    pub axis: Axis,
    /// Resize origin.
    pub origin: Point,
    /// Current pointer position.
    pub current: Point,
    /// Tree revision at resize start.
    pub base_revision: Revision,
    /// Current resize proposal.
    pub proposal: Option<ResizeProposal>,
}

/// Flattened interaction output.
///
/// Container for combined interaction rendering output. Current update
/// functions return [`DragUpdate`] and [`ResizeUpdate`], whose fields map to
/// this shape.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InteractionFrame {
    /// Overlay geometry.
    pub overlay: OverlayFrame,
    /// Optional full preview frame.
    pub preview: Option<PreviewFrame>,
    /// Current proposal.
    pub proposal: Option<Proposal>,
}

/// Overlay geometry produced during interactions.
///
/// Returned from [`update_drag`] and [`update_resize`] for rendering drop
/// targets, ghosts, and active interaction affordances without mutating the
/// committed [`TileTree`].
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OverlayFrame {
    /// Drop targets worth drawing in the overlay.
    ///
    /// [`update_drag`] keeps this focused on the active accepted target, or the
    /// active rejected target when there is no accepted one. Use
    /// [`DragUpdate::candidates`] or [`drop_targets_for_drag`] when an advanced
    /// UI needs every generated candidate.
    pub drop_targets: Vec<DropTargetFrame>,
    /// Preview ghost rectangles.
    pub ghost_rects: Vec<GhostFrame>,
    /// Dragged subject geometry.
    pub dragged: Option<DraggedFrame>,
    /// Active drop target.
    pub active_target: Option<DropTargetId>,
    /// Overlay hit regions.
    pub hit_regions: Vec<OverlayHitRegion>,
}

/// Overlay hit region.
///
/// Produced inside [`OverlayFrame::hit_regions`] so overlay hit testing can
/// take precedence over the base [`LayoutFrame`] during drag/drop.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OverlayHitRegion {
    /// Region rectangle.
    pub rect: Rect,
    /// Region z-order.
    pub z: i16,
    /// Drop target id.
    pub target: DropTargetId,
}

/// Opaque drop target id.
///
/// Returned in [`DropTargetFrame`] and [`OverlayFrame::active_target`] to give
/// renderers a stable handle for highlighting one generated target.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DropTargetId(
    /// Numeric target id.
    pub u32,
);

/// Candidate drop target.
///
/// Returned by [`drop_targets_for_drag`] and [`update_drag`]. Renderers draw the
/// `rect` or `preview_rect`; commit code lowers the selected `target` into a
/// [`TileOp`]. Non-accepting targets are still useful for overlays because they
/// can show why a hovered destination is unavailable.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DropTargetFrame {
    /// Drop target id.
    pub id: DropTargetId,
    /// Hit rectangle.
    pub rect: Rect,
    /// Semantic dock target.
    pub target: DockTarget,
    /// Preview rectangle for overlays.
    ///
    /// This is usually larger than `rect`: the hit zone can be an edge strip
    /// while the preview shows the pane area that would be created.
    pub preview_rect: Rect,
    /// Target priority. Higher values win.
    pub priority: i16,
    /// Distance from the pointer when generated.
    pub distance: f64,
    /// Whether this target accepts the current subject.
    ///
    /// [`pick_drop_target`] only returns accepting targets. Interaction updates
    /// may still include non-accepting targets so renderers can display invalid
    /// hover feedback.
    pub accepts: bool,
}

/// Preview ghost rectangle.
///
/// Produced in [`OverlayFrame::ghost_rects`] when an interaction wants a simple
/// preview rectangle instead of a full [`PreviewFrame`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GhostFrame {
    /// Ghost rectangle.
    pub rect: Rect,
    /// Ghost kind.
    pub kind: GhostKind,
}

/// Kind of preview ghost.
///
/// Used by [`GhostFrame`] so renderers can style valid pane/group previews and
/// invalid targets differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GhostKind {
    /// Preview for one pane.
    PreviewPane,
    /// Preview for a group.
    PreviewGroup,
    /// Invalid-target preview.
    Invalid,
}

/// Geometry for the dragged subject.
///
/// Returned in [`OverlayFrame::dragged`] from [`update_drag`] so renderers can
/// draw the item following the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DraggedFrame {
    /// Dragged subject.
    pub subject: DragSubject,
    /// Subject rectangle.
    pub rect: Rect,
}

/// Optional full preview layout.
///
/// Reserved for interactions that want to preview a complete solved layout.
/// Current MVP updates usually return simple overlay targets instead.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PreviewFrame {
    /// Preview panes.
    pub panes: Vec<PaneFrame>,
    /// Preview tab bars.
    pub tab_bars: Vec<TabBarFrame>,
    /// Preview tabs.
    pub tabs: Vec<TabFrame>,
    /// Preview split handles.
    pub split_handles: Vec<SplitHandleFrame>,
}

impl PreviewFrame {
    /// Copies renderable geometry from a solved layout frame.
    ///
    /// Use this when an interaction wants to return a complete preview layout
    /// without exposing hit regions, focus order, or paint-order details.
    #[must_use]
    pub fn from_layout(frame: &LayoutFrame) -> Self {
        Self {
            panes: frame.panes.clone(),
            tab_bars: frame.tab_bars.clone(),
            tabs: frame.tabs.clone(),
            split_handles: frame.split_handles.clone(),
        }
    }
}

/// Uncommitted layout proposal.
///
/// Returned by interaction updates and passed to
/// [`validate_proposal`](crate::validate_proposal) when the host wants policy
/// validation before committing the corresponding [`TileOp`].
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Proposal {
    /// Docking or move proposal.
    Dock(DockProposal),
    /// Resize proposal.
    Resize(ResizeProposal),
}

/// Uncommitted docking proposal.
///
/// Returned from [`update_drag`] and stored in [`DragSession`]. It does not
/// mutate the tree until [`commit_drag`] or
/// [`validate_proposal`](crate::validate_proposal) lowers it to a [`TileOp`].
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DockProposal {
    /// Move one pane.
    MovePane {
        /// Pane to move.
        pane: PaneId,
        /// Move target.
        target: DockTarget,
    },
    /// Move a whole tab group.
    MoveTabGroup {
        /// Group to move.
        group: TileId,
        /// Move target.
        target: DockTarget,
    },
    /// Reorder one tab.
    ReorderTab {
        /// Tab group.
        group: TileId,
        /// Pane tab to reorder.
        pane: PaneId,
        /// Target index.
        index: usize,
    },
    /// Future float-pane proposal.
    FloatPane {
        /// Pane to float.
        pane: PaneId,
        /// Requested floating bounds.
        bounds: Rect,
    },
}

/// Uncommitted resize proposal.
///
/// Returned from [`update_resize`] and stored in [`ResizeSession`]. The proposed
/// shares are computed from the solved split child geometry in the current
/// [`LayoutFrame`], then clamped against [`ResizeOptions::min_pane_size`] and
/// per-split minimum constraints. Commit it with [`commit_resize`] to apply the
/// proposed split shares.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResizeProposal {
    /// Split tile.
    pub split: TileId,
    /// Handle index.
    pub handle: usize,
    /// Effective pointer delta along the split axis after clamping.
    ///
    /// Expected to be finite.
    pub delta: f64,
    /// Proposed replacement shares.
    pub new_shares: Vec<f64>,
}

/// Result of updating a drag session.
///
/// Returned by [`update_drag`] on every pointer move. Render `overlay`, inspect
/// `proposal`, and leave the committed tree untouched until commit.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DragUpdate {
    /// Current proposal.
    pub proposal: Option<DockProposal>,
    /// All generated drop candidates.
    ///
    /// Inspect this for custom target visualization or diagnostics. Most UIs
    /// should render [`DragUpdate::overlay`], which contains only the active
    /// target and active ghost.
    pub candidates: Vec<DropTargetFrame>,
    /// Overlay geometry.
    pub overlay: OverlayFrame,
    /// Optional full preview.
    pub preview: Option<PreviewFrame>,
}

/// Result of updating a resize session.
///
/// Returned by [`update_resize`] on pointer movement. Hosts may render the
/// overlay immediately and choose whether to commit the proposal live or on
/// pointer-up.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResizeUpdate {
    /// Current proposal.
    pub proposal: Option<ResizeProposal>,
    /// Overlay geometry.
    pub overlay: OverlayFrame,
    /// Optional full preview.
    pub preview: Option<PreviewFrame>,
}

/// Interaction commit behavior.
///
/// Store this in [`ResizeOptions`] to describe how the embedding layer plans to
/// commit proposals. The core still returns proposals either way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CommitMode {
    /// Commit on every pointer move.
    OnEveryMove,
    /// Commit only when the pointer is released.
    OnPointerUp,
}

/// Resize options.
///
/// Construct this and pass it to [`update_resize`] to describe host resize
/// policy and geometry constraints.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResizeOptions {
    /// Minimum pane size.
    ///
    /// Expected to be finite and non-negative.
    pub min_pane_size: Size,
    /// Commit behavior preferred by the embedding layer.
    pub commit_mode: CommitMode,
}

impl Default for ResizeOptions {
    fn default() -> Self {
        Self {
            min_pane_size: Size::new(20.0, 20.0),
            commit_mode: CommitMode::OnPointerUp,
        }
    }
}

/// Drag/drop options.
///
/// Construct this and pass it to [`update_drag`] or [`drop_targets_for_drag`] to
/// control which targets are generated for a drag session.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DragOptions {
    /// Fraction of a tile edge used for split targets.
    ///
    /// Expected to be finite and in the range `0.0..=0.5`.
    pub edge_zone_fraction: f64,
    /// Fraction used for tab insertion decisions.
    ///
    /// Expected to be finite and in the range `0.0..=1.0`.
    pub tab_insert_threshold: f64,
    /// Whether floating targets should be generated.
    ///
    /// Floating targets are currently marked non-accepting because committed
    /// floating surfaces are not implemented yet. The default is `false` so
    /// unsupported targets do not appear in normal interaction overlays.
    pub allow_float: bool,
    /// Whether tab reordering is allowed.
    pub allow_reorder_tabs: bool,
    /// Whether split targets are allowed.
    pub allow_split: bool,
    /// Whether tab-into targets are allowed.
    pub allow_tab_into: bool,
}

impl Default for DragOptions {
    fn default() -> Self {
        Self {
            edge_zone_fraction: 0.25,
            tab_insert_threshold: 0.5,
            allow_float: false,
            allow_reorder_tabs: true,
            allow_split: true,
            allow_tab_into: true,
        }
    }
}

/// Starts a drag session from a frame hit.
///
/// Call this after the host has decided a pointer gesture is a drag. For tabs,
/// hosts that support click-to-activate should usually wait until pointer
/// movement exceeds their drag threshold before calling this function.
#[must_use]
pub fn begin_drag(frame: &LayoutFrame, point: Point, intent: DragIntent) -> Option<DragSession> {
    debug_assert!(point.is_finite(), "point must be finite");
    let hit = hit_test(frame, point)?;
    match (intent, hit) {
        (DragIntent::Move, HitKind::Tab { group, pane }) => Some(DragSession {
            subject: DragSubject::Tab { group, pane },
            source: DragSource::Tab { group, pane },
            origin: point,
            current: point,
            base_revision: frame.revision,
            proposal: None,
        }),
        (DragIntent::Move, HitKind::TabBar { group }) => Some(DragSession {
            subject: DragSubject::TabGroup(group),
            source: DragSource::TabBar { group },
            origin: point,
            current: point,
            base_revision: frame.revision,
            proposal: None,
        }),
        (DragIntent::Move, HitKind::Pane { pane }) => Some(DragSession {
            subject: DragSubject::Pane(pane),
            source: DragSource::PaneChrome { pane },
            origin: point,
            current: point,
            base_revision: frame.revision,
            proposal: None,
        }),
        _ => None,
    }
}

/// Updates a drag session.
#[must_use]
pub fn update_drag(
    tree: &TileTree,
    frame: &LayoutFrame,
    drag: &mut DragSession,
    point: Point,
    options: &DragOptions,
) -> DragUpdate {
    debug_assert!(point.is_finite(), "point must be finite");
    drag.current = point;
    let targets = drop_targets_for_drag(tree, frame, drag, options);
    let active_frame = pick_drop_target_frame(&targets, point, true);
    let active_target = active_frame.map(|target| target.id);
    let rejected_frame = if active_frame.is_none() {
        pick_drop_target_frame(&targets, point, false)
    } else {
        None
    };
    let proposal = active_frame.and_then(|target| proposal_for_drag(drag, &target, options));
    let overlay_targets = active_frame
        .into_iter()
        .chain(rejected_frame)
        .collect::<Vec<_>>();
    drag.proposal = proposal.clone();

    DragUpdate {
        proposal,
        candidates: targets.clone(),
        overlay: OverlayFrame {
            active_target,
            hit_regions: targets
                .iter()
                .map(|target| OverlayHitRegion {
                    rect: target.rect,
                    z: target.priority,
                    target: target.id,
                })
                .collect(),
            drop_targets: overlay_targets,
            ghost_rects: ghost_rects_for_drag(drag, active_frame, rejected_frame),
            dragged: Some(DraggedFrame {
                subject: drag.subject,
                rect: Rect::new(point.x - 8.0, point.y - 8.0, point.x + 8.0, point.y + 8.0),
            }),
        },
        preview: None,
    }
}

/// Commits a drag session.
pub fn commit_drag(tree: &mut TileTree, drag: DragSession) -> Result<TileOp, TileError> {
    if drag.base_revision != tree.revision() {
        return Err(TileError::StaleInteraction);
    }
    let proposal = drag.proposal.ok_or(TileError::InvalidOperation)?;
    let op = op_for_dock_proposal(proposal)?;
    tree.apply(op.clone())?;
    Ok(op)
}

/// Starts a resize session from a split handle hit.
#[must_use]
pub fn begin_resize(frame: &LayoutFrame, point: Point) -> Option<ResizeSession> {
    debug_assert!(point.is_finite(), "point must be finite");
    match hit_test(frame, point)? {
        HitKind::SplitHandle { split, handle } => {
            let handle_frame = frame
                .split_handles
                .iter()
                .find(|candidate| candidate.split == split && candidate.handle == handle)?;
            Some(ResizeSession {
                split,
                handle,
                axis: handle_frame.axis,
                origin: point,
                current: point,
                base_revision: frame.revision,
                proposal: None,
            })
        }
        _ => None,
    }
}

/// Updates a resize session.
#[must_use]
pub fn update_resize(
    tree: &TileTree,
    frame: &LayoutFrame,
    resize: &mut ResizeSession,
    point: Point,
    options: &ResizeOptions,
) -> ResizeUpdate {
    debug_assert!(point.is_finite(), "point must be finite");
    debug_assert!(
        options.min_pane_size.is_finite()
            && options.min_pane_size.width >= 0.0
            && options.min_pane_size.height >= 0.0,
        "ResizeOptions::min_pane_size must be finite and non-negative",
    );
    resize.current = point;
    let proposal = resize_proposal_from_frame(tree, frame, resize, point, options);
    let preview = proposal
        .as_ref()
        .and_then(|proposal| preview_for_resize(tree, frame, proposal, options));
    resize.proposal = proposal.clone();
    ResizeUpdate {
        proposal,
        overlay: OverlayFrame::default(),
        preview,
    }
}

/// Commits a resize session.
pub fn commit_resize(tree: &mut TileTree, resize: ResizeSession) -> Result<TileOp, TileError> {
    if resize.base_revision != tree.revision() {
        return Err(TileError::StaleInteraction);
    }
    let proposal = resize.proposal.ok_or(TileError::InvalidOperation)?;
    let op = TileOp::SetSplitShares {
        split: proposal.split,
        shares: proposal.new_shares,
    };
    tree.apply(op.clone())?;
    Ok(op)
}

/// Generates candidate drop targets for a drag session.
///
/// Call this after [`begin_drag`] or from [`update_drag`] when the pointer
/// moves. The returned targets are ranked and stable for deterministic
/// selection. Some candidates may have [`DropTargetFrame::accepts`] set to
/// `false`; renderers can still draw these as invalid destinations, but
/// [`pick_drop_target`] ignores them when choosing the commit-ready target.
#[must_use]
pub fn drop_targets_for_drag(
    tree: &TileTree,
    frame: &LayoutFrame,
    drag: &DragSession,
    options: &DragOptions,
) -> Vec<DropTargetFrame> {
    let mut targets = Vec::new();
    if matches!(drag.subject, DragSubject::TabGroup(_)) {
        return targets;
    }
    debug_assert!(
        (0.0..=0.5).contains(&options.edge_zone_fraction),
        "DragOptions::edge_zone_fraction must be finite and in 0.0..=0.5",
    );
    debug_assert!(
        (0.0..=1.0).contains(&options.tab_insert_threshold),
        "DragOptions::tab_insert_threshold must be finite and in 0.0..=1.0",
    );
    let edge_fraction = options.edge_zone_fraction;

    if options.allow_split
        && let Some(root_rect) = frame_bounds(frame)
    {
        push_edge_targets(
            &mut targets,
            tree.root(),
            root_rect,
            drag.current,
            edge_fraction,
            10,
            |target| target_accepts(tree, frame, drag, target, options),
        );
    }

    if options.allow_split {
        for pane in &frame.panes {
            push_edge_targets(
                &mut targets,
                pane.tile,
                pane.rect,
                drag.current,
                edge_fraction,
                25,
                |target| target_accepts(tree, frame, drag, target, options),
            );
        }
    }

    if options.allow_tab_into {
        for bar in &frame.tab_bars {
            let index = match drag.subject {
                DragSubject::Tab { group, .. }
                    if group == bar.group && options.allow_reorder_tabs =>
                {
                    tab_insert_index(frame, bar.group, drag.current, options.tab_insert_threshold)
                }
                _ => None,
            };
            let id =
                DropTargetId(u32::try_from(targets.len()).expect("drop target arena exhausted"));
            let target = DockTarget::TabInto {
                group: bar.group,
                index,
            };
            targets.push(DropTargetFrame {
                id,
                rect: bar.rect,
                target,
                preview_rect: bar.rect,
                priority: 30,
                distance: rect_distance(bar.rect, drag.current),
                accepts: target_accepts(tree, frame, drag, target, options),
            });
        }

        for pane in &frame.panes {
            if matches!(tree.node(pane.tile), Some(TileNode::Tabs(_))) {
                let rect = center_rect(pane.rect, edge_fraction);
                if rect.width() > 0.0 && rect.height() > 0.0 {
                    let target = DockTarget::TabInto {
                        group: pane.tile,
                        index: None,
                    };
                    let id = DropTargetId(
                        u32::try_from(targets.len()).expect("drop target arena exhausted"),
                    );
                    targets.push(DropTargetFrame {
                        id,
                        rect,
                        target,
                        preview_rect: pane.rect,
                        priority: 15,
                        distance: rect_distance(rect, drag.current),
                        accepts: target_accepts(tree, frame, drag, target, options),
                    });
                }
            }
        }
    }

    if options.allow_float
        && let Some(root_rect) = frame_bounds(frame)
        && !root_rect.contains(drag.current)
    {
        let bounds = Rect::new(
            drag.current.x - 120.0,
            drag.current.y - 80.0,
            drag.current.x + 120.0,
            drag.current.y + 80.0,
        );
        let id = DropTargetId(u32::try_from(targets.len()).expect("drop target arena exhausted"));
        targets.push(DropTargetFrame {
            id,
            rect: bounds,
            target: DockTarget::Float { bounds },
            preview_rect: bounds,
            priority: 5,
            distance: 0.0,
            accepts: false,
        });
    }

    targets
}

/// Picks the active drop target for a point.
///
/// Only accepting targets are considered. Selection is deterministic: contained
/// targets are ordered by priority, then distance, then original target order.
#[must_use]
pub fn pick_drop_target(targets: &[DropTargetFrame], point: Point) -> Option<DropTargetId> {
    pick_drop_target_frame(targets, point, true).map(|target| target.id)
}

fn pick_drop_target_frame(
    targets: &[DropTargetFrame],
    point: Point,
    accepts: bool,
) -> Option<DropTargetFrame> {
    let mut best: Option<(usize, DropTargetFrame)> = None;
    for (index, target) in targets.iter().copied().enumerate() {
        if target.accepts != accepts || !target.rect.contains(point) {
            continue;
        }
        match best {
            Some((best_index, best_target))
                if compare_target(target, index, best_target, best_index) != Ordering::Greater => {}
            _ => best = Some((index, target)),
        }
    }
    best.map(|(_, target)| target)
}

fn proposal_for_drag(
    drag: &DragSession,
    target: &DropTargetFrame,
    options: &DragOptions,
) -> Option<DockProposal> {
    match drag.subject {
        DragSubject::Pane(pane) | DragSubject::Tab { pane, .. } => match target.target {
            DockTarget::TabInto { group, index }
                if options.allow_reorder_tabs
                    && matches!(drag.subject, DragSubject::Tab { group: source, .. } if source == group)
                    && index.is_some() =>
            {
                Some(DockProposal::ReorderTab {
                    group,
                    pane,
                    index: index.unwrap_or(0),
                })
            }
            _ => Some(DockProposal::MovePane {
                pane,
                target: target.target,
            }),
        },
        DragSubject::TabGroup(group) => Some(DockProposal::MoveTabGroup {
            group,
            target: target.target,
        }),
    }
}

pub(crate) fn op_for_dock_proposal(proposal: DockProposal) -> Result<TileOp, TileError> {
    match proposal {
        DockProposal::MovePane { pane, target } => Ok(TileOp::MovePane { pane, target }),
        DockProposal::ReorderTab { group, pane, index } => {
            Ok(TileOp::ReorderTab { group, pane, index })
        }
        DockProposal::FloatPane { pane, bounds } => Ok(TileOp::FloatPane { pane, bounds }),
        DockProposal::MoveTabGroup { .. } => Err(TileError::Unsupported),
    }
}

fn push_edge_targets(
    targets: &mut Vec<DropTargetFrame>,
    tile: TileId,
    rect: Rect,
    point: Point,
    edge_fraction: f64,
    priority: i16,
    mut accepts: impl FnMut(DockTarget) -> bool,
) {
    let width = rect.width();
    let height = rect.height();
    debug_assert!(
        width >= 0.0 && height >= 0.0,
        "drop target generation requires non-negative pane dimensions",
    );
    let edge_w = width * edge_fraction;
    let edge_h = height * edge_fraction;
    let specs = [
        (
            Rect::new(rect.x0, rect.y0, rect.x0 + edge_w, rect.y1),
            Rect::new(rect.x0, rect.y0, rect.x0 + width * 0.5, rect.y1),
            DockTarget::Split {
                tile,
                axis: Axis::Horizontal,
                placement: Placement::Before,
                ratio: 0.5,
            },
        ),
        (
            Rect::new(rect.x1 - edge_w, rect.y0, rect.x1, rect.y1),
            Rect::new(rect.x0 + width * 0.5, rect.y0, rect.x1, rect.y1),
            DockTarget::Split {
                tile,
                axis: Axis::Horizontal,
                placement: Placement::After,
                ratio: 0.5,
            },
        ),
        (
            Rect::new(rect.x0, rect.y0, rect.x1, rect.y0 + edge_h),
            Rect::new(rect.x0, rect.y0, rect.x1, rect.y0 + height * 0.5),
            DockTarget::Split {
                tile,
                axis: Axis::Vertical,
                placement: Placement::Before,
                ratio: 0.5,
            },
        ),
        (
            Rect::new(rect.x0, rect.y1 - edge_h, rect.x1, rect.y1),
            Rect::new(rect.x0, rect.y0 + height * 0.5, rect.x1, rect.y1),
            DockTarget::Split {
                tile,
                axis: Axis::Vertical,
                placement: Placement::After,
                ratio: 0.5,
            },
        ),
    ];

    for (rect, preview_rect, target) in specs {
        let id = DropTargetId(u32::try_from(targets.len()).expect("drop target arena exhausted"));
        targets.push(DropTargetFrame {
            id,
            rect,
            target,
            preview_rect,
            priority,
            distance: rect_distance(rect, point),
            accepts: accepts(target),
        });
    }
}

fn target_accepts(
    tree: &TileTree,
    frame: &LayoutFrame,
    drag: &DragSession,
    target: DockTarget,
    options: &DragOptions,
) -> bool {
    if matches!(drag.subject, DragSubject::TabGroup(_)) {
        return false;
    }
    match target {
        DockTarget::Root | DockTarget::Replace { .. } => true,
        DockTarget::Split { tile, .. } => split_target_accepts(tree, frame, drag, tile),
        DockTarget::TabInto { group, index } => {
            if !options.allow_tab_into {
                return false;
            }
            match drag.subject {
                DragSubject::Tab { group: source, .. } if source == group => {
                    index.is_some() && options.allow_reorder_tabs
                }
                DragSubject::Pane(pane) => source_group_for_pane(frame, tree, pane) != Some(group),
                DragSubject::Tab { .. } => true,
                DragSubject::TabGroup(_) => false,
            }
        }
        DockTarget::Float { .. } => false,
    }
}

fn split_target_accepts(
    tree: &TileTree,
    frame: &LayoutFrame,
    drag: &DragSession,
    tile: TileId,
) -> bool {
    match drag.subject {
        DragSubject::Pane(pane) => match source_tile_for_pane(frame, pane) {
            Some(source) if source == tile => match tree.node(tile) {
                Some(TileNode::Tabs(tabs)) => tabs.panes.len() > 1,
                _ => false,
            },
            Some(_) => true,
            None => false,
        },
        DragSubject::Tab { group, .. } => {
            if group != tile {
                return true;
            }
            match tree.node(group) {
                Some(TileNode::Tabs(tabs)) => tabs.panes.len() > 1,
                _ => false,
            }
        }
        DragSubject::TabGroup(_) => false,
    }
}

fn source_tile_for_pane(frame: &LayoutFrame, pane: PaneId) -> Option<TileId> {
    frame
        .panes
        .iter()
        .find(|candidate| candidate.pane == pane)
        .map(|candidate| candidate.tile)
}

fn source_group_for_pane(frame: &LayoutFrame, tree: &TileTree, pane: PaneId) -> Option<TileId> {
    let tile = source_tile_for_pane(frame, pane)?;
    matches!(tree.node(tile), Some(TileNode::Tabs(_))).then_some(tile)
}

fn ghost_rects_for_drag(
    drag: &DragSession,
    active: Option<DropTargetFrame>,
    rejected: Option<DropTargetFrame>,
) -> Vec<GhostFrame> {
    if let Some(target) = active {
        return vec![GhostFrame {
            rect: target.preview_rect,
            kind: ghost_kind_for_drag(drag),
        }];
    }
    if let Some(target) = rejected {
        return vec![GhostFrame {
            rect: target.preview_rect,
            kind: GhostKind::Invalid,
        }];
    }
    Vec::new()
}

fn ghost_kind_for_drag(drag: &DragSession) -> GhostKind {
    match drag.subject {
        DragSubject::TabGroup(_) => GhostKind::PreviewGroup,
        DragSubject::Pane(_) | DragSubject::Tab { .. } => GhostKind::PreviewPane,
    }
}

fn frame_bounds(frame: &LayoutFrame) -> Option<Rect> {
    let mut bounds: Option<Rect> = None;
    for rect in frame
        .panes
        .iter()
        .map(|pane| pane.rect)
        .chain(frame.tab_bars.iter().map(|bar| bar.rect))
        .chain(frame.split_handles.iter().map(|handle| handle.rect))
    {
        bounds = Some(match bounds {
            Some(bounds) => union_rect(bounds, rect),
            None => rect,
        });
    }
    bounds
}

fn union_rect(a: Rect, b: Rect) -> Rect {
    Rect::new(
        a.x0.min(b.x0),
        a.y0.min(b.y0),
        a.x1.max(b.x1),
        a.y1.max(b.y1),
    )
}

fn center_rect(rect: Rect, edge_fraction: f64) -> Rect {
    let dx = rect.width() * edge_fraction;
    let dy = rect.height() * edge_fraction;
    Rect::new(rect.x0 + dx, rect.y0 + dy, rect.x1 - dx, rect.y1 - dy)
}

fn compare_target(
    candidate: DropTargetFrame,
    candidate_index: usize,
    best: DropTargetFrame,
    best_index: usize,
) -> Ordering {
    candidate
        .priority
        .cmp(&best.priority)
        .then_with(|| {
            best.distance
                .partial_cmp(&candidate.distance)
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| best_index.cmp(&candidate_index))
}

fn tab_insert_index(
    frame: &LayoutFrame,
    group: TileId,
    point: Point,
    threshold: f64,
) -> Option<usize> {
    let mut tabs: Vec<_> = frame
        .tabs
        .iter()
        .filter(|tab| tab.group == group)
        .copied()
        .collect();
    tabs.sort_by_key(|tab| tab.index);
    if tabs.is_empty() {
        return Some(0);
    }
    for tab in &tabs {
        let horizontal = tab.rect.width() >= tab.rect.height();
        let insert_before = if horizontal {
            tab.rect.x0 + tab.rect.width() * threshold
        } else {
            tab.rect.y0 + tab.rect.height() * threshold
        };
        let coordinate = if horizontal { point.x } else { point.y };
        if coordinate < insert_before {
            return Some(tab.index);
        }
    }
    Some(tabs.len())
}

fn resize_proposal_from_frame(
    tree: &TileTree,
    frame: &LayoutFrame,
    resize: &ResizeSession,
    point: Point,
    options: &ResizeOptions,
) -> Option<ResizeProposal> {
    let TileNode::Split(split) = tree.node(resize.split)? else {
        return None;
    };
    if resize.handle + 1 >= split.children.len() {
        return None;
    }

    let children = split_child_frames(frame, resize.split);
    if children.len() != split.children.len() {
        return None;
    }
    for child in &children {
        if split.children.get(child.index).copied() != Some(child.child) {
            return None;
        }
    }

    let left = children.get(resize.handle)?;
    let right = children.get(resize.handle + 1)?;
    let left_length = major_length(left.rect, resize.axis);
    let right_length = major_length(right.rect, resize.axis);
    if left_length <= 0.0 || right_length <= 0.0 {
        return None;
    }

    let fallback_min_major = major_size(options.min_pane_size, resize.axis);
    let min_major = split_min_major(split.children.len(), &split.constraints, fallback_min_major);
    let min_left = min_major[resize.handle];
    let min_right = min_major[resize.handle + 1];
    let lower = min_left - left_length;
    let upper = right_length - min_right;
    if lower > upper {
        return None;
    }

    let requested_delta = match resize.axis {
        Axis::Horizontal => point.x - resize.origin.x,
        Axis::Vertical => point.y - resize.origin.y,
    };
    let delta = requested_delta.clamp(lower, upper);
    if delta == 0.0 {
        return None;
    }

    let mut new_shares = Vec::with_capacity(children.len());
    for child in &children {
        let mut length = major_length(child.rect, resize.axis);
        if child.index == resize.handle {
            length += delta;
        } else if child.index == resize.handle + 1 {
            length -= delta;
        }
        if length <= 0.0 {
            return None;
        }
        new_shares.push(length);
    }

    Some(ResizeProposal {
        split: resize.split,
        handle: resize.handle,
        delta,
        new_shares,
    })
}

fn preview_for_resize(
    tree: &TileTree,
    frame: &LayoutFrame,
    proposal: &ResizeProposal,
    options: &ResizeOptions,
) -> Option<PreviewFrame> {
    let bounds = frame_bounds(frame)?;
    let split_handle_thickness = frame
        .split_handles
        .iter()
        .find(|handle| handle.split == proposal.split && handle.handle == proposal.handle)
        .map(|handle| handle_thickness(handle.rect, handle.axis))?;
    let mut preview_tree = tree.clone();
    preview_tree
        .apply(TileOp::SetSplitShares {
            split: proposal.split,
            shares: proposal.new_shares.clone(),
        })
        .ok()?;
    let preview = preview_tree.layout(LayoutInput {
        bounds,
        tab_bar_thickness: tab_bar_thickness(frame),
        split_handle_thickness,
        min_pane_size: options.min_pane_size,
        generate_drop_targets: false,
    });
    Some(PreviewFrame::from_layout(&preview))
}

fn split_child_frames(frame: &LayoutFrame, split: TileId) -> Vec<SplitChildFrame> {
    let mut children = frame
        .split_children
        .iter()
        .filter(|child| child.split == split)
        .copied()
        .collect::<Vec<_>>();
    children.sort_by_key(|child| child.index);
    children
}

fn handle_thickness(rect: Rect, axis: Axis) -> f64 {
    major_length(rect, axis)
}

fn tab_bar_thickness(frame: &LayoutFrame) -> f64 {
    frame
        .tab_bars
        .iter()
        .map(|bar| match bar.placement {
            crate::TabBarPlacement::Top | crate::TabBarPlacement::Bottom => bar.rect.height(),
            crate::TabBarPlacement::Left | crate::TabBarPlacement::Right => bar.rect.width(),
            crate::TabBarPlacement::Hidden => 0.0,
        })
        .fold(0.0, f64::max)
}
