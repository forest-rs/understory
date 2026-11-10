// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! End-to-end regressions for layout and docking.

use kurbo::{Point, Rect, Size};
use understory_tiling::*;

fn input(width: f64) -> LayoutInput {
    LayoutInput {
        bounds: Rect::new(0.0, 0.0, width, 300.0),
        tab_bar_thickness: 24.0,
        split_handle_thickness: 0.0,
        min_pane_size: Size::ZERO,
    }
}
fn widths(tree: &TileTree, input: LayoutInput) -> Vec<f64> {
    tree.layout(input)
        .panes()
        .iter()
        .map(|p| p.rect.width())
        .collect()
}
fn drag(tree: &TileTree, input: LayoutInput, origin: Point, end: Point) -> InteractionUpdate {
    let frame = tree.layout(input);
    let options = InteractionOptions::from_layout_input(input);
    let mut state = begin_interaction(&frame, origin, &options);
    update_interaction(tree, &frame, &mut state, end, &options)
}
fn commit(tree: &mut TileTree, input: LayoutInput, update: &InteractionUpdate) {
    let frame = tree.layout(input);
    let options = InteractionOptions::from_layout_input(input);
    let proposal =
        validate_interaction_update(tree, &frame, update, &DockPolicy::default(), &options)
            .unwrap();
    commit_proposal(tree, proposal).unwrap();
}

#[test]
fn rightward_reorder_uses_gap_after_source_removal() {
    let mut tree = TileTree::from_spec(TileSpec::tabs([PaneId(1), PaneId(2), PaneId(3)])).unwrap();
    // Three 100px tabs: drop A before C's midpoint, i.e. into the B/C gap.
    let update = drag(
        &tree,
        input(300.0),
        Point::new(50.0, 10.0),
        Point::new(240.0, 10.0),
    );
    commit(&mut tree, input(300.0), &update);
    assert_eq!(
        tree.panes().collect::<Vec<_>>(),
        vec![PaneId(2), PaneId(1), PaneId(3)]
    );
}

#[test]
fn cross_group_drop_uses_hovered_insertion_gap() {
    let mut tree = TileTree::from_spec(TileSpec::equal_split(
        Axis::Horizontal,
        [
            TileSpec::tabs([PaneId(1), PaneId(2)]),
            TileSpec::tabs([PaneId(3), PaneId(4)]),
        ],
    ))
    .unwrap();
    let update = drag(
        &tree,
        input(600.0),
        Point::new(50.0, 10.0),
        Point::new(320.0, 10.0),
    );
    let target = &update.overlay.drop_targets[0];
    assert_eq!(target.preview_rect, Rect::new(300.0, 0.0, 302.0, 24.0));
    assert!(target.rect.contains(Point::new(320.0, 10.0)));
    commit(&mut tree, input(600.0), &update);
    assert_eq!(
        tree.panes().collect::<Vec<_>>(),
        vec![PaneId(2), PaneId(1), PaneId(3), PaneId(4)]
    );
}

#[test]
fn flatten_preserves_nested_relative_shares() {
    let tree = TileTree::from_spec(TileSpec::equal_split(
        Axis::Horizontal,
        [
            TileSpec::equal_split(
                Axis::Horizontal,
                [TileSpec::pane(PaneId(1)), TileSpec::pane(PaneId(2))],
            ),
            TileSpec::pane(PaneId(3)),
        ],
    ))
    .unwrap();
    assert_eq!(widths(&tree, input(600.0)), vec![150.0, 150.0, 300.0]);
}

#[test]
fn root_docking_obeys_requested_half_share() {
    let mut tree = TileTree::from_spec(TileSpec::equal_split(
        Axis::Horizontal,
        [
            TileSpec::pane(PaneId(1)),
            TileSpec::pane(PaneId(2)),
            TileSpec::pane(PaneId(3)),
        ],
    ))
    .unwrap();
    tree.apply(TileOp::MovePane {
        pane: PaneId(1),
        target: DockTarget::Split {
            tile: tree.root(),
            axis: Axis::Horizontal,
            placement: Placement::After,
            ratio: 0.5,
        },
    })
    .unwrap();
    assert_eq!(widths(&tree, input(600.0)), vec![150.0, 150.0, 300.0]);
}

#[test]
fn scale_invariant_split_insertion() {
    fn run(scale: f64) -> Vec<f64> {
        let mut tree = TileTree::from_spec(TileSpec::split(
            Axis::Horizontal,
            [TileSpec::pane(PaneId(1)), TileSpec::pane(PaneId(2))],
            [scale, scale],
        ))
        .unwrap();
        tree.apply(TileOp::SplitPane {
            pane: PaneId(1),
            axis: Axis::Horizontal,
            new_pane: PaneId(3),
            placement: Placement::After,
            share: 0.5,
        })
        .unwrap();
        widths(&tree, input(600.0))
    }
    assert_eq!(run(1.0), run(0.001));
    assert_eq!(run(1.0), run(f64::MAX));
    assert_eq!(run(1.0), run(f64::from_bits(1)));
}
