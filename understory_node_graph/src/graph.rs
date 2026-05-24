// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Semantic graph document state.

use alloc::vec::Vec;

use crate::arena::Arena;
use crate::compatibility::{ConnectionContext, PortCompatibility};
use crate::ids::{EdgeId, NodeId, PortId};
use crate::revision::Revision;

/// Semantic payload for one durable node.
///
/// A `NodeData` value lives in [`GraphDoc`] and represents application meaning:
/// a shader operation, behavior-tree task, audio unit, or other domain node.
/// The node's position, collapsed state, and draw metadata live in
/// [`GraphProjection`](crate::GraphProjection) instead, so one document can be
/// shown by multiple views.
#[derive(Clone, Debug, Default)]
pub struct NodeData<M = ()> {
    /// Host-defined semantic metadata for this node.
    ///
    /// The crate never interprets this value. Use it for stable domain data
    /// such as node kind, labels, type information, or ids into a larger model.
    pub meta: M,
}

/// Port direction within the semantic graph.
///
/// Directions are the only built-in connection rule: edges are always inserted
/// from an [`Output`](Self::Output) port to an [`Input`](Self::Input) port.
/// Domain-specific compatibility, such as value types or "only one edge per
/// input", belongs in a [`PortCompatibility`](crate::PortCompatibility) policy.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PortDirection {
    /// Input/consumer side.
    Input,
    /// Output/producer side.
    Output,
}

/// Semantic payload for one durable port.
///
/// Ports belong to nodes and are the only valid edge endpoints. Hosts typically
/// use port metadata to describe socket names, value types, or whether the port
/// is optional. Visual placement is derived from the owning node's
/// [`NodeView`](crate::NodeView) plus optional [`PortView`](crate::PortView)
/// offsets.
#[derive(Clone, Debug)]
pub struct PortData<M = ()> {
    /// Owning node.
    pub owner: NodeId,
    /// Port direction.
    pub direction: PortDirection,
    /// Host-defined semantic metadata for this port.
    pub meta: M,
}

/// Semantic payload for one durable edge.
///
/// An edge records a connection from an output port to an input port. Routing,
/// visibility, hit testing, and drawing order are projection/computed concerns;
/// this type only stores the semantic connection and host metadata.
#[derive(Clone, Debug)]
pub struct EdgeData<M = ()> {
    /// Source/output port.
    pub output: PortId,
    /// Destination/input port.
    pub input: PortId,
    /// Host-defined semantic metadata for this edge.
    pub meta: M,
}

#[derive(Clone, Debug)]
struct NodeRecord<M> {
    data: NodeData<M>,
    ports: Vec<PortId>,
}

#[derive(Clone, Debug)]
struct PortRecord<M> {
    data: PortData<M>,
    edges: Vec<EdgeId>,
}

/// Error returned when the semantic endpoint checks for a connection fail.
///
/// These errors are about graph validity, not host policy. If the endpoints
/// exist and have the right directions, a [`PortCompatibility`](crate::PortCompatibility)
/// policy can still reject the connection without producing a `ConnectError`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ConnectError {
    /// A requested port does not exist.
    MissingPort(PortId),
    /// Edge endpoints must run from output to input.
    DirectionMismatch,
}

/// Durable semantic graph document.
///
/// `GraphDoc` owns node/port/edge topology and lightweight host metadata. It
/// intentionally knows nothing about positions, bounds, or interaction state.
///
/// Use this as the long-lived model that can be saved, diffed, tested, and
/// shared between views. Pair it with one or more
/// [`GraphProjection`](crate::GraphProjection) values for layout/presentation,
/// [`GraphSession`](crate::GraphSession) values for active interaction state,
/// and [`GraphComputed`](crate::GraphComputed) values for derived geometry.
///
/// The generic parameters are host metadata types for nodes, ports, and edges.
/// Use `()` when the graph only needs topology.
#[derive(Clone, Debug)]
pub struct GraphDoc<N = (), P = (), E = ()> {
    nodes: Arena<NodeId, NodeRecord<N>>,
    ports: Arena<PortId, PortRecord<P>>,
    edges: Arena<EdgeId, EdgeData<E>>,
    revision: Revision,
}

