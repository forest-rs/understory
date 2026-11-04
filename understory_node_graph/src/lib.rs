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
//! - [`GraphComputed`]: derived geometry, visibility, and edge-preview state.
//!
//! [`GraphInvalidation`] provides explicit coarse invalidation channels for the
//! derived phases. Scoped [`GraphDoc::edit`] and [`GraphProjection::edit`] calls
//! return [`GraphChanges`] receipts covering complete source revision transitions.
//! Record them in each view's invalidation state to derive only affected geometry.
//! Missing, reordered or foreign receipts fall back to a full geometry rebuild;
//! targeted hints alone cannot hide source mutations. Sources retain no journal.
//! Cached sources have instance identities: replacing or cloning a source cannot
//! silently reuse equal revision counters. Each source allocates one shared identity
//! token, using `Arc` on pointer-atomic targets and `Rc` otherwise; mutations do not
//! allocate identity tokens. Clones have independent identities and state.
//! Hosts can also provide a [`PortCompatibility`] policy to drive edge-creation
//! previews and optional connection validation without pushing a type system
//! into the crate itself. Policies receive a [`ConnectionContext`] so they can
//! inspect endpoint metadata and existing topology.
//! Observers report live [`GraphSize`] separately from phase-local entity visits
//! and actual router calls. Preview routing has its own phase; visibility reports
//! its full scan even when anchor and route derivation were targeted.
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
//!     NodeView, NoopGraphDeriveObserver, PortDirection, StraightEdgeRouter,
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
//! let mut observer = NoopGraphDeriveObserver;
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
//! This crate is `no_std` and uses `alloc`. The default `std` feature enables
//! `kurbo/std` through the viewport stack. `no_std` consumers should disable
//! default features and enable `libm`:
//!
//! ```text
//! understory_node_graph = { version = "...", default-features = false, features = ["libm"] }
//! ```

#![no_std]

extern crate alloc;

mod arena;
mod change;
mod compatibility;
mod computed;
mod edit;
mod element;
mod graph;
mod ids;
mod invalidation;
mod observe;
mod projection;
mod revision;
mod routing;
mod session;

pub use change::{ChangeSource, GraphChanges};
pub use compatibility::{
    AllowAllPortConnections, ConnectionContext, PortCompatibility, RejectDuplicateConnections,
    SingleInputConnections,
};
pub use computed::{EdgePreview, EdgePreviewTarget, GraphComputed};
pub use edit::{GraphEdit, ProjectionEdit};
pub use element::HitTarget;
pub use graph::{ConnectError, EdgeData, GraphDoc, NodeData, PortData, PortDirection};
pub use ids::{EdgeId, GraphElementId, NodeId, PortId};
pub use invalidation::{
    GRAPH, GraphInvalidation, GraphInvalidationCause, InvalidationTarget, PROJECTION, ROUTING,
    SESSION, VIEWPORT, VISIBILITY,
};
pub use observe::{
    DeriveMetrics, DeriveMode, DerivePhase, GraphDeriveObserver, GraphSize, NoopGraphDeriveObserver,
};
pub use projection::{EdgeView, GraphProjection, NodeView, PortAnchor, PortView};
pub use revision::Revision;
pub use routing::{
    EdgeRouter, OrthogonalEdgeRouter, RouteContext, RouteRequest, RoutedEdge, StraightEdgeRouter,
};
pub use session::{GraphSession, InteractionState};
