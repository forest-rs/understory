// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A host mirror must follow result reports without comparing geometry snapshots.

use std::collections::{HashMap, HashSet};

use kurbo::{Point, Rect, Size, Vec2};
use understory_node_graph::*;

#[derive(Default)]
struct Mirror {
    revision: Revision,
    nodes: HashMap<NodeId, Rect>,
    ports: HashMap<PortId, Point>,
    edges: HashMap<EdgeId, RoutedEdge>,
    visible_nodes: HashSet<NodeId>,
    visible_edges: HashSet<EdgeId>,
    preview: Option<EdgePreview>,
}

impl Mirror {
    fn apply(&mut self, changes: &ComputedChanges, computed: &GraphComputed) {
        if changes.reset {
            *self = Self::default();
        } else {
            assert_eq!(
                self.revision, changes.before,
                "result reports must be applied in order"
            );
        }
        for id in &changes.nodes.removed {
            self.nodes.remove(id);
        }
        for id in &changes.nodes.updated {
            self.nodes.insert(*id, computed.node_bounds(*id).unwrap());
        }
        for id in &changes.ports.removed {
            self.ports.remove(id);
        }
        for id in &changes.ports.updated {
            self.ports.insert(*id, computed.port_anchor(*id).unwrap());
        }
        for id in &changes.edges.removed {
            self.edges.remove(id);
        }
        for id in &changes.edges.updated {
            self.edges
                .insert(*id, computed.edge_route(*id).unwrap().clone());
        }
        for id in &changes.visible_nodes.exited {
            self.visible_nodes.remove(id);
        }
        self.visible_nodes.extend(&changes.visible_nodes.entered);
        for id in &changes.visible_edges.exited {
            self.visible_edges.remove(id);
        }
        self.visible_edges.extend(&changes.visible_edges.entered);
        if changes.preview_changed {
            self.preview = computed.preview().cloned();
        }
        self.revision = changes.after;
    }

    fn check(&self, f: &Fixture) {
        assert_eq!(
            self.nodes,
            f.doc
                .iter_nodes()
                .filter_map(|(id, _)| f.computed.node_bounds(id).map(|v| (id, v)))
                .collect(),
            "nodes mirror must match the realized result"
        );
        assert_eq!(
            self.ports,
            f.doc
                .iter_ports()
                .filter_map(|(id, _)| f.computed.port_anchor(id).map(|v| (id, v)))
                .collect(),
            "ports mirror must match the realized result"
        );
        assert_eq!(
            self.edges,
            f.doc
                .iter_edges()
                .filter_map(|(id, _)| f.computed.edge_route(id).map(|v| (id, v.clone())))
                .collect(),
            "edges mirror must match the realized result"
        );
        assert_eq!(
            self.visible_nodes,
            f.computed.visible_nodes().iter().copied().collect(),
            "visible_nodes mirror must match the realized result"
        );
        assert_eq!(
            self.visible_edges,
            f.computed.visible_edges().iter().copied().collect(),
            "visible_edges mirror must match the realized result"
        );
        assert_eq!(
            self.preview.as_ref(),
            f.computed.preview(),
            "preview mirror must match the realized result"
        );
    }
}

struct Fixture {
    doc: GraphDoc,
    projection: GraphProjection,
    session: GraphSession,
    computed: GraphComputed,
    invalidation: GraphInvalidation,
    nodes: Vec<NodeId>,
    ports: Vec<PortId>,
    edges: Vec<EdgeId>,
}

impl Fixture {
    fn new() -> Self {
        let mut doc = GraphDoc::new();
        let mut projection = GraphProjection::new();
        let nodes: Vec<_> = (0..4).map(|_| doc.add_node(NodeData::default())).collect();
        let ports: Vec<_> = nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                projection.set_node_view(
                    *node,
                    NodeView::new(Point::new(index as f64 * 100., 0.), Size::new(60., 40.), ()),
                );
                doc.add_port(
                    *node,
                    if index % 2 == 0 {
                        PortDirection::Output
                    } else {
                        PortDirection::Input
                    },
                    (),
                )
                .unwrap()
            })
            .collect();
        let edges = vec![
            doc.add_edge(ports[0], ports[1], ()).unwrap(),
            doc.add_edge(ports[2], ports[3], ()).unwrap(),
        ];
        Self {
            doc,
            projection,
            nodes,
            ports,
            edges,
            session: GraphSession::new(Rect::new(-100., -100., 1000., 1000.)),
            computed: GraphComputed::new(),
            invalidation: GraphInvalidation::new(),
        }
    }

    fn rebuild(&mut self, mirror: &mut Mirror) -> ComputedChanges {
        let changes = self.computed.rebuild_with_changes(
            &self.doc,
            &self.projection,
            &self.session,
            &mut self.invalidation,
            &StraightEdgeRouter,
            &mut NoopGraphDeriveObserver,
        );
        mirror.apply(&changes, &self.computed);
        mirror.check(self);
        changes
    }
}

