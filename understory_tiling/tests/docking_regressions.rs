// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! End-to-end regressions for layout and docking.

use kurbo::{Rect, Size};
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
