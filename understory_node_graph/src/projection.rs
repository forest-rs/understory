// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Per-view graph projection state.

use hashbrown::HashMap;
use kurbo::{Point, Rect, Size, Vec2};

use crate::ids::{EdgeId, NodeId, PortId};
use crate::revision::Revision;

/// Per-node presentation data for one projection.
///
/// `NodeView` answers the spatial questions that [`GraphDoc`](crate::GraphDoc)
/// deliberately leaves open: where a node is, how large it is, and how it
/// should participate in drawing and hit testing for this view. Keep semantic
/// node data in [`NodeData`](crate::NodeData); use `meta` here for
/// view-specific state such as theme variants or layout hints.
#[derive(Clone, Debug)]
pub struct NodeView<M = ()> {
    /// Top-left node origin in world space.
    pub origin: Point,
    /// Node size in world units.
    pub size: Size,
    /// Draw and hit-test order hint.
    ///
    /// Larger values are considered above smaller values when hit testing node
    /// bodies. Equal values fall back to stable ids for deterministic results.
    pub z_index: i32,
    /// Whether the node is collapsed in this projection.
    ///
    /// The current computed layer still uses the node rectangle for visibility
    /// and hit testing. Hosts can use this flag to drive their own rendering and
    /// choose which child/detail UI to show.
    pub collapsed: bool,
    /// Host-defined view metadata for this projection.
    pub meta: M,
}

impl<M> NodeView<M> {
    /// Creates a node view at `origin` with `size` and host metadata.
    ///
    /// The created view has `z_index` set to `0` and is not collapsed. Adjust
    /// fields directly when a projection needs a different ordering or state.
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

    /// Returns the node rectangle in world coordinates.
    #[must_use]
    pub fn rect(&self) -> Rect {
        Rect::from_origin_size(self.origin, self.size)
    }
}

/// Per-port presentation data for one projection.
///
/// Ports inherit their default anchor placement from their owning node and
/// direction. `PortView` lets a projection nudge that anchor and choose the
/// interaction radius without changing the semantic port.
#[derive(Clone, Debug)]
pub struct PortView<M = ()> {
    /// Additive world-space offset applied after automatic anchor placement.
    pub anchor_offset: Vec2,
    /// Hit radius used by the default hit-test surface.
    ///
    /// This is in world units because hit testing is performed after converting
    /// the pointer from view space into world space.
    pub hit_radius: f64,
    /// Host-defined view metadata for this projection.
    pub meta: M,
}

impl<M> PortView<M> {
    /// Creates a port view with zero offset and a default hit radius.
    ///
    /// Use this when the default side-of-node anchor placement is sufficient
    /// and only host metadata needs to be attached.
    #[must_use]
    pub fn new(meta: M) -> Self {
        Self {
            anchor_offset: Vec2::ZERO,
            hit_radius: 8.0,
            meta,
        }
    }
}

/// Per-edge presentation data for one projection.
///
/// The semantic edge always remains in [`GraphDoc`](crate::GraphDoc). `EdgeView`
/// controls how that edge participates in one projection: whether it is hidden,
/// where it sits in hit-test ordering, and any host-defined drawing metadata.
#[derive(Clone, Debug)]
pub struct EdgeView<M = ()> {
    /// Draw and hit-test order hint.
    ///
    /// Larger values are considered above smaller values for edge hit testing.
    pub z_index: i32,
    /// Whether the edge is hidden in this projection.
    ///
    /// Hidden edges are omitted from routed edge geometry and visibility sets.
    pub hidden: bool,
    /// Host-defined view metadata for this projection.
    pub meta: M,
}

