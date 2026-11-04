// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Observer hooks for node-graph derived-state rebuilds.

use crate::invalidation::{GraphInvalidationCause, InvalidationTarget};

/// Major derived-state phases.
///
/// These are the coarse units that [`GraphComputed`](crate::GraphComputed)
/// reports to observers. They are intentionally higher level than individual
/// cache tables so hosts can log or profile rebuild work without coupling to
/// internal storage.
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
    /// Recompute the transient edge-creation preview.
    EdgePreview,
}

/// Whether a phase scans its complete input or a covered neighborhood.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum DeriveMode {
    /// Complete derivation of this phase's input.
    #[default]
    Full,
    /// Derivation of the affected neighborhood only.
    Targeted,
}

/// Live graph population, distinct from work performed in a derive phase.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphSize {
    /// Live document nodes.
    pub nodes: usize,
    /// Live document ports.
    pub ports: usize,
    /// Live document edges.
    pub edges: usize,
}

/// Work performed by one phase, including entities considered for cache removal.
/// Counts describe logical entity visits, not individual property lookups, CPU
/// time or allocator bytes. A missing/hidden edge can be visited without routing.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct DeriveMetrics {
    /// Live population at phase entry, independent of the amount of work.
    pub graph_size: GraphSize,
    /// Full or neighborhood derivation.
    pub mode: DeriveMode,
    /// Number of nodes considered in this phase.
    pub nodes: usize,
    /// Number of ports considered in this phase.
    pub ports: usize,
    /// Number of edges considered in this phase.
    pub edges: usize,
    /// Actual router invocations in this phase, including previews in their own phase.
    pub routed_edges: usize,
}

/// Observer for invalidation and derived-state rebuild activity.
///
/// Pass an implementation to [`GraphComputed::rebuild`](crate::GraphComputed::rebuild)
/// when a host wants instrumentation without baking logging or tracing into
/// this `no_std` crate. The default method bodies do nothing, so observers can
/// implement only the callbacks they need.
pub trait GraphDeriveObserver {
    /// Called once for each pending invalidation target before derive phases run.
    fn invalidated(&mut self, _cause: GraphInvalidationCause, _target: InvalidationTarget) {}

    /// Called before a derive phase begins.
    fn derive_begin(&mut self, _phase: DerivePhase) {}

    /// Called after a derive phase completes.
    fn derive_end(&mut self, _phase: DerivePhase, _metrics: DeriveMetrics) {}
}

/// No-op observer for callers that do not need instrumentation.
#[derive(Copy, Clone, Debug, Default)]
pub struct NoopGraphDeriveObserver;

impl GraphDeriveObserver for NoopGraphDeriveObserver {}
