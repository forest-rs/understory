// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::Placement;
use crate::frame::hit_test;
use crate::policy::dock_policy_rejection;
use crate::util::{major_length, rect_distance};
use crate::{
    Axis, DockPolicy, DockTarget, HitKind, LayoutFrame, LayoutInput, PaneId, Point, Rect, Revision,
    Size, SplitChildFrame, TabBarPlacement, TileError, TileId, TileNode, TileOp, TileTree,
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

/// Semantic identity of a drop target within a layout revision.
///
/// The id stays the same as candidates appear or disappear, including when
/// the insertion gap changes within the same tab bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DropTargetId {
    /// Edge of a tile, including split containers and the root.
    Split {
        /// Target tile.
        tile: TileId,
        /// Split axis.
        axis: Axis,
        /// Edge along the axis.
        placement: Placement,
    },
    /// Tab strip insertion zone.
    TabBar(TileId),
    /// Group content zone that appends a tab.
    TabBody(TileId),
}

impl DropTargetId {
    fn from_target(target: DockTarget) -> Option<Self> {
        match target {
            DockTarget::Root => None,
            DockTarget::Split {
                tile,
                axis,
                placement,
                ..
            } => Some(Self::Split {
                tile,
                axis,
                placement,
            }),
            DockTarget::TabInto {
                group,
                index: Some(_),
            } => Some(Self::TabBar(group)),
            DockTarget::TabInto { group, index: None } => Some(Self::TabBody(group)),
        }
    }

    fn from_proposal(proposal: &DockProposal) -> Option<Self> {
        match *proposal {
            DockProposal::MovePane { target, .. } | DockProposal::MoveTabGroup { target, .. } => {
                Self::from_target(target)
            }
            DockProposal::ReorderTab { group, .. } => Some(Self::TabBar(group)),
        }
    }
}

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
    /// while the preview shows the pane area that would be created. Edge
    /// candidates start with an estimate. The active overlay target uses the
    /// solved placement when available. A tab-bar target uses an insertion marker.
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
    /// Why the candidate cannot accept this drag, if rejected.
    pub rejection: Option<DropRejection>,
}

/// Why a generated drop target is unavailable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DropRejection {
    /// The subject cannot be placed at this target.
    IncompatibleSubject,
    /// Same-group reordering is disabled in the drag options.
    ReorderDisabled,
    /// The layout is locked.
    LayoutLocked,
    /// Moving or reordering panes is disabled.
    MoveDisabled,
    /// Splitting is disabled.
    SplitDisabled,
    /// Inserting or reordering tabs is disabled.
    TabDisabled,
    /// The destination zone is disabled.
    ZoneDisabled,
    /// The destination edge is disabled.
    EdgeDisabled,
    /// Applying the proposed edit failed.
    InvalidOperation(TileError),
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
}

/// Uncommitted resize proposal.
///
/// Stored in [`ResizeSession`] by [`update_interaction`]. The proposed major
/// lengths are computed from solved split-child geometry in the current
/// [`LayoutFrame`], then clamped against [`ResizeOptions::min_pane_size`] and
/// per-split minimum constraints. Validation converts these proportional
/// lengths directly into the split's relative weights. Commit it through
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
    /// Proposed major-axis length for each split child.
    pub new_major_lengths: Vec<f64>,
}

/// Result of updating any interaction state.
///
/// Returned by [`update_interaction`]. Store `overlay` and `preview` directly
/// in the host renderer state, then validate and commit `proposal` on
/// pointer-up if one is present.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
    /// resolves overlaps at the same priority by normalized depth into each
    /// edge zone. Workspace edges occupy a narrow outer strip so they can be
    /// targeted separately from the edges of the outermost panes.
    pub edge_zone_fraction: f64,
    /// Fraction used for tab insertion decisions.
    ///
    /// Expected to be finite and in the range `0.0..=1.0`.
    pub tab_insert_threshold: f64,
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
            allow_reorder_tabs: true,
            allow_split: true,
            allow_tab_into: true,
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
    /// Layout input used to solve drag and resize preview frames.
    ///
    /// Preview frames contain base tiling geometry. Hosts apply the same
    /// presentation transforms used for the current frame before displaying a
    /// preview.
    pub preview_layout: Option<LayoutInput>,
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
            preview_layout: None,
            drag_intent: DragIntent::Move,
            drag_threshold: 5.0,
        }
    }
}

