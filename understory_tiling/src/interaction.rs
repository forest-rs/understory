// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::Placement;
use crate::frame::hit_test;
use crate::util::{rect_distance, repaired_shares};
use crate::{
    Axis, DockTarget, HitKind, LayoutFrame, PaneFrame, PaneId, Point, Rect, Revision, Size,
    SplitHandleFrame, TabBarFrame, TabFrame, TileError, TileId, TileNode, TileOp, TileTree,
};

/// High-level drag intent.
///
/// Pass this to [`begin_drag`] with a pointer position in a [`LayoutFrame`] to
/// choose what kind of drag session may start from the hit region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
pub struct OverlayFrame {
    /// Drop targets.
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
pub struct DropTargetId(
    /// Numeric target id.
    pub u32,
);

/// Candidate drop target.
///
/// Returned by [`drop_targets_for_drag`] and [`update_drag`]. Renderers draw the
/// `rect` or `preview_rect`; commit code lowers the selected `target` into a
/// [`TileOp`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropTargetFrame {
    /// Drop target id.
    pub id: DropTargetId,
    /// Hit rectangle.
    pub rect: Rect,
    /// Semantic dock target.
    pub target: DockTarget,
    /// Preview rectangle for overlays.
    pub preview_rect: Rect,
    /// Target priority. Higher values win.
    pub priority: i16,
    /// Distance from the pointer when generated.
    pub distance: f64,
    /// Whether this target accepts the current subject.
    pub accepts: bool,
}

/// Preview ghost rectangle.
///
/// Produced in [`OverlayFrame::ghost_rects`] when an interaction wants a simple
/// preview rectangle instead of a full [`PreviewFrame`].
#[derive(Clone, Copy, Debug, PartialEq)]
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

/// Uncommitted layout proposal.
///
/// Returned by interaction updates and passed to
/// [`validate_proposal`](crate::validate_proposal) when the host wants policy
/// validation before committing the corresponding [`TileOp`].
#[derive(Clone, Debug)]
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
/// Returned from [`update_resize`] and stored in [`ResizeSession`]. Commit it
/// with [`commit_resize`] to apply the proposed split shares.
#[derive(Clone, Debug, PartialEq)]
pub struct ResizeProposal {
    /// Split tile.
    pub split: TileId,
    /// Handle index.
    pub handle: usize,
    /// Pointer delta along the split axis.
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
pub struct DragUpdate {
    /// Current proposal.
    pub proposal: Option<DockProposal>,
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
pub struct ResizeOptions {
    /// Minimum pane size.
    ///
    /// Expected to be finite and non-negative.
    pub min_pane_size: Size,
    /// Commit behavior preferred by the embedding layer.
    pub commit_mode: CommitMode,
}

/// Drag/drop options.
///
/// Construct this and pass it to [`update_drag`] or [`drop_targets_for_drag`] to
/// control which targets are generated for a drag session.
#[derive(Clone, Copy, Debug)]
pub struct DragOptions {
    /// Fraction of a tile edge used for split targets.
    ///
    /// Expected to be finite and in the range `0.0..=0.5`.
    pub edge_zone_fraction: f64,
    /// Fraction used for tab insertion decisions.
    ///
    /// Expected to be finite.
    pub tab_insert_threshold: f64,
    /// Whether floating is allowed.
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
            allow_float: true,
            allow_reorder_tabs: true,
            allow_split: true,
            allow_tab_into: true,
        }
    }
}

