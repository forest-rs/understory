// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// After you edit the crate's doc comment, run this command, then check README.md for any missing links
// cargo rdme --workspace-project=understory_node_graph --heading-base-level=0

//! Understory Node Graph: headless node-graph document and projection primitives.
//!
//! This crate provides the model behind a node graph editor/viewer without
//! collapsing everything into one monolithic editor instance.
//!
//! The current v0 shape is built around four explicit layers:
//! - [`GraphDoc`]: durable semantic graph content.
//! - [`GraphProjection`]: one 2D presentation of that graph.
//! - [`GraphSession`]: ephemeral interaction state for one projection user.
//! - [`GraphComputed`]: derived geometry, visibility, and hit-testing caches.
//!
//! [`GraphInvalidation`] provides explicit coarse invalidation channels for the
//! derived phases. The current v0 implementation still recomputes whole phases,
//! but it does so behind an explicit boundary instead of hiding the work.
//! When callers provide targeted invalidation such as
//! [`InvalidationTarget::Node`] or [`InvalidationTarget::Edge`], the computed
//! layer narrows anchor and route recomputation to the affected neighborhood.
//! Hosts can also provide a [`PortCompatibility`] policy to drive edge-creation
//! previews and optional connection validation without pushing a type system
//! into the crate itself. Policies receive a [`ConnectionContext`] so they can
//! inspect endpoint metadata and existing topology.
//!
//! The crate intentionally does **not** own rendering, widget policy, graph
//! execution, or application-specific node semantics.
//!
//! ## Architectural split
//!
//! Existing node-graph systems often tangle together:
//! - document data,
//! - positions and layout,
//! - interaction state,
//! - rendering structures,
//! - and runtime/execution concerns.
//!
//! `understory_node_graph` instead keeps those concerns separate so the same
//! graph document can support:
//! - a full editor,
//! - a read-only viewer,
//! - a minimap,
//! - tests and headless tools,
//! - and alternate projections over the same graph.
//!
//! ## Minimal example
//!
//! ```rust
//! use kurbo::{Point, Rect, Size};
//! use understory_node_graph::{
//!     GraphComputed, GraphDoc, GraphInvalidation, GraphProjection, GraphSession, NodeData,
//!     NodeView, NoopGraphViewObserver, PortDirection, StraightEdgeRouter,
//! };
//!
//! let mut doc = GraphDoc::<&'static str, &'static str, ()>::new();
//! let node = doc.add_node(NodeData { meta: "Add" });
//! let input = doc.add_port(node, PortDirection::Input, "lhs").unwrap();
//! let output = doc.add_port(node, PortDirection::Output, "sum").unwrap();
//! assert_ne!(input, output);
//!
//! let mut projection = GraphProjection::<(), (), ()>::new();
//! projection.set_node_view(
//!     node,
//!     NodeView::new(Point::new(100.0, 80.0), Size::new(180.0, 96.0), ()),
//! );
//!
//! let session = GraphSession::new(Rect::new(0.0, 0.0, 800.0, 600.0));
//!
//! let mut computed = GraphComputed::new();
//! let mut invalidation = GraphInvalidation::new();
//! let mut observer = NoopGraphViewObserver;
//!
//! assert!(computed.rebuild(
//!     &doc,
//!     &projection,
//!     &session,
//!     &mut invalidation,
//!     &StraightEdgeRouter,
//!     &mut observer,
//! ));
//! assert_eq!(computed.visible_nodes(), &[node]);
//! assert!(computed.hit_test_world(&doc, &projection, Point::new(110.0, 90.0)).is_some());
//! ```
//!
//! ## Intentional v0 limits
//!
//! This crate starts with nodes, ports, and edges only. It deliberately defers:
//! - groups and comments,
//! - spatial acceleration backends,
//! - text/label modeling,
//! - and execution/runtime semantics.
//!
//! It also keeps the top-level pieces separate instead of forcing everything
//! into one giant editor object. A host can reuse one [`GraphDoc`] with
//! multiple projections, sessions, and computed caches.
//!
//! Those can be added later as clear layers above or beside this one.
//!
//! This crate is `no_std` and uses `alloc`.

#![no_std]

extern crate alloc;

mod arena;
mod compatibility;
mod computed;
mod element;
mod graph;
mod ids;
mod invalidation;
mod observe;
mod projection;
mod revision;
mod routing;
mod session;

pub use compatibility::{AllowAllPortConnections, ConnectionContext, PortCompatibility};
pub use computed::{EdgePreview, EdgePreviewTarget, GraphComputed};
pub use element::HitTarget;
pub use graph::{ConnectError, EdgeData, GraphDoc, NodeData, PortData, PortDirection};
pub use ids::{EdgeId, GraphElementId, NodeId, PortId};
pub use invalidation::{
    GRAPH, GraphInvalidation, GraphInvalidationCause, HIT_TEST, InvalidationTarget, PROJECTION,
    ROUTING, SESSION, VIEWPORT, VISIBILITY,
};
pub use observe::{DeriveMetrics, DerivePhase, GraphViewObserver, NoopGraphViewObserver};
pub use projection::{EdgeView, GraphProjection, NodeView, PortView};
pub use revision::Revision;
pub use routing::{EdgeRouter, OrthogonalEdgeRouter, RouteContext, RoutedEdge, StraightEdgeRouter};
pub use session::{GraphSession, InteractionState};
