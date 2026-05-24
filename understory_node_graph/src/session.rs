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
    /// Mutating this field directly requires calling [`GraphSession::bump_revision`]
    /// so derived visibility and preview state can observe the change.
    pub selection: Selection<GraphElementId>,
    /// Hover target, if any.
    ///
    /// Use [`GraphComputed::hit_test_view`](crate::GraphComputed::hit_test_view)
    /// or [`GraphComputed::hit_test_world`](crate::GraphComputed::hit_test_world)
    /// to compute candidates, then store the chosen target here.
    pub hover: Option<HitTarget>,
    /// Focused element, if any.
    pub focus: Option<GraphElementId>,
    /// Active gesture state.
    pub interaction: InteractionState,
    /// Current 2D viewport.
    ///
    /// Mutating this field directly requires calling [`GraphSession::bump_revision`].
    pub viewport: Viewport2D,
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
    /// The revision changes when setter methods alter hover, focus, or
    /// interaction state. Call [`GraphSession::bump_revision`] after direct
    /// mutation of public fields such as `selection` or `viewport`.
    #[must_use]
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Sets the hover target and bumps the revision if it changed.
    pub fn set_hover(&mut self, hover: Option<HitTarget>) {
        if self.hover != hover {
            self.hover = hover;
            self.revision.bump();
        }
    }

    /// Sets the focused element and bumps the revision if it changed.
    pub fn set_focus(&mut self, focus: Option<GraphElementId>) {
        if self.focus != focus {
            self.focus = focus;
            self.revision.bump();
        }
    }

    /// Sets the active interaction and bumps the revision if it changed.
    pub fn set_interaction(&mut self, interaction: InteractionState) {
        if self.interaction != interaction {
            self.interaction = interaction;
            self.revision.bump();
        }
    }

    /// Returns the current viewport rectangle in world coordinates.
    ///
    /// [`GraphComputed`](crate::GraphComputed) uses this rectangle to build
    /// visible node and edge lists.
    #[must_use]
    pub fn visible_world_rect(&self) -> Rect {
        self.viewport.visible_world_rect()
    }

    /// Marks the session changed after direct mutation.
    ///
    /// This is the escape hatch for public fields whose types already have
    /// their own mutation APIs. Prefer the setter methods when changing hover,
    /// focus, or interaction state.
    pub fn bump_revision(&mut self) {
        self.revision.bump();
    }
}
