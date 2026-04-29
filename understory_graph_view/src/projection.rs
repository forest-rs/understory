// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Per-view graph projection state.

use hashbrown::HashMap;
use kurbo::{Point, Rect, Size, Vec2};

use crate::ids::{EdgeId, NodeId, PortId};
use crate::revision::Revision;

/// Per-node presentation data for a projection.
#[derive(Clone, Debug)]
pub struct NodeView<M = ()> {
    /// Top-left node origin in world space.
    pub origin: Point,
    /// Node size in world units.
    pub size: Size,
    /// Draw/hit order hint.
    pub z_index: i32,
    /// Whether the node is collapsed in this projection.
    pub collapsed: bool,
    /// Host-defined view metadata.
    pub meta: M,
}

impl<M> NodeView<M> {
    /// Creates a node view.
    #[must_use]
    pub fn new(origin: Point, size: Size, meta: M) -> Self {
        Self {
            origin,
            size,
            z_index: 0,
            collapsed: false,
            meta,
        }
    }

    /// Returns the node rectangle.
    #[must_use]
    pub fn rect(&self) -> Rect {
        Rect::from_origin_size(self.origin, self.size)
    }
}

/// Per-port presentation data for a projection.
#[derive(Clone, Debug)]
pub struct PortView<M = ()> {
    /// Additive offset applied after automatic anchor placement.
    pub anchor_offset: Vec2,
    /// Hit radius used by the default hit-test surface.
    pub hit_radius: f64,
    /// Host-defined view metadata.
    pub meta: M,
}

impl<M> PortView<M> {
    /// Creates a port view with zero offset and a default hit radius.
    #[must_use]
    pub fn new(meta: M) -> Self {
        Self {
            anchor_offset: Vec2::ZERO,
            hit_radius: 8.0,
            meta,
        }
    }
}

/// Per-edge presentation data for a projection.
#[derive(Clone, Debug)]
pub struct EdgeView<M = ()> {
    /// Draw/hit order hint.
    pub z_index: i32,
    /// Whether the edge is hidden in this projection.
    pub hidden: bool,
    /// Host-defined view metadata.
    pub meta: M,
}

impl<M> EdgeView<M> {
    /// Creates a visible edge view.
    #[must_use]
    pub fn new(meta: M) -> Self {
        Self {
            z_index: 0,
            hidden: false,
            meta,
        }
    }
}

/// One spatial/view projection of a graph document.
#[derive(Clone, Debug)]
pub struct GraphProjection<NV = (), PV = (), EV = ()> {
    node_views: HashMap<NodeId, NodeView<NV>>,
    port_views: HashMap<PortId, PortView<PV>>,
    edge_views: HashMap<EdgeId, EdgeView<EV>>,
    revision: Revision,
}

impl<NV, PV, EV> Default for GraphProjection<NV, PV, EV> {
    fn default() -> Self {
        Self {
            node_views: HashMap::new(),
            port_views: HashMap::new(),
            edge_views: HashMap::new(),
            revision: Revision::new(),
        }
    }
}

impl<NV, PV, EV> GraphProjection<NV, PV, EV> {
    /// Creates an empty projection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current revision.
    #[must_use]
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Sets the view state for a node.
    pub fn set_node_view(&mut self, node: NodeId, view: NodeView<NV>) -> Option<NodeView<NV>> {
        self.revision.bump();
        self.node_views.insert(node, view)
    }

    /// Sets the view state for a port.
    pub fn set_port_view(&mut self, port: PortId, view: PortView<PV>) -> Option<PortView<PV>> {
        self.revision.bump();
        self.port_views.insert(port, view)
    }

    /// Sets the view state for an edge.
    pub fn set_edge_view(&mut self, edge: EdgeId, view: EdgeView<EV>) -> Option<EdgeView<EV>> {
        self.revision.bump();
        self.edge_views.insert(edge, view)
    }

    /// Removes a node view.
    pub fn remove_node_view(&mut self, node: NodeId) -> Option<NodeView<NV>> {
        let removed = self.node_views.remove(&node);
        if removed.is_some() {
            self.revision.bump();
        }
        removed
    }

    /// Removes a port view.
    pub fn remove_port_view(&mut self, port: PortId) -> Option<PortView<PV>> {
        let removed = self.port_views.remove(&port);
        if removed.is_some() {
            self.revision.bump();
        }
        removed
    }

    /// Removes an edge view.
    pub fn remove_edge_view(&mut self, edge: EdgeId) -> Option<EdgeView<EV>> {
        let removed = self.edge_views.remove(&edge);
        if removed.is_some() {
            self.revision.bump();
        }
        removed
    }

    /// Returns a node view.
    #[must_use]
    pub fn node_view(&self, node: NodeId) -> Option<&NodeView<NV>> {
        self.node_views.get(&node)
    }

    /// Returns a port view.
    #[must_use]
    pub fn port_view(&self, port: PortId) -> Option<&PortView<PV>> {
        self.port_views.get(&port)
    }

    /// Returns an edge view.
    #[must_use]
    pub fn edge_view(&self, edge: EdgeId) -> Option<&EdgeView<EV>> {
        self.edge_views.get(&edge)
    }

    /// Iterates over node views.
    pub fn iter_node_views(&self) -> impl Iterator<Item = (NodeId, &NodeView<NV>)> {
        self.node_views.iter().map(|(id, view)| (*id, view))
    }

    /// Iterates over port views.
    pub fn iter_port_views(&self) -> impl Iterator<Item = (PortId, &PortView<PV>)> {
        self.port_views.iter().map(|(id, view)| (*id, view))
    }

    /// Iterates over edge views.
    pub fn iter_edge_views(&self) -> impl Iterator<Item = (EdgeId, &EdgeView<EV>)> {
        self.edge_views.iter().map(|(id, view)| (*id, view))
    }
}
