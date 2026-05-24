// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Derived node-graph geometry, visibility, and preview state.

use alloc::vec::Vec;
use core::cmp::Ordering;

use hashbrown::{HashMap, HashSet};
use kurbo::{Point, Rect, Vec2};

use crate::compatibility::{AllowAllPortConnections, PortCompatibility};
use crate::element::HitTarget;
use crate::graph::{GraphDoc, PortDirection};
use crate::ids::{EdgeId, NodeId, PortId};
use crate::invalidation::{GraphInvalidation, GraphInvalidationCause, InvalidationTarget};
use crate::observe::{DeriveMetrics, DerivePhase, GraphDeriveObserver};
use crate::projection::GraphProjection;
use crate::revision::Revision;
use crate::routing::{EdgeRouter, RouteContext, RoutedEdge};
use crate::session::GraphSession;

const DEFAULT_PORT_HIT_RADIUS: f64 = 8.0;
const DEFAULT_EDGE_HIT_TOLERANCE: f64 = 6.0;

/// Derived geometry, visibility, and preview state for one graph view.
///
/// `GraphComputed` is intentionally explicit. Hosts call [`GraphComputed::rebuild`]
/// after mutating the graph document, projection, session, or routing policy.
/// It reads the durable [`GraphDoc`], the current [`GraphProjection`], and the
/// active [`GraphSession`], then caches the data a renderer or interaction layer
/// usually wants immediately:
/// - node bounds + port anchors,
/// - edge routes,
/// - visible node/edge sets,
/// - edge-creation preview routes,
/// - and hit-test surfaces.
///
/// The current v0 implementation recomputes whole phases when broad source
/// revisions change. When hosts provide targeted invalidation, it can narrow
/// anchor and route recomputation to the affected neighborhood. This keeps the
/// public API calm while leaving room for finer-grained backends later.
///
/// `GraphComputed` is a cache, not an owner. If the document, projection, or
/// session changes, call `rebuild` before using query or hit-test results.
#[derive(Clone, Debug)]
pub struct GraphComputed {
    node_bounds: HashMap<NodeId, Rect>,
    port_anchors: HashMap<PortId, Point>,
    edge_routes: HashMap<EdgeId, RoutedEdge>,
    visible_nodes: Vec<NodeId>,
    visible_edges: Vec<EdgeId>,
    preview: Option<EdgePreview>,
    revision: Revision,
    initialized: bool,
    doc_revision: Revision,
    projection_revision: Revision,
    session_revision: Revision,
}

/// Derived edge-creation preview geometry.
///
/// This appears while [`InteractionState::CreateEdge`](crate::InteractionState::CreateEdge)
/// is active. A renderer can draw the route exactly like a normal edge while
/// styling the target according to whether a hovered port is compatible.
#[derive(Clone, Debug)]
pub struct EdgePreview {
    /// Source/output port that started the gesture.
    pub from: PortId,
    /// Target endpoint for the preview.
    pub target: EdgePreviewTarget,
    /// Realized route geometry for the preview edge.
    pub route: RoutedEdge,
}

/// Target endpoint for an edge-creation preview.
///
/// The preview either follows the pointer directly or snaps to a hovered input
/// port. Snapped previews carry compatibility so the UI can show allowed and
/// rejected targets without committing an edge.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum EdgePreviewTarget {
    /// Preview extends toward the current pointer in world space.
    Pointer(Point),
    /// Preview snapped to a candidate port, with compatibility status.
    Port {
        /// Candidate target port.
        port: PortId,
        /// Whether the current compatibility policy would allow connecting to this port.
        compatible: bool,
    },
}

#[derive(Clone, Debug)]
enum GeometryRebuild {
    Full,
    Targeted(TargetedGeometry),
}

#[derive(Clone, Debug, Default)]
struct TargetedGeometry {
    nodes: HashSet<NodeId>,
    edges: HashSet<EdgeId>,
}

impl TargetedGeometry {
    fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty()
    }

    fn include_node<N, P, E>(&mut self, doc: &GraphDoc<N, P, E>, node: NodeId) {
        self.nodes.insert(node);
        if let Some(ports) = doc.node_ports(node) {
            for port in ports.iter().copied() {
                self.include_port(doc, port);
            }
        }
    }

    fn include_port<N, P, E>(&mut self, doc: &GraphDoc<N, P, E>, port: PortId) {
        if let Some(port_data) = doc.port(port) {
            self.nodes.insert(port_data.owner);
        }
        if let Some(edges) = doc.port_edges(port) {
            self.edges.extend(edges.iter().copied());
        }
    }
}

impl Default for GraphComputed {
    fn default() -> Self {
        Self {
            node_bounds: HashMap::new(),
            port_anchors: HashMap::new(),
            edge_routes: HashMap::new(),
            visible_nodes: Vec::new(),
            visible_edges: Vec::new(),
            preview: None,
            revision: Revision::new(),
            initialized: false,
            doc_revision: Revision::new(),
            projection_revision: Revision::new(),
            session_revision: Revision::new(),
        }
    }
}

impl GraphComputed {
    /// Creates an empty computed-state cache.
    ///
    /// The first [`GraphComputed::rebuild`] performs a full derivation even when
    /// no explicit invalidation has been marked.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current computed-state revision.
    ///
    /// The revision changes only when a rebuild actually runs at least one
    /// derived phase.
    #[must_use]
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Returns the realized world-space bounds for `node`.
    ///
    /// Bounds exist only for live document nodes that also have a node view in
    /// the projection used for the most recent rebuild.
    #[must_use]
    pub fn node_bounds(&self, node: NodeId) -> Option<Rect> {
        self.node_bounds.get(&node).copied()
    }

