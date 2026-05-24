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
///
/// Causes group changes by the derived work they may affect. Hosts usually use
/// the helper methods on [`GraphInvalidation`] instead of constructing causes
/// directly, but the enum is useful for diagnostics and observers.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum GraphInvalidationCause {
    /// Semantic graph changes, such as node, port, edge, or metadata edits.
    Graph,
    /// Projection changes, such as node bounds, port offsets, or edge visibility.
    Projection,
    /// Session changes, such as selection, hover, focus, or interaction state.
    Session,
    /// Viewport changes that affect visible-node and visible-edge sets.
    Viewport,
    /// Routing policy or edge route hint changes.
    Routing,
    /// Visibility policy changes not otherwise represented by projection or viewport edits.
    Visibility,
}

impl GraphInvalidationCause {
    /// Returns the coarse invalidation channel backing this typed cause.
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

/// Target of invalidation within a cause.
///
/// A broad target such as [`InvalidationTarget::Graph`] asks derived caches to
/// rebuild the relevant phase completely. More specific targets let
/// [`GraphComputed`](crate::GraphComputed) narrow geometry work when possible.
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
///
/// `GraphInvalidation` is the bridge between host mutations and
/// [`GraphComputed`](crate::GraphComputed). Revisions catch ordinary document,
/// projection, and session changes; invalidation adds intent: which subsystem
/// changed, and whether the change can be scoped to a node, port, or edge.
///
/// Hosts can mark broad causes while prototyping and move to targeted helpers
/// for hot paths later. `GraphComputed` clears the causes it consumes during a
/// successful rebuild.
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
    ///
    /// The revision is useful for instrumentation or host caches that need to
    /// detect that invalidation state changed, even if they do not inspect every
    /// target.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.set.generation()
    }

    /// Marks `target` invalid in the channel implied by `cause`.
    ///
    /// Returns `true` when this call added new invalidation state, and `false`
    /// when the same target was already marked for that cause.
    pub fn mark(&mut self, cause: GraphInvalidationCause, target: InvalidationTarget) -> bool {
        self.set.mark(target, cause.channel())
    }

    /// Marks the whole semantic graph invalid.
    ///
    /// Use this after structural edits when it is simpler or safer to rebuild
    /// all semantic-derived geometry.
    pub fn mark_graph(&mut self) -> bool {
        self.mark(GraphInvalidationCause::Graph, InvalidationTarget::Graph)
    }

    /// Marks one semantic node invalid.
    ///
    /// Use this when a node metadata edit affects only that node's derived
    /// geometry and connected edge routes.
    pub fn mark_graph_node(&mut self, node: NodeId) -> bool {
        self.mark(
            GraphInvalidationCause::Graph,
            InvalidationTarget::Node(node),
        )
    }

    /// Marks one semantic port invalid.
    ///
    /// This also lets derived geometry include the owning node and connected
    /// edges in the targeted rebuild.
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
    ///
    /// Use this after bulk layout changes or when stale projection entries may
    /// be widespread.
    pub fn mark_projection(&mut self) -> bool {
        self.mark(
            GraphInvalidationCause::Projection,
            InvalidationTarget::Projection,
        )
    }

    /// Marks one projected node invalid.
    ///
    /// Use this after moving or resizing a node view.
    pub fn mark_projection_node(&mut self, node: NodeId) -> bool {
        self.mark(
            GraphInvalidationCause::Projection,
            InvalidationTarget::Node(node),
        )
    }

    /// Marks one projected port invalid.
    ///
    /// Use this after changing a port view's anchor offset or hit radius.
    pub fn mark_projection_port(&mut self, port: PortId) -> bool {
        self.mark(
            GraphInvalidationCause::Projection,
            InvalidationTarget::Port(port),
        )
    }

    /// Marks one projected edge invalid.
    ///
    /// Use this after changing an edge view's hidden state or ordering metadata.
    pub fn mark_projection_edge(&mut self, edge: EdgeId) -> bool {
        self.mark(
            GraphInvalidationCause::Projection,
            InvalidationTarget::Edge(edge),
        )
    }

    /// Marks the whole session invalid.
    ///
    /// The session revision usually catches setter-based changes. This helper is
    /// useful after direct field mutation or external selection updates.
    pub fn mark_session(&mut self) -> bool {
        self.mark(GraphInvalidationCause::Session, InvalidationTarget::Session)
    }

    /// Marks the viewport invalid.
    ///
    /// Use this after pan, zoom, or view-size changes that affect visibility.
    pub fn mark_viewport(&mut self) -> bool {
        self.mark(
            GraphInvalidationCause::Viewport,
            InvalidationTarget::Viewport,
        )
    }

    /// Marks edge routing invalid.
    ///
    /// Use this after changing the router implementation or global route hints.
    pub fn mark_routing(&mut self) -> bool {
        self.mark(GraphInvalidationCause::Routing, InvalidationTarget::Graph)
    }

    /// Marks one routed edge invalid.
    ///
    /// Use this for per-edge routing hints when node and port anchors are still
    /// valid.
    pub fn mark_routing_edge(&mut self, edge: EdgeId) -> bool {
        self.mark(
            GraphInvalidationCause::Routing,
            InvalidationTarget::Edge(edge),
        )
    }

    /// Marks visibility invalid.
    ///
    /// Use this when host visibility policy changes without a corresponding
    /// viewport or projection revision change.
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
    ///
    /// Observers and diagnostics can use this to explain why a derive phase
    /// ran. The iterator yields copied target values.
    pub fn iter(
        &self,
        cause: GraphInvalidationCause,
    ) -> impl Iterator<Item = InvalidationTarget> + '_ {
        self.set.iter(cause.channel())
    }

    /// Clears one invalidation cause.
    ///
    /// Most callers should let [`GraphComputed`](crate::GraphComputed) clear
    /// causes after it consumes them.
    pub fn clear(&mut self, cause: GraphInvalidationCause) {
        self.set.clear(cause.channel());
    }

    /// Clears all invalidation causes.
    ///
    /// This is mainly useful when discarding a computed cache or intentionally
    /// synchronizing from scratch.
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
