// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::Placement;
use crate::frame::hit_test;
use crate::util::{major_length, major_size, rect_distance, split_min_major};
use crate::{
    Axis, DockTarget, HitKind, LayoutFrame, LayoutInput, PaneId, Point, Rect, Revision, Size,
    SplitChildFrame, TileError, TileId, TileNode, TileOp, TileTree,
};

/// High-level drag intent.
///
/// Store this in [`InteractionOptions`] to choose what kind of drag may start
/// from a pointer-down hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DragIntent {
    /// Move the hit pane, tab, or tab group.
    Move,
}

/// Current interaction state.
///
/// Returned by [`begin_interaction`] and passed back to [`update_interaction`]
/// while the pointer moves. Hosts can inspect the variants for diagnostics, but
/// ordinary interaction code should not need to construct sessions directly.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum InteractionState {
    /// No active interaction.
    None,
    /// Pointer-down gesture that may become a drag after movement crosses a
    /// threshold.
    PendingDrag(PendingDrag),
    /// Active drag session.
    Drag(DragSession),
    /// Active resize session.
    Resize(ResizeSession),
}

/// Active drag session.
///
/// Stored in [`InteractionState::Drag`] after a pending drag crosses its
/// threshold. [`update_interaction`] refreshes `current` and `proposal` as the
/// pointer moves.
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

/// Pending drag gesture before the movement threshold is crossed.
///
/// Stored in [`InteractionState::PendingDrag`] by [`begin_interaction`].
/// [`update_interaction`] promotes it to [`InteractionState::Drag`] only after
/// the pointer has moved at least `threshold` from the origin. If the pointer is
/// released first, the host can treat the gesture as a click.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PendingDrag {
    session: DragSession,
    threshold: f64,
}

impl PendingDrag {
    /// Returns the pointer-down origin.
    #[must_use]
    pub const fn origin(&self) -> Point {
        self.session.origin
    }

    /// Returns the movement threshold in logical pixels.
    #[must_use]
    pub const fn threshold(&self) -> f64 {
        self.threshold
    }

    /// Returns the drag subject that would be started.
    #[must_use]
    pub const fn subject(&self) -> DragSubject {
        self.session.subject
    }

    /// Returns whether `point` crosses this pending drag's threshold.
    #[must_use]
    pub fn is_ready(&self, point: Point) -> bool {
        debug_assert!(point.is_finite(), "point must be finite");
        drag_distance_squared(self.session.origin, point) >= self.threshold * self.threshold
    }

    /// Returns a real drag session when `point` crosses the threshold.
    ///
    /// The returned session has the original pointer-down origin and the
    /// supplied `point` as its current position.
    #[must_use]
    pub fn update(&self, point: Point) -> Option<DragSession> {
        if !self.is_ready(point) {
            return None;
        }
        let mut session = self.session.clone();
        session.current = point;
        Some(session)
    }
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
/// Stored in [`InteractionState::Resize`] by [`begin_interaction`] when the
/// pointer starts on a split handle. [`update_interaction`] refreshes `current`
/// and `proposal` as the pointer moves.
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

/// Overlay geometry produced during interactions.
///
/// Returned from [`update_interaction`] for rendering drop targets, ghosts, and
/// active interaction affordances without mutating the committed [`TileTree`].
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OverlayFrame {
    /// Drop targets worth drawing in the overlay.
    ///
    /// [`update_interaction`] keeps this focused on the active accepted target,
    /// or the active rejected target when there is no accepted one. Use
    /// [`InteractionUpdate::candidates`] or [`drop_targets_for_drag`] when an
    /// advanced UI needs every generated candidate.
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
/// Returned by [`drop_targets_for_drag`] and drag updates. Renderers draw the
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
    /// Ranking distance from the pointer when generated.
    ///
    /// Edge targets use distance to the dock edge, which keeps overlapping
    /// edge zones reachable even when [`DragOptions::edge_zone_fraction`] is
    /// large. Other targets use distance to their hit rectangle.
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
/// preview rectangle instead of a full [`LayoutFrame`].
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
/// Returned in [`OverlayFrame::dragged`] from [`update_interaction`] so
/// renderers can draw the item following the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DraggedFrame {
    /// Dragged subject.
    pub subject: DragSubject,
    /// Subject rectangle.
    pub rect: Rect,
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
/// Stored in [`DragSession`] by [`update_interaction`]. It does not mutate the
/// tree until [`validate_interaction_update`](crate::validate_interaction_update)
/// or [`validate_proposal`](crate::validate_proposal) lowers it to a [`TileOp`].
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
/// Stored in [`ResizeSession`] by [`update_interaction`]. The proposed shares
/// are computed from the solved split child geometry in the current
/// [`LayoutFrame`], then clamped against [`ResizeOptions::min_pane_size`] and
/// per-split minimum constraints. Commit it through
/// [`validate_interaction_update`](crate::validate_interaction_update) and
/// [`commit_proposal`](crate::commit_proposal).
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

/// Result of updating any interaction state.
///
/// Returned by [`update_interaction`]. Store `overlay` and `preview` directly
/// in the host renderer state, then validate and commit `proposal` on
/// pointer-up if one is present.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InteractionUpdate {
    /// Current proposal.
    pub proposal: Option<Proposal>,
    /// Tree revision captured when the interaction began.
    ///
    /// Pass this to [`ProposalValidationInput::with_base_revision`](crate::ProposalValidationInput::with_base_revision)
    /// if the host commits through [`validate_proposal`](crate::validate_proposal).
    pub base_revision: Option<Revision>,
    /// All generated drop candidates.
    ///
    /// Populated for drag updates. Resize updates leave this empty because
    /// resize interactions do not generate dock targets.
    pub candidates: Vec<DropTargetFrame>,
    /// Overlay geometry.
    pub overlay: OverlayFrame,
    /// Optional render-ready preview layout.
    pub preview: Option<LayoutFrame>,
}

/// Resize options.
///
/// Store this in [`InteractionOptions`] to describe host resize geometry
/// constraints.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResizeOptions {
    /// Minimum pane size.
    ///
    /// Expected to be finite and non-negative.
    pub min_pane_size: Size,
}

