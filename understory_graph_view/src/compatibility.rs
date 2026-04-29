// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Host-defined port compatibility policies.

use crate::graph::PortData;

/// Policy for deciding whether an output port may connect to an input port.
///
/// The semantic graph owns durable port metadata, but the meaning of that
/// metadata is application-defined. `PortCompatibility` lets hosts supply a
/// small policy object for connection preview and edge insertion without
/// forcing a particular type system into the crate.
pub trait PortCompatibility<P> {
    /// Returns `true` if `output` may connect to `input`.
    fn can_connect(&self, output: &PortData<P>, input: &PortData<P>) -> bool;
}

/// Compatibility policy that accepts any directionally valid connection.
#[derive(Copy, Clone, Debug, Default)]
pub struct AllowAllPortConnections;

impl<P> PortCompatibility<P> for AllowAllPortConnections {
    fn can_connect(&self, _output: &PortData<P>, _input: &PortData<P>) -> bool {
        true
    }
}
