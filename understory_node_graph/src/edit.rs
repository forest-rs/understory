// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Scoped mutation surfaces that cannot bypass change collection.

use crate::{
    ConnectError, EdgeData, EdgeId, EdgeView, GraphDoc, GraphElementId, GraphProjection, NodeData,
    NodeId, NodeView, PortCompatibility, PortData, PortDirection, PortId, PortView,
};
use alloc::vec::Vec;
use kurbo::{Point, Vec2};

/// Scoped graph mutations collected by [`GraphDoc::edit`].
/// Existing untracked mutation methods remain available on `GraphDoc` and cause
/// a full derive whenever a receipt does not cover their revision transition.
#[derive(Debug)]
pub struct GraphEdit<'a, N, P, E> {
    pub(crate) doc: &'a mut GraphDoc<N, P, E>,
    pub(crate) targets: Vec<GraphElementId>,
}

impl<N, P, E> GraphEdit<'_, N, P, E> {
    /// Inspects the document being edited without exposing untracked mutations.
    #[must_use]
    pub fn document(&self) -> &GraphDoc<N, P, E> {
        self.doc
    }
    /// Inserts a node.
    pub fn add_node(&mut self, data: NodeData<N>) -> NodeId {
        let id = self.doc.add_node(data);
        self.targets.push(id.into());
        id
    }
    /// Inserts a port and invalidates its owner's distributed anchors.
    pub fn add_port(&mut self, owner: NodeId, direction: PortDirection, meta: P) -> Option<PortId> {
        let id = self.doc.add_port(owner, direction, meta)?;
        self.targets
            .extend([GraphElementId::Node(owner), GraphElementId::Port(id)]);
        Some(id)
    }
    /// Inserts an edge after ordinary endpoint checks.
    pub fn add_edge(
        &mut self,
        output: PortId,
        input: PortId,
        meta: E,
    ) -> Result<EdgeId, ConnectError> {
        let id = self.doc.add_edge(output, input, meta)?;
        self.targets.push(id.into());
        Ok(id)
    }
    /// Inserts an edge only when the host's connection policy permits it.
    pub fn add_edge_with<C: PortCompatibility<N, P, E>>(
        &mut self,
        output: PortId,
        input: PortId,
        meta: E,
        policy: &C,
    ) -> Result<Option<EdgeId>, ConnectError> {
        if !self.doc.can_connect_with(output, input, policy)? {
            return Ok(None);
        }
        self.add_edge(output, input, meta).map(Some)
    }
    /// Removes a node, recording retired ports and edges before cascading removal.
    pub fn remove_node(&mut self, node: NodeId) -> Option<NodeData<N>> {
        let ports = self.doc.node_ports(node)?.to_vec();
        for port in ports {
            self.removing_port(port);
        }
        let data = self.doc.remove_node(node)?;
        self.targets.push(node.into());
        Some(data)
    }
    fn removing_port(&mut self, port: PortId) {
        if let Some(data) = self.doc.port(port) {
            self.targets
                .extend([GraphElementId::Node(data.owner), GraphElementId::Port(port)]);
            self.targets.extend(
                self.doc
                    .port_edges(port)
                    .unwrap_or(&[])
                    .iter()
                    .map(|id| GraphElementId::Edge(*id)),
            );
        }
    }
    /// Removes a port and its edges, including changes to sibling anchor placement.
    pub fn remove_port(&mut self, port: PortId) -> Option<PortData<P>> {
        self.removing_port(port);
        self.doc.remove_port(port)
    }
    /// Removes an edge.
    pub fn remove_edge(&mut self, edge: EdgeId) -> Option<EdgeData<E>> {
        let data = self.doc.remove_edge(edge)?;
        self.targets.push(edge.into());
        Some(data)
    }
    /// Updates node metadata conservatively as a node-local geometry change.
    pub fn update_node_meta<R>(
        &mut self,
        node: NodeId,
        update: impl FnOnce(&mut N) -> R,
    ) -> Option<R> {
        self.targets.push(node.into());
        let result = self.doc.update_node_meta(node, update)?;
        Some(result)
    }
    /// Updates port metadata conservatively as a port-local geometry change.
    pub fn update_port_meta<R>(
        &mut self,
        port: PortId,
        update: impl FnOnce(&mut P) -> R,
    ) -> Option<R> {
        self.targets.push(port.into());
        let result = self.doc.update_port_meta(port, update)?;
        Some(result)
    }
    /// Updates edge metadata conservatively as an edge-local routing change.
    pub fn update_edge_meta<R>(
        &mut self,
        edge: EdgeId,
        update: impl FnOnce(&mut E) -> R,
    ) -> Option<R> {
        self.targets.push(edge.into());
        let result = self.doc.update_edge_meta(edge, update)?;
        Some(result)
    }
}

/// Scoped view mutations collected by [`GraphProjection::edit`].
#[derive(Debug)]
pub struct ProjectionEdit<'a, N, P, E> {
    pub(crate) projection: &'a mut GraphProjection<N, P, E>,
    pub(crate) targets: Vec<GraphElementId>,
}

impl<N, P, E> ProjectionEdit<'_, N, P, E> {
    /// Inspects the projection without exposing untracked mutations.
    #[must_use]
    pub fn projection(&self) -> &GraphProjection<N, P, E> {
        self.projection
    }
    /// Inserts or replaces a node's presentation.
    pub fn set_node_view(&mut self, node: NodeId, view: NodeView<N>) -> Option<NodeView<N>> {
        self.targets.push(node.into());
        self.projection.set_node_view(node, view)
    }
    /// Changes a node's presentation.
    pub fn update_node_view<R>(
        &mut self,
        node: NodeId,
        update: impl FnOnce(&mut NodeView<N>) -> R,
    ) -> Option<R> {
        self.targets.push(node.into());
        let result = self.projection.update_node_view(node, update)?;
        Some(result)
    }
    /// Moves a node to a world-space origin.
    pub fn set_node_origin(&mut self, node: NodeId, origin: Point) -> Option<Point> {
        self.update_node_view(node, |view| core::mem::replace(&mut view.origin, origin))
    }
    /// Moves a node by a world-space displacement.
    pub fn translate_node(&mut self, node: NodeId, delta: Vec2) -> bool {
        self.update_node_view(node, |view| view.origin += delta)
            .is_some()
    }
    /// Inserts or replaces a port's presentation.
    pub fn set_port_view(&mut self, port: PortId, view: PortView<P>) -> Option<PortView<P>> {
        self.targets.push(port.into());
        self.projection.set_port_view(port, view)
    }
    /// Inserts or replaces an edge's presentation.
    pub fn set_edge_view(&mut self, edge: EdgeId, view: EdgeView<E>) -> Option<EdgeView<E>> {
        self.targets.push(edge.into());
        self.projection.set_edge_view(edge, view)
    }
    /// Removes a node's presentation and its realized anchors and routes.
    pub fn remove_node_view(&mut self, node: NodeId) -> Option<NodeView<N>> {
        let data = self.projection.remove_node_view(node)?;
        self.targets.push(node.into());
        Some(data)
    }
    /// Restores a port's automatic presentation defaults.
    pub fn remove_port_view(&mut self, port: PortId) -> Option<PortView<P>> {
        let data = self.projection.remove_port_view(port)?;
        self.targets.push(port.into());
        Some(data)
    }
    /// Restores an edge's default presentation.
    pub fn remove_edge_view(&mut self, edge: EdgeId) -> Option<EdgeView<E>> {
        let data = self.projection.remove_edge_view(edge)?;
        self.targets.push(edge.into());
        Some(data)
    }
}
