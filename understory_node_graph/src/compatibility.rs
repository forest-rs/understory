// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Host-defined port compatibility policies.

use core::fmt;

use crate::graph::{EdgeData, GraphDoc, PortData};
use crate::ids::{EdgeId, PortId};

/// Context passed to a host connection policy.
///
/// The semantic graph enforces that connections run from output ports to input
/// ports before this context is constructed. Host policies can then inspect
/// endpoint metadata and existing topology to apply domain rules such as type
/// compatibility, single-input ports, duplicate-edge rejection, or cycle
/// prevention.
///
/// This context is read-only by design. A policy answers "may this connection
/// exist?" while [`GraphDoc::add_edge_with`](crate::GraphDoc::add_edge_with)
/// remains responsible for actually mutating the graph.
pub struct ConnectionContext<'a, N, P, E> {
    doc: &'a GraphDoc<N, P, E>,
    output: PortId,
    input: PortId,
    output_port: &'a PortData<P>,
    input_port: &'a PortData<P>,
}

impl<N, P, E> fmt::Debug for ConnectionContext<'_, N, P, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectionContext")
            .field("output", &self.output)
            .field("input", &self.input)
            .finish_non_exhaustive()
    }
}

impl<'a, N, P, E> ConnectionContext<'a, N, P, E> {
    pub(crate) fn new(
        doc: &'a GraphDoc<N, P, E>,
        output: PortId,
        input: PortId,
        output_port: &'a PortData<P>,
        input_port: &'a PortData<P>,
    ) -> Self {
        Self {
            doc,
            output,
            input,
            output_port,
            input_port,
        }
    }

    /// Returns the graph document being checked.
    ///
    /// Use this when policy decisions need broader topology or metadata than
    /// the two endpoint ports.
    #[must_use]
    pub fn doc(&self) -> &'a GraphDoc<N, P, E> {
        self.doc
    }

    /// Returns the output/source port id.
    #[must_use]
    pub fn output(&self) -> PortId {
        self.output
    }

    /// Returns the input/destination port id.
    #[must_use]
    pub fn input(&self) -> PortId {
        self.input
    }

    /// Returns the output/source port data.
    #[must_use]
    pub fn output_port(&self) -> &'a PortData<P> {
        self.output_port
    }

    /// Returns the input/destination port data.
    #[must_use]
    pub fn input_port(&self) -> &'a PortData<P> {
        self.input_port
    }

    /// Returns edges touching the output/source port.
    ///
    /// This is a convenience for policies that limit fan-out or reject exact
    /// duplicates.
    #[must_use]
    pub fn output_edges(&self) -> &'a [EdgeId] {
        self.doc.port_edges(self.output).unwrap_or(&[])
    }

    /// Returns edges touching the input/destination port.
    ///
    /// This is a convenience for policies such as "an input accepts only one
    /// connection".
    #[must_use]
    pub fn input_edges(&self) -> &'a [EdgeId] {
        self.doc.port_edges(self.input).unwrap_or(&[])
    }

    /// Returns the existing edge between these exact endpoints, if present.
    ///
    /// Use this to prevent duplicate parallel edges while still allowing a port
    /// to participate in other connections.
    #[must_use]
    pub fn duplicate_edge(&self) -> Option<(EdgeId, &'a EdgeData<E>)> {
        self.doc.edge_between(self.output, self.input)
    }
}

/// Policy for deciding whether an output port may connect to an input port.
///
/// The semantic graph owns durable port metadata, but the meaning of that
/// metadata and topology is application-defined. `PortCompatibility` lets
/// hosts supply a small policy object for connection preview and edge insertion
/// without forcing a particular type system into the crate.
pub trait PortCompatibility<N, P, E> {
    /// Returns `true` if the requested connection is allowed.
    ///
    /// This method is called only after the graph has confirmed that both ports
    /// exist and have output-to-input directions.
    fn can_connect(&self, cx: ConnectionContext<'_, N, P, E>) -> bool;
}

/// Compatibility policy that accepts any directionally valid connection.
///
/// Use this for viewers, prototypes, or graph domains where all output-to-input
/// connections are meaningful.
#[derive(Copy, Clone, Debug, Default)]
pub struct AllowAllPortConnections;

impl<N, P, E> PortCompatibility<N, P, E> for AllowAllPortConnections {
    fn can_connect(&self, _cx: ConnectionContext<'_, N, P, E>) -> bool {
        true
    }
}

/// Compatibility policy that rejects exact duplicate connections.
///
/// This still allows multiple distinct outputs to connect to the same input and
/// one output to connect to multiple distinct inputs.
#[derive(Copy, Clone, Debug, Default)]
pub struct RejectDuplicateConnections;

impl<N, P, E> PortCompatibility<N, P, E> for RejectDuplicateConnections {
    fn can_connect(&self, cx: ConnectionContext<'_, N, P, E>) -> bool {
        cx.duplicate_edge().is_none()
    }
}

/// Compatibility policy for single-input graphs without duplicate edges.
///
/// This accepts a connection only when the input port has no existing edges and
/// the exact output-to-input edge is not already present.
#[derive(Copy, Clone, Debug, Default)]
pub struct SingleInputConnections;

impl<N, P, E> PortCompatibility<N, P, E> for SingleInputConnections {
    fn can_connect(&self, cx: ConnectionContext<'_, N, P, E>) -> bool {
        cx.input_edges().is_empty() && cx.duplicate_edge().is_none()
    }
}
