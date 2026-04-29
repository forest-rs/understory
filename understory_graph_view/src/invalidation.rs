// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Graph-view invalidation channels and tracked targets.

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
/// Hit-test surface changed.
pub const HIT_TEST: Channel = Channel::new(6);

/// Typed invalidation cause for graph-view systems.
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
    /// Hit-test changes.
    HitTest,
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
            Self::HitTest => HIT_TEST,
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
    /// A node.
    Node(NodeId),
    /// A port.
    Port(PortId),
    /// An edge.
    Edge(EdgeId),
}

/// Coarse invalidation state for graph-view derived caches.
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
