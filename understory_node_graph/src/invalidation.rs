// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Node-graph invalidation channels and tracked targets.

use invalidation::{Channel, InvalidationSet};

use crate::ids::{EdgeId, NodeId, PortId};

/// Semantic graph content changed.
pub const GRAPH: Channel = Channel::new(0);
/// Projection/layout state changed.
pub const PROJECTION: Channel = Channel::new(1);
/// Session/interaction state changed.
pub const SESSION: Channel = Channel::new(2);
/// Viewport state changed.
pub const VIEWPORT: Channel = Channel::new(3);
/// Routing policy or edge route hints changed.
pub const ROUTING: Channel = Channel::new(4);
/// Visibility state changed.
pub const VISIBILITY: Channel = Channel::new(5);

/// Typed invalidation cause for node-graph systems.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum GraphInvalidationCause {
    /// Semantic graph changes.
    Graph,
    /// Projection changes.
    Projection,
    /// Session changes.
    Session,
    /// Viewport changes.
    Viewport,
    /// Routing changes.
    Routing,
    /// Visibility changes.
    Visibility,
}

impl GraphInvalidationCause {
    /// Returns the coarse invalidation channel.
    #[must_use]
    pub const fn channel(self) -> Channel {
        match self {
            Self::Graph => GRAPH,
            Self::Projection => PROJECTION,
            Self::Session => SESSION,
            Self::Viewport => VIEWPORT,
            Self::Routing => ROUTING,
            Self::Visibility => VISIBILITY,
        }
    }
}

/// Target of invalidation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum InvalidationTarget {
    /// Whole graph document.
    Graph,
    /// Whole projection.
    Projection,
    /// Whole session.
    Session,
    /// Whole viewport.
    Viewport,
    /// A node.
    Node(NodeId),
    /// A port.
    Port(PortId),
    /// An edge.
    Edge(EdgeId),
}

/// Coarse invalidation state for node-graph derived caches.
#[derive(Clone, Debug, Default)]
pub struct GraphInvalidation {
    set: InvalidationSet<InvalidationTarget>,
}

impl GraphInvalidation {
    /// Creates an empty invalidation state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current invalidation revision.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.set.generation()
    }

    /// Marks `target` invalid in the channel implied by `cause`.
    pub fn mark(&mut self, cause: GraphInvalidationCause, target: InvalidationTarget) -> bool {
        self.set.mark(target, cause.channel())
    }

    /// Marks the whole semantic graph invalid.
    pub fn mark_graph(&mut self) -> bool {
        self.mark(GraphInvalidationCause::Graph, InvalidationTarget::Graph)
    }

    /// Marks one semantic node invalid.
    pub fn mark_graph_node(&mut self, node: NodeId) -> bool {
        self.mark(
            GraphInvalidationCause::Graph,
            InvalidationTarget::Node(node),
        )
    }

    /// Marks one semantic port invalid.
    pub fn mark_graph_port(&mut self, port: PortId) -> bool {
        self.mark(
            GraphInvalidationCause::Graph,
            InvalidationTarget::Port(port),
        )
    }

    /// Marks one semantic edge invalid.
    pub fn mark_graph_edge(&mut self, edge: EdgeId) -> bool {
        self.mark(
            GraphInvalidationCause::Graph,
            InvalidationTarget::Edge(edge),
        )
    }

    /// Marks the whole projection invalid.
    pub fn mark_projection(&mut self) -> bool {
        self.mark(
            GraphInvalidationCause::Projection,
            InvalidationTarget::Projection,
        )
    }

    /// Marks one projected node invalid.
    pub fn mark_projection_node(&mut self, node: NodeId) -> bool {
        self.mark(
            GraphInvalidationCause::Projection,
            InvalidationTarget::Node(node),
        )
    }

    /// Marks one projected port invalid.
    pub fn mark_projection_port(&mut self, port: PortId) -> bool {
        self.mark(
            GraphInvalidationCause::Projection,
            InvalidationTarget::Port(port),
        )
    }

    /// Marks one projected edge invalid.
    pub fn mark_projection_edge(&mut self, edge: EdgeId) -> bool {
        self.mark(
            GraphInvalidationCause::Projection,
            InvalidationTarget::Edge(edge),
        )
    }

    /// Marks the whole session invalid.
    pub fn mark_session(&mut self) -> bool {
        self.mark(GraphInvalidationCause::Session, InvalidationTarget::Session)
    }

    /// Marks the viewport invalid.
    pub fn mark_viewport(&mut self) -> bool {
        self.mark(
            GraphInvalidationCause::Viewport,
            InvalidationTarget::Viewport,
        )
    }

    /// Marks edge routing invalid.
    pub fn mark_routing(&mut self) -> bool {
        self.mark(GraphInvalidationCause::Routing, InvalidationTarget::Graph)
    }

    /// Marks one routed edge invalid.
    pub fn mark_routing_edge(&mut self, edge: EdgeId) -> bool {
        self.mark(
            GraphInvalidationCause::Routing,
            InvalidationTarget::Edge(edge),
        )
    }

    /// Marks visibility invalid.
    pub fn mark_visibility(&mut self) -> bool {
        self.mark(
            GraphInvalidationCause::Visibility,
            InvalidationTarget::Graph,
        )
    }

    /// Returns `true` if any targets are invalid in `cause`'s channel.
    #[must_use]
    pub fn has_cause(&self, cause: GraphInvalidationCause) -> bool {
        self.set.has_invalidated(cause.channel())
    }

    /// Returns `true` if there are no pending invalidations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    /// Iterates invalid targets for `cause`.
    pub fn iter(
        &self,
        cause: GraphInvalidationCause,
    ) -> impl Iterator<Item = InvalidationTarget> + '_ {
        self.set.iter(cause.channel())
    }

    /// Clears one invalidation cause.
    pub fn clear(&mut self, cause: GraphInvalidationCause) {
        self.set.clear(cause.channel());
    }

    /// Clears all invalidation causes.
    pub fn clear_all(&mut self) {
        self.set.clear_all();
    }
}

#[cfg(test)]
mod tests {
    use super::{GraphInvalidation, GraphInvalidationCause, InvalidationTarget};
    use crate::ids::{EdgeId, NodeId, PortId};

    #[test]
    fn helper_methods_mark_expected_causes_and_targets() {
        let mut invalidation = GraphInvalidation::new();
        let node = NodeId::from_parts(4, 0);
        let port = PortId::from_parts(7, 0);
        let edge = EdgeId::from_parts(9, 0);

        assert!(invalidation.mark_graph_node(node));
        assert!(invalidation.mark_projection_port(port));
        assert!(invalidation.mark_routing_edge(edge));
        assert!(invalidation.mark_viewport());

        assert_eq!(
            invalidation
                .iter(GraphInvalidationCause::Graph)
                .collect::<alloc::vec::Vec<_>>(),
            alloc::vec![InvalidationTarget::Node(node)]
        );
        assert_eq!(
            invalidation
                .iter(GraphInvalidationCause::Projection)
                .collect::<alloc::vec::Vec<_>>(),
            alloc::vec![InvalidationTarget::Port(port)]
        );
        assert_eq!(
            invalidation
                .iter(GraphInvalidationCause::Routing)
                .collect::<alloc::vec::Vec<_>>(),
            alloc::vec![InvalidationTarget::Edge(edge)]
        );
        assert_eq!(
            invalidation
                .iter(GraphInvalidationCause::Viewport)
                .collect::<alloc::vec::Vec<_>>(),
            alloc::vec![InvalidationTarget::Viewport]
        );
    }
}