impl<N, P, E> Default for GraphDoc<N, P, E> {
    fn default() -> Self {
        Self {
            nodes: Arena::new(),
            ports: Arena::new(),
            edges: Arena::new(),
            revision: Revision::new(),
        }
    }
}

impl<N, P, E> GraphDoc<N, P, E> {
    /// Creates an empty graph document.
    ///
    /// A fresh document has no projection data. After inserting nodes or ports,
    /// add matching views to a [`GraphProjection`](crate::GraphProjection)
    /// before expecting them to appear in computed geometry or hit tests.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current document revision.
    ///
    /// The revision changes after every successful topology or metadata
    /// mutation. [`GraphComputed`](crate::GraphComputed) uses it to decide
    /// whether semantic geometry may need to be rebuilt.
    #[must_use]
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Returns the number of live nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the number of live ports.
    #[must_use]
    pub fn port_count(&self) -> usize {
        self.ports.len()
    }

    /// Returns the number of live edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Returns `true` if `node` is live.
    #[must_use]
    pub fn contains_node(&self, node: NodeId) -> bool {
        self.nodes.contains(node)
    }

    /// Returns `true` if `port` is live.
    #[must_use]
    pub fn contains_port(&self, port: PortId) -> bool {
        self.ports.contains(port)
    }

    /// Returns `true` if `edge` is live.
    #[must_use]
    pub fn contains_edge(&self, edge: EdgeId) -> bool {
        self.edges.contains(edge)
    }

    /// Inserts a new node and returns its stable document id.
    ///
    /// The returned [`NodeId`] is valid until that node is removed. If a slot is
    /// later reused, the generation changes so stale ids do not refer to the new
    /// node.
    pub fn add_node(&mut self, data: NodeData<N>) -> NodeId {
        self.revision.bump();
        self.nodes.insert(NodeRecord {
            data,
            ports: Vec::new(),
        })
    }

    /// Removes a node and cascades through its ports and connected edges.
    ///
    /// This keeps the semantic graph internally consistent: no live port or
    /// edge will keep pointing at the removed node. Projection entries are not
    /// removed automatically, so hosts should also clear related
    /// [`GraphProjection`](crate::GraphProjection) views or mark them stale.
    pub fn remove_node(&mut self, node: NodeId) -> Option<NodeData<N>> {
        let ports = self.node_ports(node)?.to_vec();
        for port in ports {
            let _ = self.remove_port(port);
        }
        let removed = self.nodes.remove(node)?;
        self.revision.bump();
        Some(removed.data)
    }

    /// Inserts a new port under `owner`.
    ///
    /// Returns `None` when `owner` is not a live node. The port is appended to
    /// the owner's port list; anchor placement later uses that list order to
    /// distribute input and output ports along the node edge.
    pub fn add_port(&mut self, owner: NodeId, direction: PortDirection, meta: P) -> Option<PortId> {
        let node = self.nodes.get_mut(owner)?;
        let port = self.ports.insert(PortRecord {
            data: PortData {
                owner,
                direction,
                meta,
            },
            edges: Vec::new(),
        });
        node.ports.push(port);
        self.revision.bump();
        Some(port)
    }

    /// Removes a port and all connected edges.
    ///
    /// The port is also removed from its owning node's port list. Projection
    /// data is left alone so hosts can decide whether to reuse, animate out, or
    /// discard view state.
    pub fn remove_port(&mut self, port: PortId) -> Option<PortData<P>> {
        let edge_ids = self.port_edges(port)?.to_vec();
        for edge in edge_ids {
            let _ = self.remove_edge(edge);
        }
        let record = self.ports.remove(port)?;
        if let Some(node) = self.nodes.get_mut(record.data.owner) {
            node.ports.retain(|candidate| *candidate != port);
        }
        self.revision.bump();
        Some(record.data)
    }

