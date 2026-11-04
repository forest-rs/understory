// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Covered edits must agree with a fresh derive without routing unrelated edges.
use core::cell::Cell;
use kurbo::{Point, Rect, Size};
use understory_node_graph::*;

#[derive(Default)]
struct Router(Cell<usize>);
impl EdgeRouter for Router {
    fn route(&self, _: EdgeId, cx: &RouteContext) -> RoutedEdge {
        self.0.set(self.0.get() + 1);
        RoutedEdge::from_points(vec![cx.output_anchor, cx.input_anchor])
    }
}

struct Fixture {
    doc: GraphDoc,
    projection: GraphProjection,
    session: GraphSession,
    computed: GraphComputed,
    invalidation: GraphInvalidation,
    router: Router,
    nodes: Vec<NodeId>,
    ports: Vec<PortId>,
    edges: Vec<EdgeId>,
}
impl Fixture {
    fn new() -> Self {
        let mut doc = GraphDoc::new();
        let nodes: Vec<_> = (0..6).map(|_| doc.add_node(NodeData::default())).collect();
        let mut projection = GraphProjection::new();
        let mut ports = vec![];
        let mut edges = vec![];
        for (index, node) in nodes.iter().copied().enumerate() {
            projection.set_node_view(
                node,
                NodeView::new(Point::new(index as f64 * 100., 0.), Size::new(60., 40.), ()),
            );
            ports.push(
                doc.add_port(
                    node,
                    if index % 2 == 0 {
                        PortDirection::Output
                    } else {
                        PortDirection::Input
                    },
                    (),
                )
                .unwrap(),
            );
        }
        for pair in ports.as_chunks::<2>().0 {
            edges.push(doc.add_edge(pair[0], pair[1], ()).unwrap());
        }
        let mut fixture = Self {
            doc,
            projection,
            nodes,
            ports,
            edges,
            session: GraphSession::new(Rect::new(-100., -100., 1000., 1000.)),
            computed: GraphComputed::new(),
            invalidation: GraphInvalidation::new(),
            router: Router::default(),
        };
        fixture.rebuild();
        fixture.router.0.set(0);
        fixture
    }
    fn rebuild(&mut self) -> bool {
        self.computed.rebuild(
            &self.doc,
            &self.projection,
            &self.session,
            &mut self.invalidation,
            &self.router,
            &mut NoopGraphDeriveObserver,
        )
    }
    fn agrees_with_fresh(&self) {
        let mut fresh = GraphComputed::new();
        fresh.rebuild(
            &self.doc,
            &self.projection,
            &self.session,
            &mut GraphInvalidation::new(),
            &StraightEdgeRouter,
            &mut NoopGraphDeriveObserver,
        );
        for node in &self.nodes {
            assert_eq!(
                self.computed.node_bounds(*node),
                fresh.node_bounds(*node),
                "node {node:?} must match a full derive"
            );
        }
        for port in &self.ports {
            assert_eq!(
                self.computed.port_anchor(*port),
                fresh.port_anchor(*port),
                "port {port:?} must match a full derive"
            );
        }
        for edge in &self.edges {
            assert_eq!(
                self.computed.edge_route(*edge).map(|route| &route.points),
                fresh.edge_route(*edge).map(|route| &route.points),
                "edge {edge:?} must match a full derive"
            );
        }
        assert_eq!(
            self.computed.visible_nodes(),
            fresh.visible_nodes(),
            "node visibility must match a full derive"
        );
        assert_eq!(
            self.computed.visible_edges(),
            fresh.visible_edges(),
            "edge visibility must match a full derive"
        );
    }
}

#[test]
fn consecutive_receipts_reroute_only_neighbors_and_can_feed_multiple_views() {
    let mut f = Fixture::new();
    let mut other = f.computed.clone();
    let (_, first) = f
        .projection
        .edit(|edit| edit.translate_node(f.nodes[0], (10., 5.).into()));
    let (_, second) = f
        .projection
        .edit(|edit| edit.translate_node(f.nodes[0], (10., 5.).into()));
    f.invalidation.record(&first);
    f.invalidation.record(&second);
    f.rebuild();
    assert_eq!(f.router.0.get(), 1);
    f.agrees_with_fresh();
    let mut invalidation = GraphInvalidation::new();
    invalidation.record(&first);
    invalidation.record(&second);
    other.rebuild(
        &f.doc,
        &f.projection,
        &f.session,
        &mut invalidation,
        &f.router,
        &mut NoopGraphDeriveObserver,
    );
    assert_eq!(f.router.0.get(), 2);
    assert_eq!(
        other.node_bounds(f.nodes[0]),
        f.computed.node_bounds(f.nodes[0])
    );
    assert!(!f.rebuild());
}

