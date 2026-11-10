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
fn ghost_matches_solved_result_after_source_disappears() {
    let tree = TileTree::from_spec(TileSpec::equal_split(
        Axis::Horizontal,
        [TileSpec::pane(PaneId(1)), TileSpec::pane(PaneId(2))],
    ))
    .unwrap();
    let update = drag(
        &tree,
        input(600.0),
        Point::new(100.0, 150.0),
        Point::new(590.0, 150.0),
    );
    let ghost = update.overlay.ghost_rects[0].rect;
    let preview = update.preview.unwrap();
    let actual = preview
        .panes()
        .iter()
        .find(|p| p.pane == PaneId(1))
        .unwrap()
        .rect;
    assert_eq!(ghost, actual);
}

#[test]
fn edge_preview_includes_target_group_chrome() {
    let tree = TileTree::from_spec(TileSpec::equal_split(
        Axis::Horizontal,
        [
            TileSpec::tabs([PaneId(1), PaneId(2)]),
            TileSpec::tabs([PaneId(3), PaneId(4)]),
        ],
    ))
    .unwrap();
    // Inspect whether the candidate generator offers a complete-group edge geometry.
    let frame = tree.layout(input(600.0));
    let source = DragSession {
        subject: DragSubject::Tab {
            group: frame.tabs()[0].group,
            pane: PaneId(1),
        },
        source: DragSource::Tab {
            group: frame.tabs()[0].group,
            pane: PaneId(1),
        },
        origin: Point::new(50.0, 10.0),
        current: Point::new(590.0, 150.0),
        base_revision: tree.revision(),
        proposal: None,
    };
    let targets = drop_targets_for_drag(&tree, &frame, &source, &DragOptions::default());
    let id = pick_drop_target(&targets, source.current).unwrap();
    let target = targets.iter().find(|t| t.id == id).unwrap();
    // Splitting beside a tab group reserves the full group's height, including its bar.
    assert_eq!(target.preview_rect.y0, 0.0);
}

#[test]
fn small_bounds_do_not_emit_split_handles_outside_root() {
    let tree = TileTree::from_spec(TileSpec::equal_split(
        Axis::Horizontal,
        [
            TileSpec::pane(PaneId(1)),
            TileSpec::pane(PaneId(2)),
            TileSpec::pane(PaneId(3)),
        ],
    ))
    .unwrap();
    let mut layout = input(2.0);
    layout.split_handle_thickness = 6.0;
    let frame = tree.layout(layout);
    for handle in frame.split_handles() {
        assert!(
            handle.rect.x1 <= layout.bounds.x1,
            "{:?} exceeds {:?}",
            handle.rect,
            layout.bounds
        );
    }
}

#[test]
fn nested_minima_are_preserved_when_root_has_enough_room() {
    let tree = TileTree::from_spec(TileSpec::equal_split(
        Axis::Horizontal,
        [
            TileSpec::equal_split(
                Axis::Vertical,
                [
                    TileSpec::equal_split(
                        Axis::Horizontal,
                        [TileSpec::pane(PaneId(1)), TileSpec::pane(PaneId(2))],
                    ),
                    TileSpec::pane(PaneId(3)),
                ],
            ),
            TileSpec::pane(PaneId(4)),
        ],
    ))
    .unwrap();
    let mut layout = input(300.0);
    layout.split_handle_thickness = 6.0;
    layout.min_pane_size = Size::new(80.0, 40.0);
    let frame = tree.layout(layout);
    let pane = frame.panes().iter().find(|p| p.pane == PaneId(1)).unwrap();
    assert!(
        pane.rect.width() >= 80.0,
        "pane width {}",
        pane.rect.width()
    );
}

#[test]
fn root_docking_respects_split_permissions() {
    let tree = TileTree::from_spec(TileSpec::tabs([PaneId(1), PaneId(2)])).unwrap();
    let mut policy = DockPolicy::default();
    policy.permissions.allow_split = false;
    let proposal = Proposal::Dock(DockProposal::MovePane {
        pane: PaneId(1),
        target: DockTarget::Root,
    });
    let validated = validate_proposal(ProposalValidationInput::new(
        &tree,
        proposal.clone(),
        &policy,
    ));
    assert!(
        matches!(validated, Err(TileError::PolicyRejected)),
        "{validated:?}"
    );
    policy.permissions.allow_split = true;
    policy.permissions.allowed_edges = EdgeSet::LEFT;
    assert!(matches!(
        validate_proposal(ProposalValidationInput::new(&tree, proposal, &policy)),
        Err(TileError::PolicyRejected)
    ));
}

