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
}

/// Lightweight derive metrics for one rebuild phase.
///
/// Metrics describe the current graph size, not necessarily the exact number of
/// entries touched by a targeted rebuild. They are meant for logging, debug UI,
/// and broad performance tracking.
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