#[test]
fn missing_reordered_or_foreign_receipts_cannot_hide_edits() {
    for mode in 0..4 {
        let mut f = Fixture::new();
        let (_, first) = f
            .projection
            .edit(|edit| edit.translate_node(f.nodes[0], (10., 0.).into()));
        let (_, second) = f
            .projection
            .edit(|edit| edit.translate_node(f.nodes[2], (20., 0.).into()));
        match mode {
            0 => f.invalidation.record(&second),
            1 => {
                f.invalidation.record(&second);
                f.invalidation.record(&first);
            }
            2 => {
                let mut foreign = Fixture::new();
                let (_, receipt) = foreign.projection.edit(|edit| {
                    edit.translate_node(foreign.nodes[0], (10., 0.).into());
                    edit.translate_node(foreign.nodes[2], (20., 0.).into());
                });
                f.invalidation.record(&receipt);
            }
            _ => {
                f.invalidation.record(&first);
                f.invalidation.record(&second);
                f.projection.translate_node(f.nodes[4], (30., 0.).into());
            }
        }
        f.rebuild();
        assert_eq!(f.router.0.get(), 3);
        f.agrees_with_fresh();
    }
}

#[test]
fn cascading_removal_prunes_retired_ids_without_touching_other_routes() {
    for remove_node in [false, true] {
        let mut f = Fixture::new();
        let (_, receipt) = f.doc.edit(|edit| {
            if remove_node {
                edit.remove_node(f.nodes[0]);
            } else {
                edit.remove_port(f.ports[0]);
            }
        });
        f.invalidation.record(&receipt);
        f.rebuild();
        assert_eq!(f.router.0.get(), 0);
        f.agrees_with_fresh();
        assert_eq!(f.computed.port_anchor(f.ports[0]), None);
        assert!(f.computed.edge_route(f.edges[0]).is_none());
    }
}

#[test]
fn replacing_equal_revision_sources_and_cloned_layouts_rebuilds_the_cache() {
    let mut f = Fixture::new();
    let mut other = f.projection.clone();
    f.projection.translate_node(f.nodes[0], (10., 0.).into());
    other.translate_node(f.nodes[0], (70., 0.).into());
    f.rebuild();
    f.router.0.set(0);
    assert_eq!(f.projection.revision(), other.revision());
    f.projection = other;
    assert!(f.rebuild());
    assert_eq!(f.router.0.get(), 3);
    f.agrees_with_fresh();
    f.computed.reset();
    assert!(f.computed.node_bounds(f.nodes[0]).is_none());
    assert!(f.rebuild());
    f.agrees_with_fresh();
}

#[test]
fn panicking_scoped_edits_leave_a_detectable_revision_gap() {
    let mut f = Fixture::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.projection.edit(|edit| {
            edit.update_node_view(f.nodes[0], |view| {
                view.origin.x = 123.;
                panic!("candidate interrupted");
            })
        });
    }));
    assert!(result.is_err());
    f.rebuild();
    assert_eq!(f.router.0.get(), 3);
    f.agrees_with_fresh();
}

#[test]
fn caught_panics_inside_an_edit_still_cover_the_partially_changed_entity() {
    let mut f = Fixture::new();
    let (_, changes) = f.projection.edit(|edit| {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            edit.update_node_view(f.nodes[0], |view| {
                view.origin.x = 123.;
                panic!("interrupted node update");
            });
        }));
        edit.translate_node(f.nodes[2], (12., 0.).into());
    });
    f.invalidation.record(&changes);
    f.rebuild();
    assert_eq!(f.router.0.get(), 2);
    f.agrees_with_fresh();
}

#[test]
fn port_insertion_and_edge_replacement_match_full_derivation() {
    let mut f = Fixture::new();
    let (port, changes) = f
        .doc
        .edit(|edit| edit.add_port(f.nodes[1], PortDirection::Input, ()).unwrap());
    f.ports.push(port);
    f.invalidation.record(&changes);
    f.rebuild();
    assert_eq!(f.router.0.get(), 1);
    f.agrees_with_fresh();
    f.router.0.set(0);
    let (edge, changes) = f.doc.edit(|edit| {
        edit.remove_edge(f.edges[0]);
        edit.add_edge(f.ports[0], f.ports[3], ()).unwrap()
    });
    f.edges.push(edge);
    f.invalidation.record(&changes);
    f.rebuild();
    assert_eq!(f.router.0.get(), 1);
    f.agrees_with_fresh();
}