    /// Returns the realized world-space anchor position for `port`.
    ///
    /// Anchors are derived from the owning node's rectangle, the port direction,
    /// sibling port order, and optional [`PortView`](crate::PortView) offset.
    #[must_use]
    pub fn port_anchor(&self, port: PortId) -> Option<Point> {
        self.port_anchors.get(&port).copied()
    }

    /// Returns the realized route geometry for `edge`.
    ///
    /// Hidden edges and edges with missing endpoint anchors do not have routes.
    #[must_use]
    pub fn edge_route(&self, edge: EdgeId) -> Option<&RoutedEdge> {
        self.edge_routes.get(&edge)
    }

    /// Returns the currently visible nodes in document order.
    ///
    /// Visibility is derived from intersection with the session viewport. It is
    /// meant as a rendering/materialization aid, not as a selection model.
    #[must_use]
    pub fn visible_nodes(&self) -> &[NodeId] {
        &self.visible_nodes
    }

    /// Returns the currently visible edges in document order.
    ///
    /// Edges are visible when their routed bounds intersect the session viewport.
    #[must_use]
    pub fn visible_edges(&self) -> &[EdgeId] {
        &self.visible_edges
    }

    /// Returns the current edge-creation preview, if any.
    ///
    /// The preview is rebuilt from session interaction and hover state. It is
    /// `None` outside an edge-creation gesture or when the source port no longer
    /// has a computed anchor.
    #[must_use]
    pub fn preview(&self) -> Option<&EdgePreview> {
        self.preview.as_ref()
    }

    /// Rebuilds derived state when source revisions or invalidation changed.
    ///
    /// This uses [`AllowAllPortConnections`] for edge-preview compatibility. It
    /// is appropriate for viewers or editors that validate connections later.
    /// Editors that need live type/socket feedback should use
    /// [`GraphComputed::rebuild_with_compatibility`].
    ///
    /// Returns `true` when any derived phase ran. When work runs, the relevant
    /// invalidation causes are cleared from `invalidation`.
    pub fn rebuild<N, P, E, NV, PV, EV, R, O>(
        &mut self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
        session: &GraphSession,
        invalidation: &mut GraphInvalidation,
        router: &R,
        observer: &mut O,
    ) -> bool
    where
        R: EdgeRouter,
        O: GraphDeriveObserver,
    {
        self.rebuild_with_compatibility(
            doc,
            projection,
            session,
            invalidation,
            router,
            &AllowAllPortConnections,
            observer,
        )
    }

    /// Rebuilds derived state using a host-defined connection compatibility policy.
    ///
    /// The compatibility policy is used only for edge-preview snapping. Semantic
    /// edge insertion remains explicit through [`GraphDoc::add_edge_with`].
    /// This split lets a host show rejected targets during a drag without
    /// mutating the document.
    ///
    /// The observer receives invalidation notifications before any derive phase
    /// runs, then phase begin/end callbacks for phases that actually rebuild.
    pub fn rebuild_with_compatibility<N, P, E, NV, PV, EV, R, C, O>(
        &mut self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
        session: &GraphSession,
        invalidation: &mut GraphInvalidation,
        router: &R,
        compatibility: &C,
        observer: &mut O,
    ) -> bool
    where
        R: EdgeRouter,
        C: PortCompatibility<N, P, E>,
        O: GraphDeriveObserver,
    {
        let metrics = DeriveMetrics {
            nodes: doc.node_count(),
            ports: doc.port_count(),
            edges: doc.edge_count(),
        };

        for cause in [
            GraphInvalidationCause::Graph,
            GraphInvalidationCause::Projection,
            GraphInvalidationCause::Session,
            GraphInvalidationCause::Viewport,
            GraphInvalidationCause::Routing,
            GraphInvalidationCause::Visibility,
        ] {
            for target in invalidation.iter(cause) {
                observer.invalidated(cause, target);
            }
        }

        let geometry_plan = self.geometry_rebuild_plan(doc, invalidation, projection);
        let geometry_dirty = geometry_plan.is_some();
        let visibility_dirty = geometry_dirty
            || !self.initialized
            || self.session_revision != session.revision()
            || invalidation.has_cause(GraphInvalidationCause::Session)
            || invalidation.has_cause(GraphInvalidationCause::Viewport)
            || invalidation.has_cause(GraphInvalidationCause::Visibility);
        let preview_dirty = geometry_dirty || visibility_dirty;

        if let Some(geometry_plan) = geometry_plan {
            observer.derive_begin(DerivePhase::NodeBounds);
            match &geometry_plan {
                GeometryRebuild::Full => self.rebuild_node_bounds(doc, projection),
                GeometryRebuild::Targeted(targets) => {
                    self.rebuild_node_bounds_targeted(doc, projection, targets);
                }
            }
            observer.derive_end(DerivePhase::NodeBounds, metrics);

            observer.derive_begin(DerivePhase::PortAnchors);
            match &geometry_plan {
                GeometryRebuild::Full => self.rebuild_port_anchors(doc, projection),
                GeometryRebuild::Targeted(targets) => {
                    self.rebuild_port_anchors_targeted(doc, projection, targets);
                }
            }
            observer.derive_end(DerivePhase::PortAnchors, metrics);

            observer.derive_begin(DerivePhase::EdgeRouting);
            match &geometry_plan {
                GeometryRebuild::Full => self.rebuild_edge_routes(doc, projection, router),
                GeometryRebuild::Targeted(targets) => {
                    self.rebuild_edge_routes_targeted(doc, projection, router, targets);
                }
            }
            observer.derive_end(DerivePhase::EdgeRouting, metrics);
        }

        if visibility_dirty {
            observer.derive_begin(DerivePhase::Visibility);
            self.rebuild_visibility(session);
            observer.derive_end(DerivePhase::Visibility, metrics);
        }

        if preview_dirty {
            self.rebuild_preview(doc, session, router, compatibility);
        }

        let rebuilt = geometry_dirty || visibility_dirty || preview_dirty;
        if rebuilt {
            self.initialized = true;
            self.doc_revision = doc.revision();
            self.projection_revision = projection.revision();
            self.session_revision = session.revision();
            self.revision.bump();
        }

        if geometry_dirty {
            invalidation.clear(GraphInvalidationCause::Graph);
            invalidation.clear(GraphInvalidationCause::Projection);
            invalidation.clear(GraphInvalidationCause::Routing);
        }
        if visibility_dirty {
            invalidation.clear(GraphInvalidationCause::Session);
            invalidation.clear(GraphInvalidationCause::Viewport);
            invalidation.clear(GraphInvalidationCause::Visibility);
        }
        rebuilt
    }