impl Default for ResizeOptions {
    fn default() -> Self {
        Self {
            min_pane_size: Size::new(20.0, 20.0),
        }
    }
}

/// Drag/drop options.
///
/// Store this in [`InteractionOptions`] or pass it to [`drop_targets_for_drag`]
/// to control which targets are generated for a drag session.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DragOptions {
    /// Fraction of a tile edge used for split targets.
    ///
    /// Expected to be finite and in the range `0.0..=0.5`. Values above the
    /// default `0.25` intentionally create overlapping edge zones. The picker
    /// resolves those overlaps by normalized depth into each edge zone, then by
    /// priority and stable target order.
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
    /// Layout input used to produce full drag preview frames.
    ///
    /// Set this to the same geometry input used to create the current
    /// [`LayoutFrame`] when the host wants [`InteractionUpdate::preview`] to
    /// contain a solved preview layout. Leave it as `None` to use only overlay
    /// ghosts.
    pub preview_layout: Option<LayoutInput>,
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
            preview_layout: None,
        }
    }
}

/// Options for updating any interaction state.
///
/// Construct this with [`InteractionOptions::from_layout_input`] when pointer
/// interaction should use the same geometry constraints as layout solving.
/// Pass it to [`update_interaction`] so drag and resize updates can be handled
/// through one host code path.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InteractionOptions {
    /// Drag/drop options.
    pub drag: DragOptions,
    /// Resize options.
    pub resize: ResizeOptions,
    /// Drag intent used by [`begin_interaction`].
    pub drag_intent: DragIntent,
    /// Pointer movement required before a pending drag becomes active.
    ///
    /// Expected to be finite and non-negative.
    pub drag_threshold: f64,
}

impl Default for InteractionOptions {
    fn default() -> Self {
        Self {
            drag: DragOptions::default(),
            resize: ResizeOptions::default(),
            drag_intent: DragIntent::Move,
            drag_threshold: 5.0,
        }
    }
}

impl InteractionOptions {
    /// Creates interaction options from a layout input.
    ///
    /// Use this in renderers that keep one layout input per frame. Drag
    /// previews will relayout with `input`, and resize interactions will use
    /// `input.min_pane_size` for solved-geometry clamping.
    #[must_use]
    pub fn from_layout_input(input: LayoutInput) -> Self {
        let drag = DragOptions {
            preview_layout: Some(input),
            ..DragOptions::default()
        };

        let resize = ResizeOptions {
            min_pane_size: input.min_pane_size,
        };

        Self {
            drag,
            resize,
            ..Self::default()
        }
    }
}