/// Starts a drag session from a frame hit.
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
    let active_target = pick_drop_target(&targets, point);
    let proposal = active_target
        .and_then(|id| targets.iter().find(|target| target.id == id))
        .and_then(|target| proposal_for_drag(drag, target, options));
    drag.proposal = proposal.clone();

    DragUpdate {
        proposal,
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
            drop_targets: targets,
            ghost_rects: Vec::new(),
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
    _frame: &LayoutFrame,
    resize: &mut ResizeSession,
    point: Point,
    _options: &ResizeOptions,
) -> ResizeUpdate {
    debug_assert!(point.is_finite(), "point must be finite");
    resize.current = point;
    let delta = match resize.axis {
        Axis::Horizontal => point.x - resize.origin.x,
        Axis::Vertical => point.y - resize.origin.y,
    };
    let new_shares = match tree.node(resize.split) {
        Some(TileNode::Split(split)) => {
            let mut shares = repaired_shares(split.children.len(), &split.shares);
            if resize.handle + 1 < shares.len() {
                let delta_share = delta / 100.0;
                shares[resize.handle] = (shares[resize.handle] + delta_share).max(0.01);
                shares[resize.handle + 1] = (shares[resize.handle + 1] - delta_share).max(0.01);
            }
            shares
        }
        _ => Vec::new(),
    };
    let proposal = (!new_shares.is_empty()).then_some(ResizeProposal {
        split: resize.split,
        handle: resize.handle,
        delta,
        new_shares,
    });
    resize.proposal = proposal.clone();
    ResizeUpdate {
        proposal,
        overlay: OverlayFrame::default(),
        preview: None,
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
#[must_use]
pub fn drop_targets_for_drag(
    _tree: &TileTree,
    frame: &LayoutFrame,
    drag: &DragSession,
    options: &DragOptions,
) -> Vec<DropTargetFrame> {
    let mut targets = Vec::new();
    debug_assert!(
        options.edge_zone_fraction.is_finite(),
        "DragOptions::edge_zone_fraction must be finite",
    );
    debug_assert!(
        (0.0..=0.5).contains(&options.edge_zone_fraction),
        "DragOptions::edge_zone_fraction must be in 0.0..=0.5",
    );
    debug_assert!(
        options.tab_insert_threshold.is_finite(),
        "DragOptions::tab_insert_threshold must be finite",
    );
    let edge_fraction = options.edge_zone_fraction;

    if options.allow_split {
        for pane in &frame.panes {
            push_edge_targets(
                &mut targets,
                pane.tile,
                pane.rect,
                drag.current,
                edge_fraction,
                true,
            );
        }
    }

    if options.allow_tab_into {
        for bar in &frame.tab_bars {
            let index = match drag.subject {
                DragSubject::Tab { group, .. }
                    if group == bar.group && options.allow_reorder_tabs =>
                {
                    tab_insert_index(frame, bar.group, drag.current)
                }
                _ => None,
            };
            let id =
                DropTargetId(u32::try_from(targets.len()).expect("drop target arena exhausted"));
            targets.push(DropTargetFrame {
                id,
                rect: bar.rect,
                target: DockTarget::TabInto {
                    group: bar.group,
                    index,
                },
                preview_rect: bar.rect,
                priority: 30,
                distance: rect_distance(bar.rect, drag.current),
                accepts: true,
            });
        }
    }

    targets
}

/// Picks the active drop target for a point.
#[must_use]
pub fn pick_drop_target(targets: &[DropTargetFrame], point: Point) -> Option<DropTargetId> {
    let mut best: Option<(usize, DropTargetFrame)> = None;
    for (index, target) in targets.iter().copied().enumerate() {
        if !target.accepts || !target.rect.contains(point) {
            continue;
        }
        match best {
            Some((best_index, best_target))
                if compare_target(target, index, best_target, best_index) != Ordering::Greater => {}
            _ => best = Some((index, target)),
        }
    }
    best.map(|(_, target)| target.id)
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
    accepts: bool,
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
            DockTarget::Split {
                tile,
                axis: Axis::Horizontal,
                placement: Placement::Before,
                ratio: 0.5,
            },
        ),
        (
            Rect::new(rect.x1 - edge_w, rect.y0, rect.x1, rect.y1),
            DockTarget::Split {
                tile,
                axis: Axis::Horizontal,
                placement: Placement::After,
                ratio: 0.5,
            },
        ),
        (
            Rect::new(rect.x0, rect.y0, rect.x1, rect.y0 + edge_h),
            DockTarget::Split {
                tile,
                axis: Axis::Vertical,
                placement: Placement::Before,
                ratio: 0.5,
            },
        ),
        (
            Rect::new(rect.x0, rect.y1 - edge_h, rect.x1, rect.y1),
            DockTarget::Split {
                tile,
                axis: Axis::Vertical,
                placement: Placement::After,
                ratio: 0.5,
            },
        ),
    ];

    for (rect, target) in specs {
        let id = DropTargetId(u32::try_from(targets.len()).expect("drop target arena exhausted"));
        targets.push(DropTargetFrame {
            id,
            rect,
            target,
            preview_rect: rect,
            priority: 20,
            distance: rect_distance(rect, point),
            accepts,
        });
    }
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

fn tab_insert_index(frame: &LayoutFrame, group: TileId, point: Point) -> Option<usize> {
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
        let midpoint = if horizontal {
            (tab.rect.x0 + tab.rect.x1) * 0.5
        } else {
            (tab.rect.y0 + tab.rect.y1) * 0.5
        };
        let coordinate = if horizontal { point.x } else { point.y };
        if coordinate < midpoint {
            return Some(tab.index);
        }
    }
    Some(tabs.len())
}