#[test]
fn collapse_preserves_the_split_slot_and_active_tab_through_restore() {
    let mut tree = TileTree::from_spec(TileSpec::split(
        Axis::Horizontal,
        [
            TileSpec::tabs([PaneId(1), PaneId(2)]),
            TileSpec::pane(PaneId(3)),
        ],
        [1.0, 3.0],
    ))
    .unwrap();
    tree.apply(TileOp::ActivatePane { pane: PaneId(2) })
        .unwrap();
    let layout = input(600.0);
    let expanded = tree.layout(layout);
    let tile = expanded.panes()[0].tile;

    tree.apply(TileOp::SetCollapsed {
        tile,
        extent: Some(24.0),
    })
    .unwrap();
    let collapsed = tree.layout(layout);
    assert_eq!(
        tree.panes().collect::<Vec<_>>(),
        [PaneId(1), PaneId(2), PaneId(3)]
    );
    assert_eq!(collapsed.panes()[0].rect.x0, 24.0);
    assert_eq!(collapsed.collapsed_tiles()[0].active_pane, PaneId(2));
    assert_eq!(
        hit_test(&collapsed, Point::new(10.0, 50.0)),
        Some(HitKind::CollapsedTile { tile })
    );
    assert!(
        diff_frames(&expanded, &collapsed)
            .items
            .iter()
            .any(|item| item.item == FrameItemId::CollapsedTile(tile))
    );

    let snapshot = LayoutSnapshot::new(tree);
    assert_eq!(snapshot.schema_version(), Some(2));
    let restored = restore_snapshot(snapshot).unwrap();
    assert!(restored.repairs.is_empty());
    let mut tree = restored.tree;
    assert_eq!(tree.collapsed_extent(tile), Some(24.0));
    tree.apply(TileOp::SetCollapsed { tile, extent: None })
        .unwrap();
    assert_eq!(tree.layout(layout).panes(), expanded.panes());
}

#[test]
fn collapsed_siblings_fit_tiny_bounds_and_the_last_tile_expands() {
    let mut tree = TileTree::from_spec(TileSpec::equal_split(
        Axis::Horizontal,
        [TileSpec::pane(PaneId(1)), TileSpec::pane(PaneId(2))],
    ))
    .unwrap();
    let frame = tree.layout(input(30.0));
    let first = frame.panes()[0].tile;
    let second = frame.panes()[1].tile;
    tree.apply(TileOp::SetCollapsed {
        tile: first,
        extent: Some(20.0),
    })
    .unwrap();
    tree.apply(TileOp::SetCollapsed {
        tile: second,
        extent: Some(40.0),
    })
    .unwrap();
    let frame = tree.layout(input(30.0));
    assert!(frame.panes().is_empty());
    assert_eq!(frame.collapsed_tiles()[0].rect.width(), 10.0);
    assert_eq!(frame.collapsed_tiles()[1].rect.width(), 20.0);
    tree.apply(TileOp::ClosePane { pane: PaneId(2) }).unwrap();
    assert_eq!(tree.root(), first);
    assert_eq!(tree.collapsed_extent(first), None);
    assert_eq!(tree.layout(input(30.0)).panes()[0].pane, PaneId(1));
}

#[test]
fn reopen_returns_to_the_saved_tab_gap_after_unrelated_edits() {
    let mut tree = TileTree::from_spec(TileSpec::equal_split(
        Axis::Horizontal,
        [
            TileSpec::tabs([PaneId(1), PaneId(2), PaneId(3)]),
            TileSpec::pane(PaneId(4)),
        ],
    ))
    .unwrap();
    let bookmark = tree.close_pane_with_bookmark(PaneId(2)).unwrap();
    tree.apply(TileOp::SplitPane {
        pane: PaneId(4),
        axis: Axis::Vertical,
        new_pane: PaneId(5),
        placement: Placement::After,
        share: 0.25,
    })
    .unwrap();
    let before = tree.layout(input(600.0));
    assert_eq!(tree.reopen_pane(&bookmark), Ok(ReopenPlacement::Exact));
    assert_eq!(
        tree.panes().collect::<Vec<_>>(),
        [PaneId(1), PaneId(2), PaneId(3), PaneId(4), PaneId(5)]
    );
    let after = tree.layout(input(600.0));
    assert_eq!(after.panes()[0].pane, PaneId(2));
    assert_eq!(after.panes()[1..], before.panes()[1..]);
    let revision = tree.revision();
    assert_eq!(
        tree.reopen_pane(&bookmark),
        Err(TileError::DuplicatePane(PaneId(2)))
    );
    assert_eq!(tree.revision(), revision);
}

