// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Stable graph entity identifiers.

use core::convert::TryFrom;
use core::fmt;

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            pub(crate) const fn from_parts(index: u32, generation: u32) -> Self {
                Self(((generation as u64) << 32) | index as u64)
            }

            pub(crate) fn index(self) -> u32 {
                u32::try_from(self.0 & u64::from(u32::MAX)).expect("masked to 32 bits")
            }

            pub(crate) fn generation(self) -> u32 {
                u32::try_from(self.0 >> 32).expect("generation fits in 32 bits")
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&self.index())
                    .field(&self.generation())
                    .finish()
            }
        }
    };
}

define_id!(
    /// Stable identity for a node in a graph document.
    NodeId
);
define_id!(
    /// Stable identity for a port in a graph document.
    PortId
);
define_id!(
    /// Stable identity for an edge in a graph document.
    EdgeId
);

/// Stable identity for graph entities.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum GraphElementId {
    /// A node.
    Node(NodeId),
    /// A port.
    Port(PortId),
    /// An edge.
    Edge(EdgeId),
}

impl From<NodeId> for GraphElementId {
    fn from(value: NodeId) -> Self {
        Self::Node(value)
    }
}

impl From<PortId> for GraphElementId {
    fn from(value: PortId) -> Self {
        Self::Port(value)
    }
}

impl From<EdgeId> for GraphElementId {
    fn from(value: EdgeId) -> Self {
        Self::Edge(value)
    }
}

pub(crate) trait ArenaId: Copy {
    fn from_parts(index: u32, generation: u32) -> Self;
    fn index(self) -> u32;
    fn generation(self) -> u32;
}

impl ArenaId for NodeId {
    fn from_parts(index: u32, generation: u32) -> Self {
        Self::from_parts(index, generation)
    }

    fn index(self) -> u32 {
        self.index()
    }

    fn generation(self) -> u32 {
        self.generation()
    }
}

impl ArenaId for PortId {
    fn from_parts(index: u32, generation: u32) -> Self {
        Self::from_parts(index, generation)
    }

    fn index(self) -> u32 {
        self.index()
    }

    fn generation(self) -> u32 {
        self.generation()
    }
}

impl ArenaId for EdgeId {
    fn from_parts(index: u32, generation: u32) -> Self {
        Self::from_parts(index, generation)
    }

    fn index(self) -> u32 {
        self.index()
    }

    fn generation(self) -> u32 {
        self.generation()
    }
}
