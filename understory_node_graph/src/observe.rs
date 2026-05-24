// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Observer hooks for node-graph derived-state rebuilds.

use crate::invalidation::{GraphInvalidationCause, InvalidationTarget};

/// Major derived-state phases.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DerivePhase {
    /// Recompute node bounds.
    NodeBounds,
    /// Recompute port anchors.
    PortAnchors,
    /// Recompute edge routes.
    EdgeRouting,
    /// Recompute visibility sets.
    Visibility,
}

/// Lightweight derive metrics.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct DeriveMetrics {
    /// Number of nodes touched.
    pub nodes: usize,
    /// Number of ports touched.
    pub ports: usize,
    /// Number of edges touched.
    pub edges: usize,
}

/// Observer for invalidation and derived-state rebuild activity.
pub trait GraphDeriveObserver {
    /// Called when a target is invalidated.
    fn invalidated(&mut self, _cause: GraphInvalidationCause, _target: InvalidationTarget) {}

    /// Called before a derive phase begins.
    fn derive_begin(&mut self, _phase: DerivePhase) {}

    /// Called after a derive phase completes.
    fn derive_end(&mut self, _phase: DerivePhase, _metrics: DeriveMetrics) {}
}

/// No-op observer.
#[derive(Copy, Clone, Debug, Default)]
pub struct NoopGraphDeriveObserver;

impl GraphDeriveObserver for NoopGraphDeriveObserver {}