#[test]
fn reopen_preserves_the_current_relative_shares_of_other_children() {
    let mut tree = TileTree::from_spec(TileSpec::split(
        Axis::Horizontal,
        [
            TileSpec::pane(PaneId(1)),
            TileSpec::pane(PaneId(2)),
            TileSpec::pane(PaneId(3)),
        ],
        [1.0, 2.0, 1.0],
    ))
    .unwrap();
    let bookmark = tree.close_pane_with_bookmark(PaneId(2)).unwrap();
    tree.apply(TileOp::SetSplitShares {
        split: tree.root(),
        shares: vec![3.0, 1.0],
    })
    .unwrap();
    assert_eq!(tree.reopen_pane(&bookmark), Ok(ReopenPlacement::Exact));
    assert_eq!(widths(&tree, input(800.0)), [300.0, 400.0, 100.0]);
}

#[test]
fn reopen_recreates_a_binary_slot_or_reports_the_root_fallback() {
    let mut tree = TileTree::from_spec(TileSpec::split(
        Axis::Vertical,
        [TileSpec::pane(PaneId(1)), TileSpec::tabs([PaneId(2)])],
        [1.0, 3.0],
    ))
    .unwrap();
    let bookmark = tree.close_pane_with_bookmark(PaneId(2)).unwrap();
    assert_eq!(tree.reopen_pane(&bookmark), Ok(ReopenPlacement::Exact));
    let frame = tree.layout(input(600.0));
    assert_eq!(frame.panes()[0].rect.height(), 75.0);
    assert_eq!(frame.tab_bars()[0].rect.y0, 75.0);

    let mut tree = TileTree::single_pane(PaneId(3));
    assert_eq!(
        tree.reopen_pane(&bookmark),
        Ok(ReopenPlacement::Approximate)
    );
    assert_eq!(tree.panes().collect::<Vec<_>>(), [PaneId(3), PaneId(2)]);
}

#[test]
fn narrow_container_edge_zone_docks_beside_the_whole_split() {
    let tree = TileTree::from_spec(TileSpec::equal_split(
        Axis::Horizontal,
        [
            TileSpec::equal_split(
                Axis::Vertical,
                [TileSpec::pane(PaneId(1)), TileSpec::pane(PaneId(2))],
            ),
            TileSpec::pane(PaneId(3)),
        ],
    ))
    .unwrap();
    let frame = tree.layout(input(600.0));
    let container = frame
        .split_children()
        .iter()
        .find(|child| child.split == tree.root())
        .unwrap()
        .child;
    let update = drag(
        &tree,
        input(600.0),
        Point::new(500.0, 150.0),
        Point::new(5.0, 150.0),
    );
    assert_eq!(
        update.overlay.active_target,
        Some(DropTargetId::Split {
            tile: container,
            axis: Axis::Horizontal,
            placement: Placement::Before,
        })
    );
    let update = drag(
        &tree,
        input(600.0),
        Point::new(500.0, 150.0),
        Point::new(40.0, 100.0),
    );
    assert_eq!(
        update.overlay.active_target,
        Some(DropTargetId::Split {
            tile: frame.panes()[0].tile,
            axis: Axis::Horizontal,
            placement: Placement::Before,
        })
    );
}

#[test]
fn tab_bar_target_identity_survives_gap_changes() {
    let tree = TileTree::from_spec(TileSpec::tabs([PaneId(1), PaneId(2), PaneId(3)])).unwrap();
    let frame = tree.layout(input(300.0));
    let options = InteractionOptions::from_layout_input(input(300.0));
    let mut state = begin_interaction(&frame, Point::new(50.0, 10.0), &options);
    let first = update_interaction(&tree, &frame, &mut state, Point::new(140.0, 10.0), &options);
    let second = update_interaction(&tree, &frame, &mut state, Point::new(240.0, 10.0), &options);
    assert_eq!(
        first.overlay.active_target,
        Some(DropTargetId::TabBar(tree.root()))
    );
    assert_eq!(first.overlay.active_target, second.overlay.active_target);
    assert_ne!(first.overlay.ghost_rects, second.overlay.ghost_rects);
}