impl InteractionOptions {
    /// Creates interaction options from a layout input.
    ///
    /// Use this in renderers that keep one layout input per frame. Drag
    /// and resize previews will relayout with `input`, and resize interactions
    /// will use `input.min_pane_size` for solved-geometry clamping.
    #[must_use]
    pub fn from_layout_input(input: LayoutInput) -> Self {
        let resize = ResizeOptions {
            min_pane_size: input.min_pane_size,
        };

        Self {
            drag: DragOptions::default(),
            resize,
            preview_layout: Some(input),
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
    update_interaction_with_policy(tree, frame, state, point, options, &DockPolicy::default())
}

/// Updates an interaction using the same policy used to validate its commit.
///
/// Drag candidates expose policy rejection reasons and only eligible proposals
/// produce placement previews. Validate the update again on pointer-up with
/// the current policy before committing it.
#[must_use]
pub fn update_interaction_with_policy(
    tree: &TileTree,
    frame: &LayoutFrame,
    state: &mut InteractionState,
    point: Point,
    options: &InteractionOptions,
    policy: &DockPolicy,
) -> InteractionUpdate {
    if frame.revision() != tree.revision() {
        *state = InteractionState::None;
        return InteractionUpdate::default();
    }
    match state {
        InteractionState::None => InteractionUpdate::default(),
        InteractionState::PendingDrag(pending) => {
            let Some(mut drag) = pending.update(point) else {
                return InteractionUpdate::default();
            };
            let update = update_drag(
                tree,
                frame,
                &mut drag,
                point,
                &options.drag,
                options.preview_layout,
                policy,
            );
            *state = InteractionState::Drag(drag);
            update
        }
        InteractionState::Drag(drag) => update_drag(
            tree,
            frame,
            drag,
            point,
            &options.drag,
            options.preview_layout,
            policy,
        ),
        InteractionState::Resize(resize) if policy.locked_layout => {
            resize.proposal = None;
            InteractionUpdate::default()
        }
        InteractionState::Resize(resize) => update_resize(
            tree,
            frame,
            resize,
            point,
            &options.resize,
            options.preview_layout,
        ),
    }
}

#[must_use]
fn update_drag(
    tree: &TileTree,
    frame: &LayoutFrame,
    drag: &mut DragSession,
    point: Point,
    options: &DragOptions,
    preview_layout: Option<LayoutInput>,
    policy: &DockPolicy,
) -> InteractionUpdate {
    debug_assert!(point.is_finite(), "point must be finite");
    drag.current = point;
    let mut targets = drop_targets_for_drag_with_policy(tree, frame, drag, options, policy);
    let (active_frame, proposal, solved) = loop {
        let Some(target) = pick_drag_target(&targets, drag) else {
            break (None, None, None);
        };
        let candidate = targets
            .iter_mut()
            .find(|candidate| candidate.id == target.id)
            .expect("selected target came from the candidates");
        let Some(proposal) = proposal_for_drag(frame, drag, target.target) else {
            candidate.accepts = false;
            candidate.rejection = Some(DropRejection::IncompatibleSubject);
            continue;
        };
        match tree.edited(op_for_dock_proposal(&proposal)) {
            Ok(edited) => {
                let solved = edited
                    .as_ref()
                    .unwrap_or(tree)
                    .layout(preview_layout.unwrap_or(frame.layout_input));
                if matches!(target.target, DockTarget::Split { .. } | DockTarget::Root)
                    && let Some(rect) = solved_drop_rect(&solved, drag.subject)
                {
                    candidate.preview_rect = rect;
                }
                break (Some(*candidate), Some(proposal), Some(solved));
            }
            Err(error) => {
                candidate.accepts = false;
                candidate.rejection = Some(DropRejection::InvalidOperation(error));
            }
        }
    };
    let active_target = active_frame.map(|target| target.id);
    let rejected_frame = if active_frame.is_none() {
        pick_drop_target_frame(&targets, point, false)
    } else {
        None
    };
    let overlay_targets = active_frame
        .into_iter()
        .chain(rejected_frame)
        .collect::<Vec<_>>();
    drag.proposal = proposal.clone();

    let hit_regions = targets
        .iter()
        .map(|target| OverlayHitRegion {
            rect: target.rect,
            z: target.priority,
            target: target.id,
        })
        .collect();
    InteractionUpdate {
        proposal: proposal.map(Proposal::Dock),
        base_revision: Some(drag.base_revision),
        candidates: targets,
        overlay: OverlayFrame {
            active_target,
            hit_regions,
            drop_targets: overlay_targets,
            ghost_rects: ghost_rects_for_drag(drag, active_frame, rejected_frame),
            dragged: Some(DraggedFrame {
                subject: drag.subject,
                rect: Rect::new(point.x - 8.0, point.y - 8.0, point.x + 8.0, point.y + 8.0),
            }),
        },
        preview: preview_layout.and(solved),
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
    preview_layout: Option<LayoutInput>,
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
        .and_then(|proposal| preview_for_resize(tree, proposal, preview_layout));
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
    drop_targets_for_drag_with_policy(tree, frame, drag, options, &DockPolicy::default())
}

/// Generates drag targets with policy acceptance and rejection reasons.
///
/// Use [`update_interaction_with_policy`] to also probe the selected edit and
/// solve its preview.
#[must_use]
pub fn drop_targets_for_drag_with_policy(
    tree: &TileTree,
    frame: &LayoutFrame,
    drag: &DragSession,
    options: &DragOptions,
    policy: &DockPolicy,
) -> Vec<DropTargetFrame> {
    if frame.revision() != tree.revision() || drag.base_revision != tree.revision() {
        return Vec::new();
    }
    let mut targets = Vec::new();
    debug_assert!(
        (0.0..=0.5).contains(&options.edge_zone_fraction),
        "DragOptions::edge_zone_fraction must be finite and in 0.0..=0.5",
    );
    debug_assert!(
        (0.0..=1.0).contains(&options.tab_insert_threshold),
        "DragOptions::tab_insert_threshold must be finite and in 0.0..=1.0",
    );
    let edge_fraction = options.edge_zone_fraction;
    let rejection_for = |target| {
        if matches!(target, DockTarget::TabInto { group, index: Some(_) }
            if matches!(drag.subject, DragSubject::Tab { group: source, .. } if source == group))
            && !options.allow_reorder_tabs
        {
            return Some(DropRejection::ReorderDisabled);
        }
        if !target_accepts(tree, frame, drag, target, options) {
            return Some(DropRejection::IncompatibleSubject);
        }
        let Some(proposal) = proposal_for_drag(frame, drag, target) else {
            return Some(DropRejection::IncompatibleSubject);
        };
        dock_policy_rejection(&proposal, policy)
    };

    if options.allow_split {
        let split_workspace = matches!(tree.node(tree.root()), Some(TileNode::Split(_)));
        let (max_depth, priority) = if split_workspace {
            // The edge of the viewport docks beside the entire workspace.
            // Slightly farther inward, the outermost tile owns the same edge.
            (4.0, 28)
        } else {
            (f64::INFINITY, 25)
        };
        push_edge_targets(
            &mut targets,
            tree.root(),
            frame_bounds(frame),
            drag.current,
            edge_fraction,
            max_depth,
            priority,
            rejection_for,
        );
    }

    if options.allow_split {
        for child in &frame.split_children {
            if matches!(tree.node(child.child), Some(TileNode::Split(_))) {
                push_edge_targets(
                    &mut targets,
                    child.child,
                    child.rect,
                    drag.current,
                    edge_fraction.min(0.08),
                    f64::INFINITY,
                    27,
                    rejection_for,
                );
            }
        }
        for pane in &frame.panes {
            if pane.tile == tree.root() {
                continue;
            }
            push_edge_targets(
                &mut targets,
                pane.tile,
                pane_tile_bounds(frame, pane),
                drag.current,
                edge_fraction,
                f64::INFINITY,
                25,
                rejection_for,
            );
        }
    }

    if options.allow_tab_into {
        for bar in &frame.tab_bars {
            if !bar.viewport.contains(drag.current) {
                continue;
            }
            let (index, marker) =
                tab_insertion(frame, bar, drag.current, options.tab_insert_threshold);
            let target = DockTarget::TabInto {
                group: bar.group,
                index: Some(index),
            };
            let rejection = rejection_for(target);
            targets.push(DropTargetFrame {
                id: DropTargetId::TabBar(bar.group),
                rect: bar.viewport,
                target,
                preview_rect: marker,
                priority: 30,
                distance: rect_distance(bar.viewport, drag.current),
                accepts: rejection.is_none(),
                rejection,
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
                    let rejection = rejection_for(target);
                    targets.push(DropTargetFrame {
                        id: DropTargetId::TabBody(pane.tile),
                        rect,
                        target,
                        preview_rect: pane.rect,
                        priority: 15,
                        distance: rect_distance(rect, drag.current),
                        accepts: rejection.is_none(),
                        rejection,
                    });
                }
            }
        }
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

fn pick_drag_target(targets: &[DropTargetFrame], drag: &DragSession) -> Option<DropTargetFrame> {
    let best = pick_drop_target_frame(targets, drag.current, true)?;
    let previous = drag.proposal.as_ref().and_then(DropTargetId::from_proposal);
    // Keep the current edge through small changes in normalized edge depth.
    // A different priority or leaving the hit zone still switches immediately.
    if let Some(previous @ DropTargetId::Split { .. }) = previous
        && let Some(target) = targets.iter().find(|target| target.id == previous)
        && target.accepts
        && target.priority == best.priority
        && target.rect.contains(drag.current)
        && target.distance <= best.distance + 0.1
    {
        return Some(*target);
    }
    Some(best)
}

fn proposal_for_drag(
    frame: &LayoutFrame,
    drag: &DragSession,
    target: DockTarget,
) -> Option<DockProposal> {
    match drag.subject {
        DragSubject::Pane(pane) | DragSubject::Tab { pane, .. } => match target {
            DockTarget::TabInto {
                group,
                index: Some(gap),
            } if matches!(drag.subject, DragSubject::Tab { group: source, .. } if source == group) =>
            {
                let source = frame
                    .tabs
                    .iter()
                    .find(|tab| tab.group == group && tab.pane == pane)?;
                Some(DockProposal::ReorderTab {
                    group,
                    pane,
                    index: gap - usize::from(source.index < gap),
                })
            }
            _ => Some(DockProposal::MovePane { pane, target }),
        },
        DragSubject::TabGroup(group) => Some(DockProposal::MoveTabGroup { group, target }),
    }
}

pub(crate) fn op_for_dock_proposal(proposal: &DockProposal) -> TileOp {
    match *proposal {
        DockProposal::MovePane { pane, target } => TileOp::MovePane { pane, target },
        DockProposal::MoveTabGroup { group, target } => TileOp::MoveTabGroup { group, target },
        DockProposal::ReorderTab { group, pane, index } => {
            TileOp::ReorderTab { group, pane, index }
        }
    }
}

fn push_edge_targets(
    targets: &mut Vec<DropTargetFrame>,
    tile: TileId,
    rect: Rect,
    point: Point,
    edge_fraction: f64,
    max_depth: f64,
    priority: i16,
    mut rejection_for: impl FnMut(DockTarget) -> Option<DropRejection>,
) {
    let width = rect.width();
    let height = rect.height();
    debug_assert!(
        width >= 0.0 && height >= 0.0,
        "drop target generation requires non-negative pane dimensions",
    );
    if width == 0.0 || height == 0.0 || edge_fraction == 0.0 {
        return;
    }
    let edge_w = (width * edge_fraction).min(max_depth);
    let edge_h = (height * edge_fraction).min(max_depth);
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
        let id = DropTargetId::from_target(target).expect("edge target identifies a split");
        let rejection = rejection_for(target);
        targets.push(DropTargetFrame {
            id,
            rect,
            target,
            preview_rect,
            priority,
            distance: split_edge_depth(tile_rect, point, edge_w, edge_h, target),
            accepts: rejection.is_none(),
            rejection,
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
    match target {
        DockTarget::Root => true,
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
        DragSubject::TabGroup(group) => group != tile,
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

fn pane_tile_bounds(frame: &LayoutFrame, pane: &crate::PaneFrame) -> Rect {
    frame
        .tab_bars
        .iter()
        .find(|bar| bar.group == pane.tile)
        .map_or(pane.rect, |bar| union_rect(pane.rect, bar.rect))
}

fn solved_drop_rect(frame: &LayoutFrame, subject: DragSubject) -> Option<Rect> {
    match subject {
        DragSubject::Pane(pane) | DragSubject::Tab { pane, .. } => frame
            .panes
            .iter()
            .find(|candidate| candidate.pane == pane)
            .map(|pane| pane.rect),
        DragSubject::TabGroup(group) => {
            let pane = frame.panes.iter().find(|pane| pane.tile == group)?;
            Some(pane_tile_bounds(frame, pane))
        }
    }
}

fn frame_bounds(frame: &LayoutFrame) -> Rect {
    frame
        .projection
        .as_ref()
        .map_or(frame.layout_input.bounds.abs(), |projection| {
            projection.projected_rect
        })
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

fn tab_insertion(
    frame: &LayoutFrame,
    bar: &crate::TabBarFrame,
    point: Point,
    threshold: f64,
) -> (usize, Rect) {
    let horizontal = matches!(
        bar.placement,
        TabBarPlacement::Top | TabBarPlacement::Bottom
    );
    let (coordinate, viewport_start, viewport_end) = if horizontal {
        (point.x, bar.viewport.x0, bar.viewport.x1)
    } else {
        (point.y, bar.viewport.y0, bar.viewport.y1)
    };
    let mut gap = 0;
    let mut boundary = viewport_start;
    for tab in frame.tabs.iter().filter(|tab| tab.group == bar.group) {
        let (start, end) = if horizontal {
            (tab.rect.x0, tab.rect.x1)
        } else {
            (tab.rect.y0, tab.rect.y1)
        };
        if coordinate < start + (end - start) * threshold {
            gap = tab.index;
            boundary = start;
            break;
        }
        gap = tab.index + 1;
        boundary = end;
    }

    let width = (viewport_end - viewport_start).min(2.0);
    let latest_start = (viewport_end - width).max(viewport_start);
    let start = (boundary - width / 2.0).clamp(viewport_start, latest_start);
    let end = (start + width).min(viewport_end);
    let marker = if horizontal {
        Rect::new(start, bar.viewport.y0, end, bar.viewport.y1)
    } else {
        Rect::new(bar.viewport.x0, start, bar.viewport.x1, end)
    };
    (gap, marker)
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
    if resize.handle.checked_add(1)? >= split.children.len() || !tree.can_resize_split(split) {
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

    let min_major = tree.resize_minimums(resize.split, split, frame, options.min_pane_size);
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

    let mut new_major_lengths = Vec::with_capacity(children.len());
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
        new_major_lengths.push(length);
    }

    Some(ResizeProposal {
        split: resize.split,
        handle: resize.handle,
        delta,
        new_major_lengths,
    })
}

fn preview_for_resize(
    tree: &TileTree,
    proposal: &ResizeProposal,
    input: Option<LayoutInput>,
) -> Option<LayoutFrame> {
    let input = input?;
    let edited = tree
        .edited(TileOp::SetSplitShares {
            split: proposal.split,
            shares: proposal.new_major_lengths.clone(),
        })
        .ok()?;
    Some(edited.as_ref().unwrap_or(tree).layout(input))
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

fn drag_distance_squared(origin: Point, point: Point) -> f64 {
    let dx = point.x - origin.x;
    let dy = point.y - origin.y;
    dx * dx + dy * dy
}