#[test]
fn initial_baseline_noop_and_equal_reroutes_report_results_not_work() {
    let (mut f, mut mirror) = (Fixture::new(), Mirror::default());
    let first = f.rebuild(&mut mirror);
    assert!(first.reset && first.rebuilt());
    assert_eq!(first.nodes.updated, f.nodes);
    assert_eq!(first.ports.updated, f.ports);
    assert_eq!(first.edges.updated, f.edges);
    let noop = f.rebuild(&mut mirror);
    assert!(!noop.rebuilt() && noop.is_empty());
    f.invalidation.mark_routing_edge(f.edges[0]);
    let equal = f.rebuild(&mut mirror);
    assert!(equal.rebuilt() && equal.is_empty());
    f.invalidation.mark_graph();
    let full = f.rebuild(&mut mirror);
    assert!(full.rebuilt() && full.is_empty());
}

#[test]
fn targeted_moves_cascading_removals_and_uncovered_edits_update_the_same_mirror() {
    let (mut f, mut mirror) = (Fixture::new(), Mirror::default());
    f.rebuild(&mut mirror);
    let (_, edit) = f
        .projection
        .edit(|edit| edit.translate_node(f.nodes[0], Vec2::new(12., 7.)));
    f.invalidation.record(&edit);
    let moved = f.rebuild(&mut mirror);
    assert_eq!(moved.nodes.updated, [f.nodes[0]]);
    assert_eq!(moved.ports.updated, [f.ports[0]]);
    assert_eq!(moved.edges.updated, [f.edges[0]]);
    assert!(moved.visible_nodes.is_empty() && moved.visible_edges.is_empty());
    let (_, edit) = f.doc.edit(|edit| edit.remove_node(f.nodes[0]));
    f.invalidation.record(&edit);
    let removed = f.rebuild(&mut mirror);
    assert_eq!(removed.nodes.removed, [f.nodes[0]]);
    assert_eq!(removed.ports.removed, [f.ports[0]]);
    assert_eq!(removed.edges.removed, [f.edges[0]]);
    assert_eq!(removed.visible_nodes.exited, [f.nodes[0]]);
    assert_eq!(removed.visible_edges.exited, [f.edges[0]]);
    f.projection.translate_node(f.nodes[2], Vec2::new(20., 0.));
    let uncovered = f.rebuild(&mut mirror);
    assert!(!uncovered.reset);
    assert_eq!(uncovered.nodes.updated, [f.nodes[2]]);
    assert_eq!(uncovered.edges.updated, [f.edges[1]]);
}

#[test]
fn hidden_routes_and_missing_node_views_remove_and_restore_realized_results() {
    let (mut f, mut mirror) = (Fixture::new(), Mirror::default());
    f.rebuild(&mut mirror);
    let (_, edit) = f.projection.edit(|edit| {
        edit.set_edge_view(
            f.edges[0],
            EdgeView {
                hidden: true,
                ..EdgeView::new(())
            },
        );
        edit.remove_node_view(f.nodes[2]);
    });
    f.invalidation.record(&edit);
    let hidden = f.rebuild(&mut mirror);
    assert_eq!(hidden.nodes.removed, [f.nodes[2]]);
    assert_eq!(hidden.ports.removed, [f.ports[2]]);
    assert_eq!(hidden.edges.removed, f.edges);
    f.projection.set_edge_view(f.edges[0], EdgeView::new(()));
    f.projection.set_node_view(
        f.nodes[2],
        NodeView::new(Point::new(200., 0.), Size::new(60., 40.), ()),
    );
    let restored = f.rebuild(&mut mirror);
    assert_eq!(restored.nodes.updated, [f.nodes[2]]);
    assert_eq!(restored.edges.updated, f.edges);
}