#[test]
fn overlapping_edges_hold_selection_until_the_new_edge_is_clearly_closer() {
    let tree = TileTree::from_spec(TileSpec::equal_split(
        Axis::Horizontal,
        [TileSpec::pane(PaneId(1)), TileSpec::pane(PaneId(2))],
    ))
    .unwrap();
    let frame = tree.layout(input(600.0));
    let options = InteractionOptions::from_layout_input(input(600.0));
    let mut state = begin_interaction(&frame, Point::new(150.0, 150.0), &options);
    let first = update_interaction(&tree, &frame, &mut state, Point::new(310.0, 11.0), &options);
    let second = update_interaction(&tree, &frame, &mut state, Point::new(311.0, 10.0), &options);
    let third = update_interaction(&tree, &frame, &mut state, Point::new(330.0, 10.0), &options);
    assert!(matches!(
        first.overlay.active_target,
        Some(DropTargetId::Split {
            axis: Axis::Horizontal,
            ..
        })
    ));
    assert_eq!(second.overlay.active_target, first.overlay.active_target);
    assert!(matches!(
        third.overlay.active_target,
        Some(DropTargetId::Split {
            axis: Axis::Vertical,
            ..
        })
    ));
}

#[test]
fn drag_uses_the_same_edge_permissions_as_commit_validation() {
    let tree = TileTree::from_spec(TileSpec::equal_split(
        Axis::Horizontal,
        [TileSpec::pane(PaneId(1)), TileSpec::pane(PaneId(2))],
    ))
    .unwrap();
    let frame = tree.layout(input(600.0));
    let options = InteractionOptions::from_layout_input(input(600.0));
    let start = Point::new(150.0, 150.0);
    let end = Point::new(595.0, 150.0);
    let mut state = begin_interaction(&frame, start, &options);
    let allowed = update_interaction(&tree, &frame, &mut state, end, &options);
    assert!(allowed.preview.is_some());

    let mut policy = DockPolicy::default();
    policy.permissions.allowed_edges = EdgeSet::LEFT;
    let rejected =
        update_interaction_with_policy(&tree, &frame, &mut state, end, &options, &policy);
    assert!(rejected.proposal.is_none());
    assert!(rejected.preview.is_none());
    assert_eq!(
        rejected.overlay.drop_targets[0].rejection,
        Some(DropRejection::EdgeDisabled)
    );
    assert_eq!(rejected.overlay.ghost_rects[0].kind, GhostKind::Invalid);
    assert!(matches!(
        validate_interaction_update(&tree, &frame, &allowed, &policy, &options),
        Err(TileError::PolicyRejected)
    ));
}

#[test]
fn moving_a_leaf_to_a_split_keeps_its_tile_identity() {
    let mut tree = TileTree::from_spec(TileSpec::equal_split(
        Axis::Horizontal,
        [
            TileSpec::pane(PaneId(1)),
            TileSpec::pane(PaneId(2)),
            TileSpec::pane(PaneId(3)),
        ],
    ))
    .unwrap();
    let layout = input(600.0);
    let frame = tree.layout(layout);
    let moved = frame.panes()[0].tile;
    tree.apply(TileOp::MovePane {
        pane: PaneId(1),
        target: DockTarget::Split {
            tile: frame.panes()[2].tile,
            axis: Axis::Horizontal,
            placement: Placement::After,
            ratio: 0.5,
        },
    })
    .unwrap();
    let frame = tree.layout(layout);
    assert_eq!(frame.panes()[2].tile, moved);
    assert_eq!(frame.panes()[2].pane, PaneId(1));
}

#[test]
fn tab_insertion_marker_fits_a_fractional_viewport_crossing_zero() {
    let tree = TileTree::from_spec(TileSpec::tabs([PaneId(1), PaneId(2)])).unwrap();
    let layout = LayoutInput {
        bounds: Rect::new(-0.1, 0.0, 0.2, 300.0),
        ..input(1.0)
    };
    let frame = tree.layout(layout);
    let options = InteractionOptions {
        drag_threshold: 0.0,
        ..InteractionOptions::from_layout_input(layout)
    };
    let mut state = begin_interaction(&frame, Point::new(0.0, 10.0), &options);
    let update = update_interaction(&tree, &frame, &mut state, Point::new(0.1, 10.0), &options);
    assert_eq!(
        update.overlay.ghost_rects[0].rect,
        frame.tab_bars()[0].viewport
    );
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