    fn geometry_rebuild_plan<N, P, E, NV, PV, EV>(
        &self,
        doc: &GraphDoc<N, P, E>,
        invalidation: &GraphInvalidation,
        projection: &GraphProjection<NV, PV, EV>,
    ) -> Option<GeometryRebuild> {
        let doc_changed = self.doc_revision != doc.revision();
        let projection_changed = self.projection_revision != projection.revision();
        let has_geometry_cause = invalidation.has_cause(GraphInvalidationCause::Graph)
            || invalidation.has_cause(GraphInvalidationCause::Projection)
            || invalidation.has_cause(GraphInvalidationCause::Routing);

        if !self.initialized {
            return Some(GeometryRebuild::Full);
        }

        if !doc_changed && !projection_changed && !has_geometry_cause {
            return None;
        }

        match self.collect_targeted_geometry(doc, invalidation) {
            Some(targets) if !targets.is_empty() => Some(GeometryRebuild::Targeted(targets)),
            _ => Some(GeometryRebuild::Full),
        }
    }

    fn collect_targeted_geometry<N, P, E>(
        &self,
        doc: &GraphDoc<N, P, E>,
        invalidation: &GraphInvalidation,
    ) -> Option<TargetedGeometry> {
        let mut targets = TargetedGeometry::default();
        for cause in [
            GraphInvalidationCause::Graph,
            GraphInvalidationCause::Projection,
            GraphInvalidationCause::Routing,
        ] {
            for target in invalidation.iter(cause) {
                match target {
                    InvalidationTarget::Graph
                    | InvalidationTarget::Projection
                    | InvalidationTarget::Session
                    | InvalidationTarget::Viewport => return None,
                    InvalidationTarget::Node(node) => {
                        targets.include_node(doc, node);
                    }
                    InvalidationTarget::Port(port) => {
                        targets.include_port(doc, port);
                    }
                    InvalidationTarget::Edge(edge) => {
                        targets.edges.insert(edge);
                    }
                }
            }
        }
        Some(targets)
    }

    /// Returns the topmost hit target at `world_point`, if any.
    ///
    /// Use this when pointer coordinates have already been converted into graph
    /// world space, or when testing generated geometry without a viewport.
    ///
    /// Hit precedence is:
    /// - ports,
    /// - nodes, ordered by highest `z_index` then highest stable id,
    /// - edges, ordered by highest `z_index`, nearest route, then highest stable id.
    #[must_use]
    pub fn hit_test_world<N, P, E, NV, PV, EV>(
        &self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
        world_point: Point,
    ) -> Option<HitTarget> {
        self.hit_test_port(doc, projection, world_point)
            .or_else(|| self.hit_test_node(projection, world_point))
            .or_else(|| self.hit_test_edge(projection, world_point))
    }

    /// Returns the topmost hit target at `view_point`, if any.
    ///
    /// This converts `view_point` through [`GraphSession::viewport`] and then
    /// delegates to [`GraphComputed::hit_test_world`].
    #[must_use]
    pub fn hit_test_view<N, P, E, NV, PV, EV>(
        &self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
        session: &GraphSession,
        view_point: Point,
    ) -> Option<HitTarget> {
        let world_point = session.viewport.view_to_world_point(view_point);
        self.hit_test_world(doc, projection, world_point)
    }

    fn rebuild_node_bounds<N, P, E, NV, PV, EV>(
        &mut self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
    ) {
        self.node_bounds.clear();
        for (node, _) in doc.iter_nodes() {
            if let Some(view) = projection.node_view(node) {
                self.node_bounds.insert(node, view.rect());
            }
        }
    }

    fn rebuild_node_bounds_targeted<N, P, E, NV, PV, EV>(
        &mut self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
        targets: &TargetedGeometry,
    ) {
        for node in targets.nodes.iter().copied() {
            if !doc.contains_node(node) {
                self.node_bounds.remove(&node);
                continue;
            }
            match projection.node_view(node) {
                Some(view) => {
                    self.node_bounds.insert(node, view.rect());
                }
                None => {
                    self.node_bounds.remove(&node);
                }
            }
        }
        self.prune_stale_node_bounds(doc, projection);
    }