/// Starts a drag session from a frame hit.
///
/// Call this after the host has decided a pointer gesture is a drag. For tabs,
/// hosts that support click-to-activate should usually wait until pointer
/// movement exceeds their drag threshold before calling this function.
#[must_use]
fn begin_drag(frame: &LayoutFrame, point: Point, intent: DragIntent) -> Option<DragSession> {
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

/// Starts a pending drag from a frame hit.
///
/// Call this on pointer-down when the host needs to distinguish click from drag.
/// If [`PendingDrag::update`] never returns a [`DragSession`] before pointer-up,
/// the host can run its click behavior instead.
#[must_use]
fn begin_pending_drag(
    frame: &LayoutFrame,
    point: Point,
    intent: DragIntent,
    threshold: f64,
) -> Option<PendingDrag> {
    debug_assert!(point.is_finite(), "point must be finite");
    debug_assert!(
        threshold.is_finite() && threshold >= 0.0,
        "drag threshold must be finite and non-negative",
    );
    Some(PendingDrag {
        session: begin_drag(frame, point, intent)?,
        threshold,
    })
}

/// Starts an interaction from a frame hit.
///
/// Call this on pointer-down when the host wants the high-level interaction
/// path. Split-handle resize wins over pending drag because both are derived
/// from the same [`LayoutFrame`] hit regions. Store the returned state and pass
/// it to [`update_interaction`] as the pointer moves.
#[must_use]
pub fn begin_interaction(
    frame: &LayoutFrame,
    point: Point,
    options: &InteractionOptions,
) -> InteractionState {
    if let Some(resize) = begin_resize(frame, point) {
        return InteractionState::Resize(resize);
    }
    if let Some(pending) =
        begin_pending_drag(frame, point, options.drag_intent, options.drag_threshold)
    {
        return InteractionState::PendingDrag(pending);
    }
    InteractionState::None
}

/// Updates the current interaction state.
///
/// Call this on pointer movement when the host stores one [`InteractionState`].
/// Pending drags become real drag sessions only after their movement threshold
/// is crossed. The returned [`InteractionUpdate`] can be rendered and validated
/// without matching on interaction kind.
#[must_use]
pub fn update_interaction(
    tree: &TileTree,
    frame: &LayoutFrame,
    state: &mut InteractionState,
    point: Point,
    options: &InteractionOptions,
) -> InteractionUpdate {
    match state {
        InteractionState::None => InteractionUpdate::default(),
        InteractionState::PendingDrag(pending) => {
            let Some(mut drag) = pending.update(point) else {
                return InteractionUpdate::default();
            };
            let update = update_drag(tree, frame, &mut drag, point, &options.drag);
            *state = InteractionState::Drag(drag);
            update
        }
        InteractionState::Drag(drag) => update_drag(tree, frame, drag, point, &options.drag),
        InteractionState::Resize(resize) => {
            update_resize(tree, frame, resize, point, &options.resize)
        }
    }
}

#[must_use]
fn update_drag(
    tree: &TileTree,
    frame: &LayoutFrame,
    drag: &mut DragSession,
    point: Point,
    options: &DragOptions,
) -> InteractionUpdate {
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
    let preview = options.preview_layout.and_then(|input| {
        proposal
            .as_ref()
            .and_then(|proposal| preview_for_dock_proposal(tree, proposal, input))
    });
    let overlay_targets = active_frame
        .into_iter()
        .chain(rejected_frame)
        .collect::<Vec<_>>();
    drag.proposal = proposal.clone();

    InteractionUpdate {
        proposal: proposal.map(Proposal::Dock),
        base_revision: Some(drag.base_revision),
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
        preview,
    }
}

/// Starts a resize session from a split handle hit.
#[must_use]
fn begin_resize(frame: &LayoutFrame, point: Point) -> Option<ResizeSession> {
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

#[must_use]
fn update_resize(
    tree: &TileTree,
    frame: &LayoutFrame,
    resize: &mut ResizeSession,
    point: Point,
    options: &ResizeOptions,
) -> InteractionUpdate {
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
    InteractionUpdate {
        proposal: proposal.map(Proposal::Resize),
        base_revision: Some(resize.base_revision),
        candidates: Vec::new(),
        overlay: OverlayFrame::default(),
        preview,
    }
}

/// Generates candidate drop targets for a drag session.
///
/// Advanced hosts can call this while inspecting an [`InteractionState::Drag`].
/// The returned targets are ranked and stable for deterministic selection. Some
/// candidates may have [`DropTargetFrame::accepts`] set to `false`; renderers
/// can still draw these as invalid destinations, but [`pick_drop_target`]
/// ignores them when choosing the commit-ready target.
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
    let tile_rect = rect;
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
            distance: split_edge_depth(tile_rect, point, edge_w, edge_h, target),
            accepts: accepts(target),
        });
    }
}

fn split_edge_depth(
    rect: Rect,
    point: Point,
    edge_width: f64,
    edge_height: f64,
    target: DockTarget,
) -> f64 {
    fn normalized(distance: f64, length: f64) -> f64 {
        if length > 0.0 { distance / length } else { 0.0 }
    }

    match target {
        DockTarget::Split {
            axis, placement, ..
        } => match (axis, placement) {
            (Axis::Horizontal, Placement::Before) => {
                normalized((point.x - rect.x0).abs(), edge_width)
            }
            (Axis::Horizontal, Placement::After) => {
                normalized((rect.x1 - point.x).abs(), edge_width)
            }
            (Axis::Vertical, Placement::Before) => {
                normalized((point.y - rect.y0).abs(), edge_height)
            }
            (Axis::Vertical, Placement::After) => {
                normalized((rect.y1 - point.y).abs(), edge_height)
            }
        },
        _ => rect_distance(rect, point),
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
) -> Option<LayoutFrame> {
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
    Some(preview_tree.layout(LayoutInput {
        bounds,
        tab_bar_thickness: tab_bar_thickness(frame),
        split_handle_thickness,
        min_pane_size: options.min_pane_size,
        generate_drop_targets: false,
    }))
}

fn preview_for_dock_proposal(
    tree: &TileTree,
    proposal: &DockProposal,
    input: LayoutInput,
) -> Option<LayoutFrame> {
    let op = op_for_dock_proposal(proposal.clone()).ok()?;
    let mut preview_tree = tree.clone();
    preview_tree.apply(op).ok()?;
    Some(preview_tree.layout(input))
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

fn drag_distance_squared(origin: Point, point: Point) -> f64 {
    let dx = point.x - origin.x;
    let dy = point.y - origin.y;
    dx * dx + dy * dy
}