impl<M> EdgeView<M> {
    /// Creates a visible edge view with default ordering.
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
///
/// A projection is an overlay on top of [`GraphDoc`](crate::GraphDoc). It stores
/// positions, sizes, visibility choices, and view metadata for a particular
/// editor, viewer, minimap, or layout mode. Multiple projections can point at
/// the same document, and a projection can contain stale entries while a host is
/// reconciling document changes; [`GraphComputed`](crate::GraphComputed)
/// ignores entries that no longer match live document ids.
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
    ///
    /// Add node views for every document node that should produce bounds,
    /// anchors, routes, visibility, or hit-test results.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current projection revision.
    ///
    /// The revision changes after successful view insertions/removals and is
    /// used by [`GraphComputed`](crate::GraphComputed) to decide whether
    /// derived geometry may be stale.
    #[must_use]
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Sets the view state for a node and returns the previous view, if any.
    ///
    /// This does not check that the node exists in a document. That separation
    /// lets hosts build or update projections before document reconciliation.
    pub fn set_node_view(&mut self, node: NodeId, view: NodeView<NV>) -> Option<NodeView<NV>> {
        self.revision.bump();
        self.node_views.insert(node, view)
    }

    /// Sets the view state for a port and returns the previous view, if any.
    ///
    /// Port views are optional. Without one, computed anchors still exist when
    /// the owning node has a [`NodeView`], using a zero offset and the default
    /// hit radius.
    pub fn set_port_view(&mut self, port: PortId, view: PortView<PV>) -> Option<PortView<PV>> {
        self.revision.bump();
        self.port_views.insert(port, view)
    }

    /// Sets the view state for an edge and returns the previous view, if any.
    ///
    /// Edge views are optional. Without one, live edges are routed visibly with
    /// default hit ordering.
    pub fn set_edge_view(&mut self, edge: EdgeId, view: EdgeView<EV>) -> Option<EdgeView<EV>> {
        self.revision.bump();
        self.edge_views.insert(edge, view)
    }

    /// Removes a node view and returns it when present.
    ///
    /// Removing a node view also prevents that node's port anchors and edge
    /// routes from being produced on the next computed rebuild.
    pub fn remove_node_view(&mut self, node: NodeId) -> Option<NodeView<NV>> {
        let removed = self.node_views.remove(&node);
        if removed.is_some() {
            self.revision.bump();
        }
        removed
    }

    /// Removes a port view and returns it when present.
    ///
    /// The port can still have a computed anchor through default placement.
    pub fn remove_port_view(&mut self, port: PortId) -> Option<PortView<PV>> {
        let removed = self.port_views.remove(&port);
        if removed.is_some() {
            self.revision.bump();
        }
        removed
    }

    /// Removes an edge view and returns it when present.
    ///
    /// The edge can still be routed visibly if its document endpoints and port
    /// anchors are live.
    pub fn remove_edge_view(&mut self, edge: EdgeId) -> Option<EdgeView<EV>> {
        let removed = self.edge_views.remove(&edge);
        if removed.is_some() {
            self.revision.bump();
        }
        removed
    }

    /// Returns the projection data for `node`.
    #[must_use]
    pub fn node_view(&self, node: NodeId) -> Option<&NodeView<NV>> {
        self.node_views.get(&node)
    }

    /// Returns the optional projection data for `port`.
    #[must_use]
    pub fn port_view(&self, port: PortId) -> Option<&PortView<PV>> {
        self.port_views.get(&port)
    }

    /// Returns the optional projection data for `edge`.
    #[must_use]
    pub fn edge_view(&self, edge: EdgeId) -> Option<&EdgeView<EV>> {
        self.edge_views.get(&edge)
    }

    /// Iterates over stored node views.
    ///
    /// The iterator includes any stale entries still present in the projection.
    pub fn iter_node_views(&self) -> impl Iterator<Item = (NodeId, &NodeView<NV>)> {
        self.node_views.iter().map(|(id, view)| (*id, view))
    }

    /// Iterates over stored port views.
    ///
    /// The iterator includes any stale entries still present in the projection.
    pub fn iter_port_views(&self) -> impl Iterator<Item = (PortId, &PortView<PV>)> {
        self.port_views.iter().map(|(id, view)| (*id, view))
    }

    /// Iterates over stored edge views.
    ///
    /// The iterator includes any stale entries still present in the projection.
    pub fn iter_edge_views(&self) -> impl Iterator<Item = (EdgeId, &EdgeView<EV>)> {
        self.edge_views.iter().map(|(id, view)| (*id, view))
    }
}