    fn rebuild_port_anchors<N, P, E, NV, PV, EV>(
        &mut self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
    ) {
        self.port_anchors.clear();

        for (node, _) in doc.iter_nodes() {
            let Some(node_view) = projection.node_view(node) else {
                continue;
            };

            let ports = doc.node_ports(node).unwrap_or(&[]);
            let input_count = ports
                .iter()
                .filter(|port| matches!(doc.port(**port), Some(data) if data.direction == PortDirection::Input))
                .count();
            let output_count = ports
                .iter()
                .filter(|port| matches!(doc.port(**port), Some(data) if data.direction == PortDirection::Output))
                .count();

            let mut input_index = 0_usize;
            let mut output_index = 0_usize;

            for port in ports.iter().copied() {
                let Some(port_data) = doc.port(port) else {
                    continue;
                };
                let port_view = projection.port_view(port);
                let offset = port_view.map_or(Vec2::ZERO, |view| view.anchor_offset);

                let (count, index, x) = match port_data.direction {
                    PortDirection::Input => {
                        input_index += 1;
                        (input_count, input_index, node_view.origin.x)
                    }
                    PortDirection::Output => {
                        output_index += 1;
                        (
                            output_count,
                            output_index,
                            node_view.origin.x + node_view.size.width,
                        )
                    }
                };
                let y = distributed_edge(node_view.origin.y, node_view.size.height, index, count);
                self.port_anchors.insert(port, Point::new(x, y) + offset);
            }
        }
    }

    fn rebuild_port_anchors_targeted<N, P, E, NV, PV, EV>(
        &mut self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
        targets: &TargetedGeometry,
    ) {
        for node in targets.nodes.iter().copied() {
            self.update_port_anchors_for_node(doc, projection, node);
        }
        self.prune_stale_port_anchors(doc, projection);
    }

    fn rebuild_edge_routes<N, P, E, NV, PV, EV, R>(
        &mut self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
        router: &R,
    ) where
        R: EdgeRouter,
    {
        self.edge_routes.clear();
        for (edge, edge_data) in doc.iter_edges() {
            if matches!(projection.edge_view(edge), Some(view) if view.hidden) {
                continue;
            }
            let Some(output_anchor) = self.port_anchors.get(&edge_data.output).copied() else {
                continue;
            };
            let Some(input_anchor) = self.port_anchors.get(&edge_data.input).copied() else {
                continue;
            };
            let route = router.route(
                edge,
                &RouteContext {
                    output_anchor,
                    input_anchor,
                },
            );
            self.edge_routes.insert(edge, route);
        }
    }

    fn rebuild_edge_routes_targeted<N, P, E, NV, PV, EV, R>(
        &mut self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
        router: &R,
        targets: &TargetedGeometry,
    ) where
        R: EdgeRouter,
    {
        for edge in targets.edges.iter().copied() {
            self.update_edge_route(doc, projection, router, edge);
        }
        self.prune_stale_edge_routes(doc, projection);
    }

    fn rebuild_visibility(&mut self, session: &GraphSession) {
        let visible_world = session.visible_world_rect();

        self.visible_nodes.clear();
        self.visible_nodes.extend(
            self.node_bounds
                .iter()
                .filter_map(|(node, bounds)| intersects(*bounds, visible_world).then_some(*node)),
        );
        self.visible_nodes.sort_by_key(|node| node.index());

        self.visible_edges.clear();
        self.visible_edges.extend(
            self.edge_routes.iter().filter_map(|(edge, route)| {
                intersects(route.bounds, visible_world).then_some(*edge)
            }),
        );
        self.visible_edges.sort_by_key(|edge| edge.index());
    }

    fn update_port_anchors_for_node<N, P, E, NV, PV, EV>(
        &mut self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
        node: NodeId,
    ) {
        let Some(node_view) = projection.node_view(node) else {
            if let Some(ports) = doc.node_ports(node) {
                for port in ports {
                    self.port_anchors.remove(port);
                }
            }
            return;
        };

        let Some(ports) = doc.node_ports(node) else {
            return;
        };
        let input_count = ports
            .iter()
            .filter(|port| matches!(doc.port(**port), Some(data) if data.direction == PortDirection::Input))
            .count();
        let output_count = ports
            .iter()
            .filter(|port| matches!(doc.port(**port), Some(data) if data.direction == PortDirection::Output))
            .count();
        let mut input_index = 0_usize;
        let mut output_index = 0_usize;

        for port in ports.iter().copied() {
            let Some(port_data) = doc.port(port) else {
                self.port_anchors.remove(&port);
                continue;
            };
            let port_view = projection.port_view(port);
            let offset = port_view.map_or(Vec2::ZERO, |view| view.anchor_offset);
            let (count, index, x) = match port_data.direction {
                PortDirection::Input => {
                    input_index += 1;
                    (input_count, input_index, node_view.origin.x)
                }
                PortDirection::Output => {
                    output_index += 1;
                    (
                        output_count,
                        output_index,
                        node_view.origin.x + node_view.size.width,
                    )
                }
            };
            let y = distributed_edge(node_view.origin.y, node_view.size.height, index, count);
            self.port_anchors.insert(port, Point::new(x, y) + offset);
        }
    }

