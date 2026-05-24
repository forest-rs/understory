// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Graph element targeting.

use crate::{EdgeId, GraphElementId, NodeId, PortId};

/// Concrete hit target in a graph view.
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
    #[must_use]
    pub fn element_id(self) -> GraphElementId {
        match self {
            Self::Node(id) => id.into(),
            Self::Port(id) => id.into(),
            Self::Edge(id) => id.into(),
        }
    }
}