    /// Inserts an edge from `output` to `input`.
    ///
    /// This enforces only the built-in semantic rule that `output` is an output
    /// port and `input` is an input port. Use [`GraphDoc::add_edge_with`] when a
    /// host policy should reject duplicate, incompatible, or otherwise invalid
    /// domain connections.
    pub fn add_edge(
        &mut self,
        output: PortId,
        input: PortId,
        meta: E,
    ) -> Result<EdgeId, ConnectError> {
        self.validate_connect_direction(output, input)?;
        let edge = self.edges.insert(EdgeData {
            output,
            input,
            meta,
        });
        self.ports
            .get_mut(output)
            .expect("checked above")
            .edges
            .push(edge);
        self.ports
            .get_mut(input)
            .expect("checked above")
            .edges
            .push(edge);
        self.revision.bump();
        Ok(edge)
    }

    /// Returns whether `output` may connect to `input` under `policy`.
    ///
    /// This first enforces the base semantic constraint that connections run
    /// from output ports to input ports. If that succeeds, the host policy may
    /// reject the connection for domain-specific reasons.
    pub fn can_connect_with<C>(
        &self,
        output: PortId,
        input: PortId,
        policy: &C,
    ) -> Result<bool, ConnectError>
    where
        C: PortCompatibility<N, P, E>,
    {
        let (output_port, input_port) = self.connect_endpoints(output, input)?;
        Ok(policy.can_connect(ConnectionContext::new(
            self,
            output,
            input,
            output_port,
            input_port,
        )))
    }

    /// Inserts an edge only when `policy` allows the connection.
    ///
    /// Returns:
    /// - `Ok(Some(edge))` when the edge was inserted,
    /// - `Ok(None)` when the connection was directionally valid but rejected by `policy`,
    /// - `Err(...)` for missing ports or invalid directions.
    pub fn add_edge_with<C>(
        &mut self,
        output: PortId,
        input: PortId,
        meta: E,
        policy: &C,
    ) -> Result<Option<EdgeId>, ConnectError>
    where
        C: PortCompatibility<N, P, E>,
    {
        if !self.can_connect_with(output, input, policy)? {
            return Ok(None);
        }
        self.add_edge(output, input, meta).map(Some)
    }

    /// Removes an edge and detaches it from both endpoint port edge lists.
    pub fn remove_edge(&mut self, edge: EdgeId) -> Option<EdgeData<E>> {
        let removed = self.edges.remove(edge)?;
        if let Some(output) = self.ports.get_mut(removed.output) {
            output.edges.retain(|candidate| *candidate != edge);
        }
        if let Some(input) = self.ports.get_mut(removed.input) {
            input.edges.retain(|candidate| *candidate != edge);
        }
        self.revision.bump();
        Some(removed)
    }

    /// Returns immutable semantic data for `node`.
    #[must_use]
    pub fn node(&self, node: NodeId) -> Option<&NodeData<N>> {
        self.nodes.get(node).map(|record| &record.data)
    }

    /// Returns immutable semantic data for `port`.
    #[must_use]
    pub fn port(&self, port: PortId) -> Option<&PortData<P>> {
        self.ports.get(port).map(|record| &record.data)
    }

    /// Returns immutable semantic data for `edge`.
    #[must_use]
    pub fn edge(&self, edge: EdgeId) -> Option<&EdgeData<E>> {
        self.edges.get(edge)
    }

    /// Mutates host-defined node metadata and bumps the document revision.
    ///
    /// This is the intended way to edit node metadata in place without exposing
    /// topology internals. Returns `None` and leaves the revision unchanged when
    /// `node` is not live.
    pub fn update_node_meta<R, F>(&mut self, node: NodeId, update: F) -> Option<R>
    where
        F: FnOnce(&mut N) -> R,
    {
        let record = self.nodes.get_mut(node)?;
        let result = update(&mut record.data.meta);
        self.revision.bump();
        Some(result)
    }

    /// Mutates host-defined port metadata and bumps the document revision.
    ///
    /// This preserves the port owner, direction, and edge list while letting the
    /// host update its own semantic payload. Returns `None` and leaves the
    /// revision unchanged when `port` is not live.
    pub fn update_port_meta<R, F>(&mut self, port: PortId, update: F) -> Option<R>
    where
        F: FnOnce(&mut P) -> R,
    {
        let record = self.ports.get_mut(port)?;
        let result = update(&mut record.data.meta);
        self.revision.bump();
        Some(result)
    }