    fn update_edge_route<N, P, E, NV, PV, EV, R>(
        &mut self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
        router: &R,
        edge: EdgeId,
    ) where
        R: EdgeRouter,
    {
        if matches!(projection.edge_view(edge), Some(view) if view.hidden) {
            self.edge_routes.remove(&edge);
            return;
        }
        let Some(edge_data) = doc.edge(edge) else {
            self.edge_routes.remove(&edge);
            return;
        };
        let Some(output_anchor) = self.port_anchors.get(&edge_data.output).copied() else {
            self.edge_routes.remove(&edge);
            return;
        };
        let Some(input_anchor) = self.port_anchors.get(&edge_data.input).copied() else {
            self.edge_routes.remove(&edge);
            return;
        };
        let route = router.route(
            edge,
            &RouteContext {
                output_anchor,
                input_anchor,
            },
        );
        self.edge_routes.insert(edge, route);
    }

    fn prune_stale_node_bounds<N, P, E, NV, PV, EV>(
        &mut self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
    ) {
        self.node_bounds
            .retain(|node, _| doc.contains_node(*node) && projection.node_view(*node).is_some());
    }

    fn prune_stale_port_anchors<N, P, E, NV, PV, EV>(
        &mut self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
    ) {
        self.port_anchors.retain(|port, _| {
            let Some(port_data) = doc.port(*port) else {
                return false;
            };
            projection.node_view(port_data.owner).is_some()
        });
    }

    fn prune_stale_edge_routes<N, P, E, NV, PV, EV>(
        &mut self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
    ) {
        self.edge_routes.retain(|edge, _| {
            let Some(edge_data) = doc.edge(*edge) else {
                return false;
            };
            !matches!(projection.edge_view(*edge), Some(view) if view.hidden)
                && self.port_anchors.contains_key(&edge_data.output)
                && self.port_anchors.contains_key(&edge_data.input)
        });
    }

    fn hit_test_port<N, P, E, NV, PV, EV>(
        &self,
        doc: &GraphDoc<N, P, E>,
        projection: &GraphProjection<NV, PV, EV>,
        world_point: Point,
    ) -> Option<HitTarget> {
        let mut best: Option<(PortId, f64)> = None;

        for (port, anchor) in &self.port_anchors {
            let hit_radius = projection
                .port_view(*port)
                .map_or(DEFAULT_PORT_HIT_RADIUS, |view| view.hit_radius);
            let distance_sq = distance_sq(world_point, *anchor);
            if distance_sq > hit_radius * hit_radius {
                continue;
            }

            match best {
                Some((_, best_distance_sq)) if distance_sq >= best_distance_sq => {}
                _ => best = Some((*port, distance_sq)),
            }
        }

        best.map(|(port, _)| {
            debug_assert!(
                doc.contains_port(port),
                "computed port hit must refer to a live port"
            );
            HitTarget::Port(port)
        })
    }

    fn hit_test_node<NV, PV, EV>(
        &self,
        projection: &GraphProjection<NV, PV, EV>,
        world_point: Point,
    ) -> Option<HitTarget> {
        let mut best: Option<(NodeId, i32)> = None;

        for (node, bounds) in &self.node_bounds {
            if !bounds.contains(world_point) {
                continue;
            }
            let z_index = projection.node_view(*node).map_or(0, |view| view.z_index);
            match best {
                Some((best_node, best_z)) if (z_index, *node) <= (best_z, best_node) => {}
                _ => best = Some((*node, z_index)),
            }
        }

        best.map(|(node, _)| HitTarget::Node(node))
    }

    fn hit_test_edge<NV, PV, EV>(
        &self,
        projection: &GraphProjection<NV, PV, EV>,
        world_point: Point,
    ) -> Option<HitTarget> {
        let mut best: Option<(EdgeId, i32, f64)> = None;

        for (edge, route) in &self.edge_routes {
            let distance_sq = polyline_distance_sq(world_point, &route.points);
            if distance_sq > DEFAULT_EDGE_HIT_TOLERANCE * DEFAULT_EDGE_HIT_TOLERANCE {
                continue;
            }

            let z_index = projection.edge_view(*edge).map_or(0, |view| view.z_index);
            match best {
                Some((best_edge, best_z, best_distance_sq))
                    if !edge_hit_precedes(
                        *edge,
                        z_index,
                        distance_sq,
                        best_edge,
                        best_z,
                        best_distance_sq,
                    ) => {}
                _ => best = Some((*edge, z_index, distance_sq)),
            }
        }

        best.map(|(edge, _, _)| HitTarget::Edge(edge))
    }

    fn rebuild_preview<N, P, E, R, C>(
        &mut self,
        doc: &GraphDoc<N, P, E>,
        session: &GraphSession,
        router: &R,
        compatibility: &C,
    ) where
        R: EdgeRouter,
        C: PortCompatibility<N, P, E>,
    {
        let crate::session::InteractionState::CreateEdge { from, pointer } = session.interaction
        else {
            self.preview = None;
            return;
        };

        let Some(output_anchor) = self.port_anchors.get(&from).copied() else {
            self.preview = None;
            return;
        };

        let pointer_world = session.viewport.view_to_world_point(pointer);
        let snapped_target = match session.hover {
            Some(HitTarget::Port(port)) if port != from => {
                self.preview_target(doc, from, port, compatibility)
            }
            _ => None,
        };

        let (target, input_anchor) = match snapped_target {
            Some((port, compatible)) => {
                let anchor = self
                    .port_anchors
                    .get(&port)
                    .copied()
                    .unwrap_or(pointer_world);
                (EdgePreviewTarget::Port { port, compatible }, anchor)
            }
            None => (EdgePreviewTarget::Pointer(pointer_world), pointer_world),
        };

        let route = router.route(
            EdgeId::from_parts(u32::MAX, 0),
            &RouteContext {
                output_anchor,
                input_anchor,
            },
        );
        self.preview = Some(EdgePreview {
            from,
            target,
            route,
        });
    }

