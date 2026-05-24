// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Ephemeral graph interaction/session state.

use kurbo::{Point, Rect};
use understory_selection::Selection;
use understory_view2d::Viewport2D;

use crate::element::HitTarget;
use crate::ids::{GraphElementId, NodeId, PortId};
use crate::revision::Revision;

/// Active graph interaction gesture.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum InteractionState {
    /// No active gesture.
    #[default]
    Idle,
    /// A node drag gesture is active.
    DragNode {
        /// Node being dragged.
        node: NodeId,
        /// Pointer origin in view space.
        pointer_origin: Point,
    },
    /// A marquee selection gesture is active.
    Marquee {
        /// Marquee start in view space.
        start: Point,
        /// Marquee current point in view space.
        current: Point,
    },
    /// An edge-creation gesture is active.
    CreateEdge {
        /// Source/output port.
        from: PortId,
        /// Pointer point in view space.
        pointer: Point,
    },
}

/// Fast-changing session state for one graph view.
#[derive(Clone, Debug)]
pub struct GraphSession {
    /// Selection state over graph elements.
    pub selection: Selection<GraphElementId>,
    /// Hover target, if any.
    pub hover: Option<HitTarget>,
    /// Focused element, if any.
    pub focus: Option<GraphElementId>,
    /// Active gesture state.
    pub interaction: InteractionState,
    /// Current 2D viewport.
    pub viewport: Viewport2D,
    revision: Revision,
}

impl GraphSession {
    /// Creates a new session for the given view rectangle.
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

    /// Returns the current revision.
    #[must_use]
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Sets the hover target.
    pub fn set_hover(&mut self, hover: Option<HitTarget>) {
        if self.hover != hover {
            self.hover = hover;
            self.revision.bump();
        }
    }

    /// Sets the focused element.
    pub fn set_focus(&mut self, focus: Option<GraphElementId>) {
        if self.focus != focus {
            self.focus = focus;
            self.revision.bump();
        }
    }

    /// Sets the active interaction.
    pub fn set_interaction(&mut self, interaction: InteractionState) {
        if self.interaction != interaction {
            self.interaction = interaction;
            self.revision.bump();
        }
    }

    /// Returns the visible world rectangle.
    #[must_use]
    pub fn visible_world_rect(&self) -> Rect {
        self.viewport.visible_world_rect()
    }

    /// Marks the session changed after direct mutation.
    pub fn bump_revision(&mut self) {
        self.revision.bump();
    }
}