    /// Mutates host-defined edge metadata and bumps the document revision.
    ///
    /// This preserves the edge endpoints while letting the host update its own
    /// semantic payload. Returns `None` and leaves the revision unchanged when
    /// `edge` is not live.
    pub fn update_edge_meta<R, F>(&mut self, edge: EdgeId, update: F) -> Option<R>
    where
        F: FnOnce(&mut E) -> R,
    {
        let edge_data = self.edges.get_mut(edge)?;
        let result = update(&mut edge_data.meta);
        self.revision.bump();
        Some(result)
    }

    /// Returns the ports owned by `node` in insertion order.
    ///
    /// Port anchor derivation uses this order separately for input and output
    /// ports, so changing insertion order changes default visual placement.
    #[must_use]
    pub fn node_ports(&self, node: NodeId) -> Option<&[PortId]> {
        self.nodes.get(node).map(|record| record.ports.as_slice())
    }

    /// Returns the edges touching `port` in insertion order.
    #[must_use]
    pub fn port_edges(&self, port: PortId) -> Option<&[EdgeId]> {
        self.ports.get(port).map(|record| record.edges.as_slice())
    }

    /// Iterates over live nodes in arena slot order.
    ///
    /// The order is stable while the same ids remain live, but it is a storage
    /// order rather than an application sort order.
    pub fn iter_nodes(&self) -> impl Iterator<Item = (NodeId, &NodeData<N>)> {
        self.nodes.iter().map(|(id, record)| (id, &record.data))
    }

    /// Iterates over live ports in arena slot order.
    pub fn iter_ports(&self) -> impl Iterator<Item = (PortId, &PortData<P>)> {
        self.ports.iter().map(|(id, record)| (id, &record.data))
    }

    /// Iterates over live edges in arena slot order.
    pub fn iter_edges(&self) -> impl Iterator<Item = (EdgeId, &EdgeData<E>)> {
        self.edges.iter()
    }

    fn validate_connect_direction(
        &self,
        output: PortId,
        input: PortId,
    ) -> Result<(), ConnectError> {
        let _ = self.connect_endpoints(output, input)?;
        Ok(())
    }