    fn preview_target<N, P, E, C>(
        &self,
        doc: &GraphDoc<N, P, E>,
        from: PortId,
        port: PortId,
        compatibility: &C,
    ) -> Option<(PortId, bool)>
    where
        C: PortCompatibility<N, P, E>,
    {
        let candidate = doc.port(port)?;
        let compatible = match candidate.direction {
            PortDirection::Input => doc.can_connect_with(from, port, compatibility).ok(),
            PortDirection::Output => None,
        }?;
        Some((port, compatible))
    }
}

fn intersects(a: Rect, b: Rect) -> bool {
    a.x0 <= b.x1 && a.x1 >= b.x0 && a.y0 <= b.y1 && a.y1 >= b.y0
}

fn distributed_edge(origin: f64, extent: f64, index: usize, count: usize) -> f64 {
    if count == 0 {
        return origin + extent * 0.5;
    }
    let step = extent / (count as f64 + 1.0);
    origin + step * index as f64
}

fn distance_sq(a: Point, b: Point) -> f64 {
    let delta = a - b;
    delta.x * delta.x + delta.y * delta.y
}

fn polyline_distance_sq(point: Point, points: &[Point]) -> f64 {
    match points {
        [] => f64::INFINITY,
        [only] => distance_sq(point, *only),
        _ => points
            .windows(2)
            .map(|segment| segment_distance_sq(point, segment[0], segment[1]))
            .fold(f64::INFINITY, f64::min),
    }
}

fn segment_distance_sq(point: Point, start: Point, end: Point) -> f64 {
    let segment = end - start;
    let to_point = point - start;
    let length_sq = segment.x * segment.x + segment.y * segment.y;
    if length_sq <= f64::EPSILON {
        return distance_sq(point, start);
    }
    let t = ((to_point.x * segment.x) + (to_point.y * segment.y)) / length_sq;
    let t = t.clamp(0.0, 1.0);
    let closest = start + segment * t;
    distance_sq(point, closest)
}

