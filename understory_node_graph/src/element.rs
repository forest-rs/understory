// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Graph element targeting.

use crate::{EdgeId, GraphElementId, NodeId, PortId};

/// Concrete hit target in a graph view.
///
/// Hit targets come from computed geometry, not directly from the semantic
/// document. They are the right shape for pointer handling because every target
/// can be converted into a durable [`GraphElementId`] for selection or focus.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum HitTarget {
    /// A node body.
    Node(NodeId),
    /// A port target.
    Port(PortId),
    /// An edge route.
    Edge(EdgeId),
}

impl HitTarget {
    /// Returns the corresponding graph element identity.
    ///
    /// Use this when a hit-test result should become selection or focus state in
    /// [`GraphSession`](crate::GraphSession).
    #[must_use]
    pub fn element_id(self) -> GraphElementId {
        match self {
            Self::Node(id) => id.into(),
            Self::Port(id) => id.into(),
            Self::Edge(id) => id.into(),
        }
    }
}
