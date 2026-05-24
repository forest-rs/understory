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
    #[must_use]
    pub fn output_edges(&self) -> &'a [EdgeId] {
        self.doc.port_edges(self.output).unwrap_or(&[])
    }

    /// Returns edges touching the input/destination port.
    #[must_use]
    pub fn input_edges(&self) -> &'a [EdgeId] {
        self.doc.port_edges(self.input).unwrap_or(&[])
    }

    /// Returns the existing edge between these exact endpoints, if present.
    #[must_use]
    pub fn duplicate_edge(&self) -> Option<(EdgeId, &'a EdgeData<E>)> {
        self.output_edges().iter().copied().find_map(|edge| {
            let edge_data = self.doc.edge(edge)?;
            (edge_data.output == self.output && edge_data.input == self.input)
                .then_some((edge, edge_data))
        })
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
    fn can_connect(&self, cx: ConnectionContext<'_, N, P, E>) -> bool;
}

/// Compatibility policy that accepts any directionally valid connection.
#[derive(Copy, Clone, Debug, Default)]
pub struct AllowAllPortConnections;

impl<N, P, E> PortCompatibility<N, P, E> for AllowAllPortConnections {
    fn can_connect(&self, _cx: ConnectionContext<'_, N, P, E>) -> bool {
        true
    }
}