fn edge_hit_precedes(
    edge: EdgeId,
    z_index: i32,
    distance_sq: f64,
    best_edge: EdgeId,
    best_z: i32,
    best_distance_sq: f64,
) -> bool {
    if z_index != best_z {
        return z_index > best_z;
    }
    match distance_sq.total_cmp(&best_distance_sq) {
        Ordering::Less => true,
        Ordering::Equal => edge > best_edge,
        Ordering::Greater => false,
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use core::cell::Cell;

    use kurbo::{Point, Rect, Size, Vec2};

    use super::{EdgePreviewTarget, GraphComputed};
    use crate::compatibility::{ConnectionContext, PortCompatibility};
    use crate::element::HitTarget;
    use crate::graph::{GraphDoc, NodeData, PortDirection};
    use crate::invalidation::{GraphInvalidation, GraphInvalidationCause, InvalidationTarget};
    use crate::observe::NoopGraphDeriveObserver;
    use crate::projection::{EdgeView, GraphProjection, NodeView, PortView};
    use crate::routing::{EdgeRouter, RouteContext, RoutedEdge, StraightEdgeRouter};
    use crate::session::GraphSession;

    struct CountingRouter {
        calls: Cell<usize>,
    }

    impl CountingRouter {
        fn new() -> Self {
            Self {
                calls: Cell::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.get()
        }
    }

    impl EdgeRouter for CountingRouter {
        fn route(&self, edge: crate::ids::EdgeId, cx: &RouteContext) -> RoutedEdge {
            self.calls.set(self.calls.get() + 1);
            StraightEdgeRouter.route(edge, cx)
        }
    }

    #[test]
    fn rebuilds_geometry_visibility_and_hit_testing() {
        let mut doc = GraphDoc::<&'static str, &'static str, ()>::new();
        let left = doc.add_node(NodeData { meta: "left" });
        let right = doc.add_node(NodeData { meta: "right" });

        let left_out = doc
            .add_port(left, PortDirection::Output, "out")
            .expect("node exists");
        let right_in = doc
            .add_port(right, PortDirection::Input, "in")
            .expect("node exists");
        let edge = doc
            .add_edge(left_out, right_in, ())
            .expect("directions match");

        let mut projection = GraphProjection::<(), (), ()>::new();
        projection.set_node_view(
            left,
            NodeView::new(Point::new(20.0, 20.0), Size::new(100.0, 60.0), ()),
        );
        projection.set_node_view(
            right,
            NodeView::new(Point::new(220.0, 20.0), Size::new(100.0, 60.0), ()),
        );
        let mut right_in_view = PortView::new(());
        right_in_view.anchor_offset = Vec2::new(0.0, -6.0);
        projection.set_port_view(right_in, right_in_view);

        let session = GraphSession::new(Rect::new(0.0, 0.0, 180.0, 120.0));
        let mut computed = GraphComputed::new();
        let mut invalidation = GraphInvalidation::new();
        let mut observer = NoopGraphDeriveObserver;

        assert!(computed.rebuild(
            &doc,
            &projection,
            &session,
            &mut invalidation,
            &StraightEdgeRouter,
            &mut observer,
        ));
        assert_eq!(computed.visible_nodes(), &[left]);
        assert_eq!(computed.visible_edges(), &[edge]);

        let left_bounds = computed.node_bounds(left).expect("node view exists");
        assert_eq!(
            left_bounds,
            Rect::from_origin_size((20.0, 20.0), (100.0, 60.0))
        );

        let left_out_anchor = computed.port_anchor(left_out).expect("anchor exists");
        assert_eq!(left_out_anchor, Point::new(120.0, 50.0));
        let right_in_anchor = computed.port_anchor(right_in).expect("anchor exists");
        assert_eq!(right_in_anchor, Point::new(220.0, 44.0));

        let route = computed.edge_route(edge).expect("route exists");
        assert_eq!(route.points, vec![left_out_anchor, right_in_anchor]);

        assert_eq!(
            computed.hit_test_world(&doc, &projection, left_out_anchor),
            Some(HitTarget::Port(left_out))
        );
        assert_eq!(
            computed.hit_test_world(&doc, &projection, Point::new(30.0, 30.0)),
            Some(HitTarget::Node(left))
        );
        assert_eq!(
            computed.hit_test_world(&doc, &projection, Point::new(170.0, 47.0)),
            Some(HitTarget::Edge(edge))
        );
    }

    #[test]
    fn targeted_projection_invalidation_recomputes_only_affected_routes() {
        let mut doc = GraphDoc::<(), (), ()>::new();
        let a = doc.add_node(NodeData { meta: () });
        let b = doc.add_node(NodeData { meta: () });
        let c = doc.add_node(NodeData { meta: () });
        let d = doc.add_node(NodeData { meta: () });
        let e = doc.add_node(NodeData { meta: () });

        let a_out = doc.add_port(a, PortDirection::Output, ()).unwrap();
        let b_in = doc.add_port(b, PortDirection::Input, ()).unwrap();
        let b_out = doc.add_port(b, PortDirection::Output, ()).unwrap();
        let c_in = doc.add_port(c, PortDirection::Input, ()).unwrap();
        let d_out = doc.add_port(d, PortDirection::Output, ()).unwrap();
        let e_in = doc.add_port(e, PortDirection::Input, ()).unwrap();

        let ab = doc.add_edge(a_out, b_in, ()).unwrap();
        let bc = doc.add_edge(b_out, c_in, ()).unwrap();
        let de = doc.add_edge(d_out, e_in, ()).unwrap();

        let mut projection = GraphProjection::<(), (), ()>::new();
        projection.set_node_view(
            a,
            NodeView::new(Point::new(0.0, 0.0), Size::new(80.0, 60.0), ()),
        );
        projection.set_node_view(
            b,
            NodeView::new(Point::new(140.0, 0.0), Size::new(80.0, 60.0), ()),
        );
        projection.set_node_view(
            c,
            NodeView::new(Point::new(280.0, 0.0), Size::new(80.0, 60.0), ()),
        );
        projection.set_node_view(
            d,
            NodeView::new(Point::new(0.0, 120.0), Size::new(80.0, 60.0), ()),
        );
        projection.set_node_view(
            e,
            NodeView::new(Point::new(180.0, 120.0), Size::new(80.0, 60.0), ()),
        );

        let session = GraphSession::new(Rect::new(0.0, 0.0, 500.0, 240.0));
        let mut computed = GraphComputed::new();
        let mut invalidation = GraphInvalidation::new();
        let mut observer = NoopGraphDeriveObserver;
        let router = CountingRouter::new();

        assert!(computed.rebuild(
            &doc,
            &projection,
            &session,
            &mut invalidation,
            &router,
            &mut observer,
        ));
        assert_eq!(router.calls(), 3, "full rebuild should route all edges");

        let moved = {
            let mut view = projection.node_view(b).cloned().unwrap();
            view.origin = Point::new(160.0, 24.0);
            view
        };
        projection.set_node_view(b, moved);
        invalidation.mark(
            GraphInvalidationCause::Projection,
            InvalidationTarget::Node(b),
        );

        assert!(computed.rebuild(
            &doc,
            &projection,
            &session,
            &mut invalidation,
            &router,
            &mut observer,
        ));
        assert_eq!(
            router.calls(),
            5,
            "targeted node invalidation should reroute only edges touching that node"
        );

        assert!(computed.edge_route(ab).is_some());
        assert!(computed.edge_route(bc).is_some());
        assert!(computed.edge_route(de).is_some());
    }

    #[test]
    fn create_edge_preview_snaps_and_reports_compatibility() {
        struct NamesMustMatch;

        impl<N, E> PortCompatibility<N, &'static str, E> for NamesMustMatch {
            fn can_connect(&self, cx: ConnectionContext<'_, N, &'static str, E>) -> bool {
                cx.output_port().meta == cx.input_port().meta
            }
        }

        let mut doc = GraphDoc::<(), &'static str, ()>::new();
        let source = doc.add_node(NodeData { meta: () });
        let good_target = doc.add_node(NodeData { meta: () });
        let bad_target = doc.add_node(NodeData { meta: () });

        let from = doc
            .add_port(source, PortDirection::Output, "signal")
            .expect("node exists");
        let good_input = doc
            .add_port(good_target, PortDirection::Input, "signal")
            .expect("node exists");
        let bad_input = doc
            .add_port(bad_target, PortDirection::Input, "other")
            .expect("node exists");

        let mut projection = GraphProjection::<(), (), ()>::new();
        projection.set_node_view(
            source,
            NodeView::new(Point::new(0.0, 0.0), Size::new(80.0, 60.0), ()),
        );
        projection.set_node_view(
            good_target,
            NodeView::new(Point::new(180.0, 0.0), Size::new(80.0, 60.0), ()),
        );
        projection.set_node_view(
            bad_target,
            NodeView::new(Point::new(180.0, 100.0), Size::new(80.0, 60.0), ()),
        );

        let mut session = GraphSession::new(Rect::new(0.0, 0.0, 400.0, 240.0));
        session.set_hover(Some(HitTarget::Port(good_input)));
        session.set_interaction(crate::session::InteractionState::CreateEdge {
            from,
            pointer: Point::new(210.0, 28.0),
        });

        let mut computed = GraphComputed::new();
        let mut invalidation = GraphInvalidation::new();
        let mut observer = NoopGraphDeriveObserver;

        assert!(computed.rebuild_with_compatibility(
            &doc,
            &projection,
            &session,
            &mut invalidation,
            &StraightEdgeRouter,
            &NamesMustMatch,
            &mut observer,
        ));
        assert!(matches!(
            computed.preview().map(|preview| preview.target),
            Some(EdgePreviewTarget::Port {
                port,
                compatible: true
            }) if port == good_input
        ));

        session.set_hover(Some(HitTarget::Port(bad_input)));
        assert!(computed.rebuild_with_compatibility(
            &doc,
            &projection,
            &session,
            &mut invalidation,
            &StraightEdgeRouter,
            &NamesMustMatch,
            &mut observer,
        ));
        assert!(matches!(
            computed.preview().map(|preview| preview.target),
            Some(EdgePreviewTarget::Port {
                port,
                compatible: false
            }) if port == bad_input
        ));

        session.set_hover(None);
        assert!(computed.rebuild_with_compatibility(
            &doc,
            &projection,
            &session,
            &mut invalidation,
            &StraightEdgeRouter,
            &NamesMustMatch,
            &mut observer,
        ));
        assert!(matches!(
            computed.preview().map(|preview| preview.target),
            Some(EdgePreviewTarget::Pointer(_))
        ));
    }

    #[test]
    fn hit_testing_ties_are_deterministic() {
        let mut doc = GraphDoc::<(), (), ()>::new();
        let older = doc.add_node(NodeData { meta: () });
        let newer = doc.add_node(NodeData { meta: () });

        let mut projection = GraphProjection::<(), (), ()>::new();
        projection.set_node_view(
            older,
            NodeView::new(Point::new(20.0, 20.0), Size::new(80.0, 60.0), ()),
        );
        projection.set_node_view(
            newer,
            NodeView::new(Point::new(20.0, 20.0), Size::new(80.0, 60.0), ()),
        );

        let session = GraphSession::new(Rect::new(0.0, 0.0, 300.0, 160.0));
        let mut computed = GraphComputed::new();
        let mut invalidation = GraphInvalidation::new();
        let mut observer = NoopGraphDeriveObserver;
        assert!(computed.rebuild(
            &doc,
            &projection,
            &session,
            &mut invalidation,
            &StraightEdgeRouter,
            &mut observer,
        ));
        assert_eq!(
            computed.hit_test_world(&doc, &projection, Point::new(40.0, 40.0)),
            Some(HitTarget::Node(newer))
        );

        let mut older_view = projection.node_view(older).cloned().unwrap();
        older_view.z_index = 1;
        projection.set_node_view(older, older_view);
        assert!(computed.rebuild(
            &doc,
            &projection,
            &session,
            &mut invalidation,
            &StraightEdgeRouter,
            &mut observer,
        ));
        assert_eq!(
            computed.hit_test_world(&doc, &projection, Point::new(40.0, 40.0)),
            Some(HitTarget::Node(older))
        );

        let source = doc.add_node(NodeData { meta: () });
        let sink = doc.add_node(NodeData { meta: () });
        let output = doc.add_port(source, PortDirection::Output, ()).unwrap();
        let input = doc.add_port(sink, PortDirection::Input, ()).unwrap();
        let first_edge = doc.add_edge(output, input, ()).unwrap();
        let second_edge = doc.add_edge(output, input, ()).unwrap();
        projection.set_node_view(
            source,
            NodeView::new(Point::new(0.0, 100.0), Size::new(20.0, 20.0), ()),
        );
        projection.set_node_view(
            sink,
            NodeView::new(Point::new(100.0, 100.0), Size::new(20.0, 20.0), ()),
        );
        assert!(computed.rebuild(
            &doc,
            &projection,
            &session,
            &mut invalidation,
            &StraightEdgeRouter,
            &mut observer,
        ));
        assert_eq!(
            computed.hit_test_world(&doc, &projection, Point::new(60.0, 110.0)),
            Some(HitTarget::Edge(second_edge))
        );

        let mut first_edge_view = EdgeView::new(());
        first_edge_view.z_index = 1;
        projection.set_edge_view(first_edge, first_edge_view);
        assert!(computed.rebuild(
            &doc,
            &projection,
            &session,
            &mut invalidation,
            &StraightEdgeRouter,
            &mut observer,
        ));
        assert_eq!(
            computed.hit_test_world(&doc, &projection, Point::new(60.0, 110.0)),
            Some(HitTarget::Edge(first_edge))
        );
    }
}