#[test]
fn viewport_membership_changes_without_world_geometry_changes() {
    let (mut f, mut mirror) = (Fixture::new(), Mirror::default());
    let first = f.rebuild(&mut mirror);
    f.session
        .update_viewport(|view| view.pan_by_view(Vec2::new(3000., 3000.)));
    let away = f.rebuild(&mut mirror);
    assert!(away.nodes.is_empty() && away.ports.is_empty() && away.edges.is_empty());
    assert_eq!(away.visible_nodes.exited, first.visible_nodes.entered);
    assert_eq!(away.visible_edges.exited, first.visible_edges.entered);
    f.session
        .update_viewport(|view| view.pan_by_view(Vec2::new(-3000., -3000.)));
    let back = f.rebuild(&mut mirror);
    assert_eq!(back.visible_nodes.entered, away.visible_nodes.exited);
}

#[test]
fn preview_creation_motion_compatibility_and_cancellation_are_results() {
    let (mut f, mut mirror) = (Fixture::new(), Mirror::default());
    f.rebuild(&mut mirror);
    f.session.set_interaction(InteractionState::CreateEdge {
        from: f.ports[0],
        pointer: Point::new(250., 80.),
    });
    assert!(f.rebuild(&mut mirror).preview_changed);
    f.invalidation.mark_session();
    assert!(!f.rebuild(&mut mirror).preview_changed);
    f.session.set_hover(Some(HitTarget::Port(f.ports[1])));
    assert!(f.rebuild(&mut mirror).preview_changed);
    struct Reject;
    impl PortCompatibility<(), (), ()> for Reject {
        fn can_connect(&self, _: ConnectionContext<'_, (), (), ()>) -> bool {
            false
        }
    }
    f.invalidation.mark_session();
    let changes = f.computed.rebuild_with_compatibility_and_changes(
        &f.doc,
        &f.projection,
        &f.session,
        &mut f.invalidation,
        &StraightEdgeRouter,
        &Reject,
        &mut NoopGraphDeriveObserver,
    );
    assert!(changes.preview_changed);
    mirror.apply(&changes, &f.computed);
    mirror.check(&f);
    f.session.set_interaction(InteractionState::Idle);
    assert!(f.rebuild(&mut mirror).preview_changed);
    assert!(mirror.preview.is_none());
}

#[test]
fn document_replacement_and_cache_reset_establish_new_baselines() {
    let (mut f, mut mirror) = (Fixture::new(), Mirror::default());
    f.rebuild(&mut mirror);
    f.doc = f.doc.clone();
    let replaced = f.rebuild(&mut mirror);
    assert!(replaced.reset);
    assert_eq!(replaced.nodes.updated, f.nodes);
    f.computed.reset();
    assert!(f.rebuild(&mut mirror).reset);
}

#[test]
fn a_caught_router_panic_cannot_hide_partially_updated_geometry_from_the_next_report() {
    let (mut f, mut mirror) = (Fixture::new(), Mirror::default());
    f.rebuild(&mut mirror);
    let (_, edit) = f
        .projection
        .edit(|edit| edit.translate_node(f.nodes[0], Vec2::new(10., 0.)));
    f.invalidation.record(&edit);
    struct PanicRouter;
    impl EdgeRouter for PanicRouter {
        fn route(&self, _: RouteRequest, _: &RouteContext) -> RoutedEdge {
            panic!("host router failed");
        }
    }
    let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.computed.rebuild_with_changes(
            &f.doc,
            &f.projection,
            &f.session,
            &mut f.invalidation,
            &PanicRouter,
            &mut NoopGraphDeriveObserver,
        )
    }));
    assert!(interrupted.is_err());
    let recovered = f.rebuild(&mut mirror);
    assert!(recovered.reset);
    assert_eq!(recovered.nodes.updated, f.nodes);
}

#[test]
fn reused_arena_slots_leave_and_reenter_visibility_with_the_new_generation() {
    let (mut f, mut mirror) = (Fixture::new(), Mirror::default());
    f.rebuild(&mut mirror);
    let old = f.nodes[0];
    let (new, edit) = f.doc.edit(|edit| {
        edit.remove_node(old);
        edit.add_node(NodeData::default())
    });
    f.invalidation.record(&edit);
    let (_, edit) = f
        .projection
        .edit(|edit| edit.set_node_view(new, NodeView::new(Point::ZERO, Size::new(60., 40.), ())));
    f.invalidation.record(&edit);
    let changes = f.rebuild(&mut mirror);
    assert_ne!(new, old);
    assert_eq!(changes.nodes.removed, [old]);
    assert_eq!(changes.nodes.updated, [new]);
    assert_eq!(changes.visible_nodes.exited, [old]);
    assert_eq!(changes.visible_nodes.entered, [new]);
}
