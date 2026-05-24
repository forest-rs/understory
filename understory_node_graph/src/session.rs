// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Ephemeral graph interaction/session state.

use kurbo::{Point, Rect};
use understory_selection::Selection;
use understory_view2d::Viewport2D;

use crate::element::HitTarget;
use crate::ids::{GraphElementId, NodeId, PortId};
use crate::revision::Revision;

/// Active graph interaction gesture for one view.
///
/// Hosts set this from pointer/keyboard handling while a gesture is in flight.
/// [`GraphComputed`](crate::GraphComputed) reads it to derive transient state
/// such as edge-creation preview geometry; it does not perform the mutation that
/// completes the gesture.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum InteractionState {
    /// No active gesture.
    #[default]
    Idle,
    /// A node drag gesture is active.
    ///
    /// The model stores the node and the pointer origin, but the host remains
    /// responsible for applying resulting position changes to
    /// [`GraphProjection`](crate::GraphProjection).
    DragNode {
        /// Node being dragged.
        node: NodeId,
        /// Pointer origin in view space.
        pointer_origin: Point,
    },
    /// A marquee selection gesture is active.
    ///
    /// Points are stored in view coordinates so the gesture remains stable while
    /// the viewport is inspected or converted by the host.
    Marquee {
        /// Marquee start in view space.
        start: Point,
        /// Marquee current point in view space.
        current: Point,
    },
    /// An edge-creation gesture is active.
    ///
    /// The preview starts at an output port and follows the pointer until the
    /// host either commits a semantic edge or cancels the gesture.
    CreateEdge {
        /// Source/output port.
        from: PortId,
        /// Pointer point in view space.
        pointer: Point,
    },
}

/// Fast-changing session state for one graph view.
///
/// `GraphSession` holds interaction state that should not be saved into the
/// durable graph document: selection, hover, focus, the active gesture, and the
/// current viewport. Applications usually create one session per visible graph
/// surface. A second view over the same document should have a separate session
/// so its selection and viewport can diverge.
#[derive(Clone, Debug)]
pub struct GraphSession {
    /// Selection state over graph elements.
    ///
    /// Use [`GraphSession::update_selection`] when mutating selection so the
    /// session revision tracks semantic selection changes.
    selection: Selection<GraphElementId>,
    /// Hover target, if any.
    ///
    /// Use [`GraphComputed::hit_test_view`](crate::GraphComputed::hit_test_view)
    /// or [`GraphComputed::hit_test_world`](crate::GraphComputed::hit_test_world)
    /// to compute candidates, then store the chosen target here.
    hover: Option<HitTarget>,
    /// Focused element, if any.
    focus: Option<GraphElementId>,
    /// Active gesture state.
    interaction: InteractionState,
    /// Current 2D viewport.
    ///
    /// Use [`GraphSession::update_viewport`] when mutating the viewport so the
    /// session revision tracks visibility and coordinate-space changes.
    viewport: Viewport2D,
    revision: Revision,
}

impl GraphSession {
    /// Creates a new session for the given view rectangle.
    ///
    /// The rectangle is in view coordinates, commonly the widget or canvas
    /// bounds. The initial world transform comes from [`Viewport2D`].
    #[must_use]
    pub fn new(view_rect: Rect) -> Self {
        Self {
            selection: Selection::new(),
            hover: None,
            focus: None,
            interaction: InteractionState::Idle,
            viewport: Viewport2D::new(view_rect),
            revision: Revision::new(),
        }
    }

    /// Returns the current session revision.
    ///
    /// The revision changes when setter or update methods alter session-owned
    /// state. Call [`GraphSession::bump_revision`] when host state outside this
    /// type changes how cached graph computations should interpret the session.
    #[must_use]
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Returns the current selection.
    #[must_use]
    pub fn selection(&self) -> &Selection<GraphElementId> {
        &self.selection
    }

    /// Mutates selection state and bumps the session revision when it changed.
    ///
    /// The closure receives the underlying [`Selection`]. If the selection's own
    /// revision changes, the session revision changes too.
    pub fn update_selection<R>(
        &mut self,
        update: impl FnOnce(&mut Selection<GraphElementId>) -> R,
    ) -> R {
        let before = self.selection.revision();
        let result = update(&mut self.selection);
        if self.selection.revision() != before {
            self.revision.bump();
        }
        result
    }

    /// Returns the hover target, if any.
    #[must_use]
    pub fn hover(&self) -> Option<HitTarget> {
        self.hover
    }

    /// Sets the hover target and bumps the revision if it changed.
    pub fn set_hover(&mut self, hover: Option<HitTarget>) {
        if self.hover != hover {
            self.hover = hover;
            self.revision.bump();
        }
    }

    /// Returns the focused element, if any.
    #[must_use]
    pub fn focus(&self) -> Option<GraphElementId> {
        self.focus
    }

    /// Sets the focused element and bumps the revision if it changed.
    pub fn set_focus(&mut self, focus: Option<GraphElementId>) {
        if self.focus != focus {
            self.focus = focus;
            self.revision.bump();
        }
    }

    /// Returns the active interaction state.
    #[must_use]
    pub fn interaction(&self) -> &InteractionState {
        &self.interaction
    }

    /// Sets the active interaction and bumps the revision if it changed.
    pub fn set_interaction(&mut self, interaction: InteractionState) {
        if self.interaction != interaction {
            self.interaction = interaction;
            self.revision.bump();
        }
    }

    /// Returns the current viewport.
    #[must_use]
    pub fn viewport(&self) -> &Viewport2D {
        &self.viewport
    }

    /// Mutates viewport state and bumps the session revision.
    ///
    /// [`Viewport2D`] does not expose its own revision counter, so this bumps
    /// the session revision after every closure call.
    pub fn update_viewport<R>(&mut self, update: impl FnOnce(&mut Viewport2D) -> R) -> R {
        let result = update(&mut self.viewport);
        self.revision.bump();
        result
    }

    /// Returns the current viewport rectangle in world coordinates.
    ///
    /// [`GraphComputed`](crate::GraphComputed) uses this rectangle to build
    /// visible node and edge lists.
    #[must_use]
    pub fn visible_world_rect(&self) -> Rect {
        self.viewport.visible_world_rect()
    }

    /// Marks the session changed after external state changes.
    ///
    /// This is the escape hatch for host state that affects interpretation of
    /// the session but is not stored directly in `GraphSession`. Prefer the
    /// setter and update methods for state owned by this type.
    pub fn bump_revision(&mut self) {
        self.revision.bump();
    }
}

#[cfg(test)]
mod tests {
    use kurbo::{Rect, Vec2};

    use super::GraphSession;
    use crate::ids::{GraphElementId, NodeId};

    #[test]
    fn selection_update_tracks_session_revision_only_when_selection_changes() {
        let mut session = GraphSession::new(Rect::new(0.0, 0.0, 100.0, 100.0));
        let initial = session.revision();

        let node = GraphElementId::Node(NodeId::from_parts(1, 0));
        assert!(session.update_selection(|selection| selection.select_only(node)));
        assert!(session.revision() > initial);
        let after_select = session.revision();

        assert!(!session.update_selection(|selection| selection.select_only(node)));
        assert_eq!(session.revision(), after_select);
    }

    #[test]
    fn viewport_update_tracks_session_revision() {
        let mut session = GraphSession::new(Rect::new(0.0, 0.0, 100.0, 100.0));
        let initial = session.revision();

        session.update_viewport(|viewport| viewport.pan_by_view(Vec2::new(10.0, 0.0)));

        assert!(session.revision() > initial);
    }
}