    fn connect_endpoints(
        &self,
        output: PortId,
        input: PortId,
    ) -> Result<(&PortData<P>, &PortData<P>), ConnectError> {
        let output_port = self.port(output).ok_or(ConnectError::MissingPort(output))?;
        let input_port = self.port(input).ok_or(ConnectError::MissingPort(input))?;
        if output_port.direction != PortDirection::Output
            || input_port.direction != PortDirection::Input
        {
            return Err(ConnectError::DirectionMismatch);
        }
        Ok((output_port, input_port))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removing_node_prunes_ports_and_edges() {
        let mut doc = GraphDoc::<(), (), ()>::new();
        let a = doc.add_node(NodeData { meta: () });
        let b = doc.add_node(NodeData { meta: () });
        let out = doc.add_port(a, PortDirection::Output, ()).unwrap();
        let input = doc.add_port(b, PortDirection::Input, ()).unwrap();
        let edge = doc.add_edge(out, input, ()).unwrap();

        let removed = doc.remove_node(a);
        assert!(removed.is_some());
        assert!(!doc.contains_node(a));
        assert!(!doc.contains_port(out));
        assert!(!doc.contains_edge(edge));
        assert!(doc.contains_node(b));
        assert!(doc.contains_port(input));
    }

    #[test]
    fn compatibility_policy_can_reject_connections() {
        struct RejectAll;

        impl<N, P, E> PortCompatibility<N, P, E> for RejectAll {
            fn can_connect(&self, _cx: ConnectionContext<'_, N, P, E>) -> bool {
                false
            }
        }

        let mut doc = GraphDoc::<(), (), ()>::new();
        let a = doc.add_node(NodeData { meta: () });
        let b = doc.add_node(NodeData { meta: () });
        let out = doc.add_port(a, PortDirection::Output, ()).unwrap();
        let input = doc.add_port(b, PortDirection::Input, ()).unwrap();

        assert_eq!(doc.can_connect_with(out, input, &RejectAll), Ok(false));
        assert_eq!(doc.add_edge_with(out, input, (), &RejectAll), Ok(None));
        assert_eq!(doc.edge_count(), 0);
    }

    #[test]
    fn compatibility_policy_can_inspect_existing_topology() {
        struct NoDuplicateEdges;
        struct SingleInputNoDuplicate;

        impl<N, P, E> PortCompatibility<N, P, E> for NoDuplicateEdges {
            fn can_connect(&self, cx: ConnectionContext<'_, N, P, E>) -> bool {
                cx.duplicate_edge().is_none()
            }
        }

        impl<N, P, E> PortCompatibility<N, P, E> for SingleInputNoDuplicate {
            fn can_connect(&self, cx: ConnectionContext<'_, N, P, E>) -> bool {
                cx.input_edges().is_empty() && cx.duplicate_edge().is_none()
            }
        }

        let mut doc = GraphDoc::<(), (), ()>::new();
        let first_source = doc.add_node(NodeData { meta: () });
        let second_source = doc.add_node(NodeData { meta: () });
        let sink = doc.add_node(NodeData { meta: () });
        let first_out = doc
            .add_port(first_source, PortDirection::Output, ())
            .unwrap();
        let second_out = doc
            .add_port(second_source, PortDirection::Output, ())
            .unwrap();
        let input = doc.add_port(sink, PortDirection::Input, ()).unwrap();

        let edge = doc
            .add_edge_with(first_out, input, (), &NoDuplicateEdges)
            .unwrap()
            .expect("first edge is not a duplicate");
        assert_eq!(
            doc.add_edge_with(first_out, input, (), &NoDuplicateEdges),
            Ok(None),
            "exact duplicate is rejected"
        );
        let second_edge = doc
            .add_edge_with(second_out, input, (), &NoDuplicateEdges)
            .unwrap()
            .expect("same input can still accept a distinct edge under this policy");
        assert_eq!(doc.port_edges(input), Some(&[edge, second_edge][..]));

        let single_input = doc.add_port(sink, PortDirection::Input, ()).unwrap();
        let single_edge = doc
            .add_edge_with(first_out, single_input, (), &SingleInputNoDuplicate)
            .unwrap()
            .expect("empty input accepts first edge");
        assert_eq!(
            doc.add_edge_with(second_out, single_input, (), &SingleInputNoDuplicate),
            Ok(None),
            "occupied input is rejected"
        );
        assert_eq!(doc.port_edges(single_input), Some(&[single_edge][..]));
    }

    #[test]
    fn metadata_updates_bump_revision_without_exposing_topology_mutation() {
        let mut doc = GraphDoc::<&'static str, &'static str, &'static str>::new();
        let source = doc.add_node(NodeData { meta: "source" });
        let sink = doc.add_node(NodeData { meta: "sink" });
        let output = doc.add_port(source, PortDirection::Output, "out").unwrap();
        let input = doc.add_port(sink, PortDirection::Input, "in").unwrap();
        let edge = doc.add_edge(output, input, "edge").unwrap();

        let before = doc.revision();
        let old_node = doc
            .update_node_meta(source, |meta| {
                let old = *meta;
                *meta = "renamed source";
                old
            })
            .expect("node exists");
        assert_eq!(old_node, "source");
        assert!(doc.revision() > before);
        assert_eq!(doc.node(source).unwrap().meta, "renamed source");

        let old_port = doc
            .update_port_meta(output, |meta| {
                let old = *meta;
                *meta = "renamed out";
                old
            })
            .expect("port exists");
        assert_eq!(old_port, "out");
        let output_data = doc.port(output).unwrap();
        assert_eq!(output_data.owner, source);
        assert_eq!(output_data.direction, PortDirection::Output);
        assert_eq!(output_data.meta, "renamed out");

        let old_edge = doc
            .update_edge_meta(edge, |meta| {
                let old = *meta;
                *meta = "renamed edge";
                old
            })
            .expect("edge exists");
        assert_eq!(old_edge, "edge");
        let edge_data = doc.edge(edge).unwrap();
        assert_eq!(edge_data.output, output);
        assert_eq!(edge_data.input, input);
        assert_eq!(edge_data.meta, "renamed edge");

        let _ = doc.remove_edge(edge);
        let after_remove = doc.revision();
        assert_eq!(doc.update_edge_meta(edge, |_| ()), None);
        assert_eq!(doc.revision(), after_remove);
    }
}
