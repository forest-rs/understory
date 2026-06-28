// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;

use crate::*;

fn input() -> LayoutInput {
    LayoutInput {
        bounds: Rect::new(0.0, 0.0, 300.0, 200.0),
        tab_bar_thickness: 20.0,
        split_handle_thickness: 10.0,
        min_pane_size: Size::new(20.0, 20.0),
        generate_drop_targets: false,
    }
}

#[test]
fn single_pane_fills_bounds() {
    let tree = TileTree::single_pane(PaneId(1));
    let frame = tree.layout(input());
    assert_eq!(frame.panes.len(), 1);
    assert_eq!(frame.panes[0].rect, input().bounds);
}

#[test]
fn split_pane_creates_two_panes_and_handle() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();

    let frame = tree.layout(input());
    assert_eq!(frame.panes.len(), 2);
    assert_eq!(frame.split_children.len(), 2);
    assert_eq!(frame.split_handles.len(), 1);
    assert_eq!(frame.panes[0].rect.width(), 145.0);
    assert_eq!(frame.split_children[0].rect, frame.panes[0].rect);
    assert_eq!(frame.split_children[1].rect, frame.panes[1].rect);
    assert_eq!(frame.split_handles[0].rect.width(), 10.0);
    assert_eq!(frame.panes[1].rect.width(), 145.0);
}

#[test]
fn frame_diff_reports_added_and_resized_items() {
    let mut tree = TileTree::single_pane(PaneId(1));
    let before = tree.layout(input());
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let after = tree.layout(input());

    let diff = diff_frames(&before, &after);

    assert!(diff.items.iter().any(|item| {
        item.item == FrameItemId::Pane(PaneId(1)) && item.change == FrameChange::Resized
    }));
    assert!(diff.items.iter().any(|item| {
        item.item == FrameItemId::Pane(PaneId(2)) && item.change == FrameChange::Added
    }));
    assert!(diff.items.iter().any(|item| matches!(
        item,
        FrameItemDiff {
            item: FrameItemId::Pane(PaneId(2)),
            change: FrameChange::Added,
            transition: Some(FrameTransitionHint::EnteredFrom {
                item: Some(FrameItemId::Pane(PaneId(1))),
                rect,
            }),
            ..
        } if *rect == input().bounds
    )));
    assert!(diff.items.iter().any(|item| matches!(
        item,
        FrameItemDiff {
            item: FrameItemId::SplitChild { .. },
            change: FrameChange::Added,
            ..
        }
    )));
    assert!(diff.items.iter().any(|item| matches!(
        item,
        FrameItemDiff {
            item: FrameItemId::SplitHandle { handle: 0, .. },
            change: FrameChange::Added,
            ..
        }
    )));
}

#[test]
fn frame_diff_reports_moved_and_resized_items() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let before = tree.layout(input());

    tree.apply(TileOp::SetSplitShares {
        split: tree.root(),
        shares: vec![2.0, 1.0],
    })
    .unwrap();
    let after = tree.layout(input());

    let diff = diff_frames(&before, &after);

    assert!(diff.items.iter().any(|item| {
        item.item == FrameItemId::Pane(PaneId(2)) && item.change == FrameChange::MovedAndResized
    }));
}

#[test]
fn tabs_emit_bar_tabs_and_active_pane() {
    let tree = TileTree::new(TileNode::tabs(vec![PaneId(1), PaneId(2)]));
    let frame = tree.layout(input());
    assert_eq!(frame.tab_bars.len(), 1);
    assert_eq!(frame.tabs.len(), 2);
    assert_eq!(frame.panes.len(), 1);
    assert_eq!(frame.panes[0].pane, PaneId(1));
    assert_eq!(frame.panes[0].rect.y0, 20.0);
}

#[test]
fn activate_pane_changes_active_tab() {
    let mut tree = TileTree::new(TileNode::tabs(vec![PaneId(1), PaneId(2)]));
    tree.apply(TileOp::ActivatePane { pane: PaneId(2) })
        .unwrap();
    let frame = tree.layout(input());
    assert_eq!(frame.panes[0].pane, PaneId(2));
    assert_eq!(tree.revision(), Revision(1));
}

#[test]
fn hit_test_prefers_split_handle() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let frame = tree.layout(input());
    assert!(matches!(
        hit_test(&frame, Point::new(150.0, 50.0)),
        Some(HitKind::SplitHandle { .. })
    ));
}

#[test]
fn reorder_tab_moves_pane() {
    let mut tree = TileTree::new(TileNode::tabs(vec![PaneId(1), PaneId(2), PaneId(3)]));
    let group = tree.root();
    tree.apply(TileOp::ReorderTab {
        group,
        pane: PaneId(3),
        index: 0,
    })
    .unwrap();
    let Some(TileNode::Tabs(tabs)) = tree.node(group) else {
        panic!("root should be tabs");
    };
    assert_eq!(tabs.panes, vec![PaneId(3), PaneId(1), PaneId(2)]);
}

#[test]
fn move_pane_into_tab_group() {
    let mut tree = TileTree::new(TileNode::tabs(vec![PaneId(1)]));
    let group = tree.root();
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    tree.apply(TileOp::MovePane {
        pane: PaneId(2),
        target: DockTarget::TabInto {
            group,
            index: Some(1),
        },
    })
    .unwrap();
    let Some(TileNode::Tabs(tabs)) = tree.node(group) else {
        panic!("group should still be tabs");
    };
    assert_eq!(tabs.panes, vec![PaneId(1), PaneId(2)]);
}

#[test]
fn set_split_shares_repairs_bad_input() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let split = tree.root();
    tree.apply(TileOp::SetSplitShares {
        split,
        shares: vec![f64::NAN],
    })
    .unwrap();
    let Some(TileNode::Split(node)) = tree.node(split) else {
        panic!("root should be split");
    };
    assert_eq!(node.shares, vec![1.0, 1.0]);
}

#[test]
fn split_pane_rejects_invalid_share() {
    let mut tree = TileTree::single_pane(PaneId(1));

    assert!(matches!(
        tree.apply(TileOp::SplitPane {
            pane: PaneId(1),
            axis: Axis::Horizontal,
            new_pane: PaneId(2),
            placement: Placement::After,
            share: f64::NAN,
        }),
        Err(TileError::InvalidOperation)
    ));
    assert!(matches!(
        tree.apply(TileOp::SplitPane {
            pane: PaneId(1),
            axis: Axis::Horizontal,
            new_pane: PaneId(2),
            placement: Placement::After,
            share: 1.0,
        }),
        Err(TileError::InvalidOperation)
    ));
}

#[test]
fn move_pane_rejects_invalid_split_ratio() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let target = tree.root();

    assert!(matches!(
        tree.apply(TileOp::MovePane {
            pane: PaneId(2),
            target: DockTarget::Split {
                tile: target,
                axis: Axis::Horizontal,
                placement: Placement::After,
                ratio: f64::INFINITY,
            },
        }),
        Err(TileError::InvalidTarget)
    ));
}

#[test]
fn drag_update_proposes_move() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let frame = tree.layout(input());
    let mut drag = begin_drag(&frame, Point::new(20.0, 20.0), DragIntent::Move).unwrap();
    let update = update_drag(
        &tree,
        &frame,
        &mut drag,
        Point::new(290.0, 50.0),
        &DragOptions::default(),
    );
    assert!(matches!(
        update.proposal,
        Some(DockProposal::MovePane { .. })
    ));
}

#[test]
fn drag_update_returns_preview_layout_when_layout_input_is_provided() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let frame = tree.layout(input());
    let mut drag = begin_drag(&frame, Point::new(20.0, 20.0), DragIntent::Move).unwrap();
    let options = DragOptions {
        preview_layout: Some(input()),
        ..DragOptions::default()
    };

    let update = update_drag(&tree, &frame, &mut drag, Point::new(290.0, 50.0), &options);

    let preview = update.preview.unwrap();
    assert_eq!(preview.panes.len(), 2);
    assert_eq!(preview.split_children.len(), 2);
    assert_eq!(preview.split_handles.len(), 1);
    assert!(!preview.hit_regions.is_empty());
    assert!(!preview.focus_order.is_empty());
    assert!(!preview.paint_order.is_empty());
}

#[test]
fn drag_preview_can_diff_to_committed_frame() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let frame = tree.layout(input());
    let mut drag = begin_drag(&frame, Point::new(20.0, 20.0), DragIntent::Move).unwrap();
    let options = DragOptions {
        preview_layout: Some(input()),
        ..DragOptions::default()
    };
    let update = update_drag(&tree, &frame, &mut drag, Point::new(290.0, 50.0), &options);
    let preview = update.preview.unwrap();

    commit_drag(&mut tree, drag).unwrap();
    let committed = tree.layout(input());

    assert!(diff_frames(&preview, &committed).items.is_empty());
    assert!(
        diff_frames(&frame, &preview)
            .items
            .iter()
            .any(|item| matches!(
                item.transition,
                Some(FrameTransitionHint::EnteredFrom { .. })
                    | Some(FrameTransitionHint::SharedOrigin(_))
            ))
    );
}

#[test]
fn pending_drag_waits_until_threshold_is_crossed() {
    let tree = TileTree::single_pane(PaneId(1));
    let frame = tree.layout(input());
    let pending =
        begin_pending_drag(&frame, Point::new(20.0, 20.0), DragIntent::Move, 5.0).unwrap();

    assert!(pending.update(Point::new(23.0, 23.0)).is_none());

    let drag = pending.update(Point::new(26.0, 20.0)).unwrap();
    assert_eq!(drag.origin, Point::new(20.0, 20.0));
    assert_eq!(drag.current, Point::new(26.0, 20.0));
    assert_eq!(drag.subject, DragSubject::Pane(PaneId(1)));
}

#[test]
fn interaction_update_promotes_pending_drag_after_threshold() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let frame = tree.layout(input());
    let pending =
        begin_pending_drag(&frame, Point::new(20.0, 20.0), DragIntent::Move, 5.0).unwrap();
    let mut state = InteractionState::PendingDrag(pending);
    let options = InteractionOptions::from_layout_input(input());

    let before = update_interaction(&tree, &frame, &mut state, Point::new(23.0, 23.0), &options);
    assert!(before.proposal.is_none());
    assert!(matches!(state, InteractionState::PendingDrag(_)));

    let after = update_interaction(&tree, &frame, &mut state, Point::new(200.0, 5.0), &options);
    assert!(matches!(state, InteractionState::Drag(_)));
    assert!(matches!(after.proposal, Some(Proposal::Dock(_))));
    assert!(after.base_revision.is_some());
    assert!(after.preview.is_some());
}

#[test]
fn interaction_options_derive_preview_and_resize_inputs_from_layout() {
    let layout = input();
    let options = InteractionOptions::from_layout_input(layout);
    let preview_layout = options.drag.preview_layout.unwrap();

    assert_eq!(preview_layout.bounds, layout.bounds);
    assert_eq!(
        preview_layout.split_handle_thickness,
        layout.split_handle_thickness
    );
    assert_eq!(options.resize.min_pane_size, layout.min_pane_size);
}

#[test]
fn drag_local_pane_edge_target_wins_over_root_target() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let frame = tree.layout(input());
    let target_tile = frame
        .panes
        .iter()
        .find(|pane| pane.pane == PaneId(2))
        .unwrap()
        .tile;
    let mut drag = begin_drag(&frame, Point::new(20.0, 100.0), DragIntent::Move).unwrap();

    let update = update_drag(
        &tree,
        &frame,
        &mut drag,
        Point::new(200.0, 5.0),
        &DragOptions::default(),
    );

    assert!(matches!(
        update.proposal,
        Some(DockProposal::MovePane {
            target: DockTarget::Split {
                tile,
                axis: Axis::Vertical,
                placement: Placement::Before,
                ..
            },
            ..
        }) if tile == target_tile
    ));
    assert!(update.candidates.len() > update.overlay.drop_targets.len());
    assert_eq!(update.overlay.drop_targets.len(), 1);
    assert_eq!(
        update.overlay.ghost_rects,
        vec![GhostFrame {
            rect: Rect::new(155.0, 0.0, 300.0, 100.0),
            kind: GhostKind::PreviewPane,
        }]
    );
}

#[test]
fn overlapping_edge_targets_rank_by_nearest_edge() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let frame = tree.layout(input());
    let target_tile = frame
        .panes
        .iter()
        .find(|pane| pane.pane == PaneId(2))
        .unwrap()
        .tile;
    let mut drag = begin_drag(&frame, Point::new(20.0, 100.0), DragIntent::Move).unwrap();
    let options = DragOptions {
        edge_zone_fraction: 0.5,
        ..DragOptions::default()
    };

    let update = update_drag(&tree, &frame, &mut drag, Point::new(200.0, 5.0), &options);

    assert!(matches!(
        update.proposal,
        Some(DockProposal::MovePane {
            target: DockTarget::Split {
                tile,
                axis: Axis::Vertical,
                placement: Placement::Before,
                ..
            },
            ..
        }) if tile == target_tile
    ));
}

#[test]
fn dragging_pane_to_its_own_edge_is_invalid() {
    let tree = TileTree::single_pane(PaneId(1));
    let frame = tree.layout(input());
    let mut drag = begin_drag(&frame, Point::new(20.0, 20.0), DragIntent::Move).unwrap();

    let update = update_drag(
        &tree,
        &frame,
        &mut drag,
        Point::new(5.0, 100.0),
        &DragOptions::default(),
    );

    assert!(update.proposal.is_none());
    assert!(update.overlay.active_target.is_none());
    assert_eq!(
        update.overlay.ghost_rects,
        vec![GhostFrame {
            rect: Rect::new(0.0, 0.0, 150.0, 200.0),
            kind: GhostKind::Invalid,
        }]
    );
}

#[test]
fn dragging_active_pane_from_tab_group_can_split_group() {
    let tree = TileTree::new(TileNode::tabs(vec![PaneId(1), PaneId(2)]));
    let frame = tree.layout(input());
    let mut drag = begin_drag(&frame, Point::new(20.0, 50.0), DragIntent::Move).unwrap();

    let update = update_drag(
        &tree,
        &frame,
        &mut drag,
        Point::new(5.0, 100.0),
        &DragOptions::default(),
    );

    assert!(matches!(
        update.proposal,
        Some(DockProposal::MovePane {
            pane: PaneId(1),
            target: DockTarget::Split {
                tile,
                axis: Axis::Horizontal,
                placement: Placement::Before,
                ..
            },
        }) if tile == tree.root()
    ));
}

#[test]
fn drag_over_tab_group_body_proposes_tab_into_group() {
    let mut tree = TileTree::new(TileNode::tabs(vec![PaneId(1), PaneId(2)]));
    let group = tree.root();
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(3),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let frame = tree.layout(input());
    let mut drag = begin_drag(&frame, Point::new(250.0, 100.0), DragIntent::Move).unwrap();

    let update = update_drag(
        &tree,
        &frame,
        &mut drag,
        Point::new(80.0, 100.0),
        &DragOptions::default(),
    );

    assert!(matches!(
        update.proposal,
        Some(DockProposal::MovePane {
            pane: PaneId(3),
            target: DockTarget::TabInto { group: target, index: None },
        }) if target == group
    ));
}

#[test]
fn unsupported_tab_group_drag_targets_are_invalid() {
    let tree = TileTree::new(TileNode::tabs(vec![PaneId(1), PaneId(2)]));
    let frame = tree.layout(input());
    let mut drag = DragSession {
        subject: DragSubject::TabGroup(tree.root()),
        source: DragSource::TabBar { group: tree.root() },
        origin: Point::new(10.0, 10.0),
        current: Point::new(10.0, 10.0),
        base_revision: frame.revision,
        proposal: None,
    };

    let update = update_drag(
        &tree,
        &frame,
        &mut drag,
        Point::new(5.0, 100.0),
        &DragOptions::default(),
    );

    assert!(update.proposal.is_none());
    assert!(update.overlay.active_target.is_none());
    assert!(update.candidates.is_empty());
    assert!(update.overlay.drop_targets.is_empty());
    assert!(update.overlay.ghost_rects.is_empty());
}

#[test]
fn resize_update_uses_solved_geometry_and_returns_preview() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let frame = tree.layout(input());
    let mut resize = begin_resize(&frame, Point::new(150.0, 100.0)).unwrap();

    let update = update_resize(
        &tree,
        &frame,
        &mut resize,
        Point::new(200.0, 100.0),
        &ResizeOptions::default(),
    );

    let proposal = update.proposal.unwrap();
    assert_eq!(proposal.delta, 50.0);
    assert_eq!(proposal.new_shares, vec![195.0, 95.0]);

    let preview = update.preview.unwrap();
    assert_eq!(preview.panes[0].rect.width(), 195.0);
    assert_eq!(preview.split_handles[0].rect.x0, 195.0);
    assert_eq!(preview.panes[1].rect.width(), 95.0);
}

#[test]
fn resize_update_clamps_to_min_pane_size() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let frame = tree.layout(input());
    let mut resize = begin_resize(&frame, Point::new(150.0, 100.0)).unwrap();

    let update = update_resize(
        &tree,
        &frame,
        &mut resize,
        Point::new(500.0, 100.0),
        &ResizeOptions::default(),
    );

    let proposal = update.proposal.unwrap();
    assert_eq!(proposal.delta, 125.0);
    assert_eq!(proposal.new_shares, vec![270.0, 20.0]);

    let preview = update.preview.unwrap();
    assert_eq!(preview.panes[0].rect.width(), 270.0);
    assert_eq!(preview.panes[1].rect.width(), 20.0);
}

#[test]
fn validate_resize_requires_frame_context() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();

    let result = validate_proposal(ProposalValidationInput::new(
        &tree,
        Proposal::Resize(ResizeProposal {
            split: tree.root(),
            handle: 0,
            delta: 50.0,
            new_shares: vec![195.0, 95.0],
        }),
        &DockPolicyData::default(),
    ));

    assert_eq!(result.unwrap_err(), TileError::InvalidOperation);
}

#[test]
fn validate_resize_rejects_min_size_violation() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let frame = tree.layout(input());

    let result = validate_proposal(
        ProposalValidationInput::new(
            &tree,
            Proposal::Resize(ResizeProposal {
                split: tree.root(),
                handle: 0,
                delta: 135.0,
                new_shares: vec![280.0, 10.0],
            }),
            &DockPolicyData::default(),
        )
        .with_frame(&frame),
    );

    assert_eq!(result.unwrap_err(), TileError::InvalidOperation);
}

#[test]
fn validate_resize_accepts_frame_checked_proposal() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let frame = tree.layout(input());
    let mut resize = begin_resize(&frame, Point::new(150.0, 100.0)).unwrap();
    let update = update_resize(
        &tree,
        &frame,
        &mut resize,
        Point::new(200.0, 100.0),
        &ResizeOptions::default(),
    );

    let validated = validate_proposal(
        ProposalValidationInput::new(
            &tree,
            Proposal::Resize(update.proposal.unwrap()),
            &DockPolicyData::default(),
        )
        .with_frame(&frame),
    )
    .unwrap();

    assert!(matches!(validated.op, TileOp::SetSplitShares { .. }));
}

#[test]
fn tab_insert_threshold_controls_reorder_index() {
    let tree = TileTree::new(TileNode::tabs(vec![PaneId(1), PaneId(2)]));
    let frame = tree.layout(input());
    let mut drag = begin_drag(&frame, Point::new(225.0, 10.0), DragIntent::Move).unwrap();
    let options = DragOptions {
        allow_split: false,
        tab_insert_threshold: 0.75,
        ..DragOptions::default()
    };

    let update = update_drag(&tree, &frame, &mut drag, Point::new(100.0, 10.0), &options);
    assert!(matches!(
        update.proposal,
        Some(DockProposal::ReorderTab { index: 0, .. })
    ));

    let options = DragOptions {
        tab_insert_threshold: 0.25,
        ..options
    };
    let update = update_drag(&tree, &frame, &mut drag, Point::new(100.0, 10.0), &options);
    assert!(matches!(
        update.proposal,
        Some(DockProposal::ReorderTab { index: 1, .. })
    ));
}

#[test]
fn stale_drag_commit_is_rejected() {
    let mut tree = TileTree::single_pane(PaneId(1));
    let frame = tree.layout(input());
    let mut drag = begin_drag(&frame, Point::new(20.0, 20.0), DragIntent::Move).unwrap();
    drag.proposal = Some(DockProposal::MovePane {
        pane: PaneId(1),
        target: DockTarget::Root,
    });
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    assert!(matches!(
        commit_drag(&mut tree, drag),
        Err(TileError::StaleInteraction)
    ));
}

#[test]
fn validate_proposal_rejects_locked_layout() {
    let tree = TileTree::new(TileNode::tabs(vec![PaneId(1), PaneId(2)]));
    let group = tree.root();
    let policy = DockPolicyData {
        locked_layout: true,
        ..DockPolicyData::default()
    };

    let result = validate_proposal(ProposalValidationInput::new(
        &tree,
        Proposal::Dock(DockProposal::ReorderTab {
            group,
            pane: PaneId(2),
            index: 0,
        }),
        &policy,
    ));

    assert_eq!(result.unwrap_err(), TileError::PolicyRejected);
}

#[test]
fn validate_proposal_rejects_invalid_target() {
    let tree = TileTree::single_pane(PaneId(1));

    let result = validate_proposal(ProposalValidationInput::new(
        &tree,
        Proposal::Dock(DockProposal::MovePane {
            pane: PaneId(1),
            target: DockTarget::TabInto {
                group: TileId(999),
                index: None,
            },
        }),
        &DockPolicyData::default(),
    ));

    assert_eq!(result.unwrap_err(), TileError::InvalidTileId);
}

#[test]
fn validate_proposal_rejects_disallowed_tab_zone() {
    let mut tree = TileTree::new(TileNode::tabs(vec![PaneId(1)]));
    let group = tree.root();
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let policy = DockPolicyData {
        default_pane_capabilities: PaneCapabilities {
            allowed_zones: ZoneSet::SPLIT,
            ..PaneCapabilities::default()
        },
        ..DockPolicyData::default()
    };

    let result = validate_proposal(ProposalValidationInput::new(
        &tree,
        Proposal::Dock(DockProposal::MovePane {
            pane: PaneId(2),
            target: DockTarget::TabInto { group, index: None },
        }),
        &policy,
    ));

    assert_eq!(result.unwrap_err(), TileError::PolicyRejected);
}

#[test]
fn validate_proposal_rejects_disallowed_split_edge() {
    let tree = TileTree::new(TileNode::tabs(vec![PaneId(1), PaneId(2)]));
    let group = tree.root();
    let policy = DockPolicyData {
        default_pane_capabilities: PaneCapabilities {
            allowed_edges: EdgeSet::RIGHT,
            ..PaneCapabilities::default()
        },
        ..DockPolicyData::default()
    };

    let result = validate_proposal(ProposalValidationInput::new(
        &tree,
        Proposal::Dock(DockProposal::MovePane {
            pane: PaneId(2),
            target: DockTarget::Split {
                tile: group,
                axis: Axis::Horizontal,
                placement: Placement::Before,
                ratio: 0.5,
            },
        }),
        &policy,
    ));

    assert_eq!(result.unwrap_err(), TileError::PolicyRejected);
}

#[test]
fn commit_proposal_applies_validated_operation() {
    let mut tree = TileTree::new(TileNode::tabs(vec![PaneId(1), PaneId(2)]));
    let group = tree.root();

    let proposal = validate_proposal(ProposalValidationInput::new(
        &tree,
        Proposal::Dock(DockProposal::ReorderTab {
            group,
            pane: PaneId(2),
            index: 0,
        }),
        &DockPolicyData::default(),
    ))
    .unwrap();
    let Some(TileNode::Tabs(tabs)) = tree.node(group) else {
        panic!("group should remain tabs");
    };
    assert_eq!(tabs.panes, vec![PaneId(1), PaneId(2)]);

    let op = commit_proposal(&mut tree, proposal).unwrap();

    assert!(matches!(op, TileOp::ReorderTab { .. }));
    let Some(TileNode::Tabs(tabs)) = tree.node(group) else {
        panic!("group should remain tabs");
    };
    assert_eq!(tabs.panes, vec![PaneId(2), PaneId(1)]);
}

#[test]
fn interaction_update_validates_and_commits_resize_proposal() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let frame = tree.layout(input());
    let resize = begin_resize(&frame, Point::new(150.0, 100.0)).unwrap();
    let mut state = InteractionState::Resize(resize);
    let options = InteractionOptions::from_layout_input(input());

    let update = update_interaction(
        &tree,
        &frame,
        &mut state,
        Point::new(200.0, 100.0),
        &options,
    );
    assert!(matches!(update.proposal, Some(Proposal::Resize(_))));
    assert!(update.preview.is_some());

    let policy = DockPolicyData::default();
    let validation = ProposalValidationInput::from_interaction_update(&tree, &update, &policy)
        .unwrap()
        .with_frame(&frame)
        .with_interaction_options(&options);
    let proposal = validate_proposal(validation).unwrap();
    let op = commit_proposal(&mut tree, proposal).unwrap();

    assert!(matches!(op, TileOp::SetSplitShares { .. }));
}

#[test]
fn validate_proposal_rejects_stale_interaction_revision() {
    let tree = TileTree::single_pane(PaneId(1));

    let result = validate_proposal(
        ProposalValidationInput::new(
            &tree,
            Proposal::Dock(DockProposal::MovePane {
                pane: PaneId(1),
                target: DockTarget::Root,
            }),
            &DockPolicyData::default(),
        )
        .with_base_revision(Revision(99)),
    );

    assert_eq!(result.unwrap_err(), TileError::StaleInteraction);
}

#[test]
fn repair_reports_and_applies_persisted_layout_fixes() {
    let mut tree = TileTree::from_raw_parts_for_test(
        TileId(0),
        Revision(7),
        vec![
            Some(TileNode::Split(SplitNode {
                axis: Axis::Horizontal,
                children: vec![TileId(1), TileId(2), TileId(99)],
                shares: vec![f64::NAN],
                constraints: SplitConstraints::default(),
            })),
            Some(TileNode::Tabs(TabNode {
                panes: vec![PaneId(1), PaneId(2)],
                active: 99,
                placement: TabBarPlacement::Top,
            })),
            Some(TileNode::Split(SplitNode {
                axis: Axis::Vertical,
                children: vec![TileId(3)],
                shares: vec![1.0],
                constraints: SplitConstraints::default(),
            })),
            Some(TileNode::pane(PaneId(3))),
            Some(TileNode::pane(PaneId(4))),
        ],
    );

    let report = tree.repair();
    let actions = &report.actions;

    assert!(actions.contains(&RepairAction::RepairedShares(TileId(0))));
    assert!(actions.contains(&RepairAction::RepairedActiveTab(TileId(1))));
    assert!(actions.contains(&RepairAction::CollapsedSplit(TileId(2))));
    assert!(actions.contains(&RepairAction::RemovedInvalidNode(TileId(99))));
    assert!(actions.contains(&RepairAction::RemovedInvalidNode(TileId(4))));

    let Some(TileNode::Split(root)) = tree.node(TileId(0)) else {
        panic!("root should remain a split");
    };
    assert_eq!(root.children, vec![TileId(1), TileId(3)]);
    assert_eq!(root.shares, vec![1.0, 1.0]);

    let Some(TileNode::Tabs(tabs)) = tree.node(TileId(1)) else {
        panic!("first child should remain tabs");
    };
    assert_eq!(tabs.active, 0);
    assert!(tree.node(TileId(2)).is_none());
    assert!(tree.node(TileId(4)).is_none());
}

#[test]
fn repair_reports_same_axis_split_merges() {
    let mut tree = TileTree::from_raw_parts_for_test(
        TileId(0),
        Revision(0),
        vec![
            Some(TileNode::Split(SplitNode {
                axis: Axis::Horizontal,
                children: vec![TileId(1), TileId(4)],
                shares: vec![1.0, 1.0],
                constraints: SplitConstraints::default(),
            })),
            Some(TileNode::Split(SplitNode {
                axis: Axis::Horizontal,
                children: vec![TileId(2), TileId(3)],
                shares: vec![2.0, 1.0],
                constraints: SplitConstraints::default(),
            })),
            Some(TileNode::pane(PaneId(1))),
            Some(TileNode::pane(PaneId(2))),
            Some(TileNode::pane(PaneId(3))),
        ],
    );

    let report = tree.repair();
    let actions = &report.actions;

    assert!(actions.contains(&RepairAction::CollapsedSplit(TileId(1))));
    assert!(actions.contains(&RepairAction::RepairedShares(TileId(0))));

    let Some(TileNode::Split(root)) = tree.node(TileId(0)) else {
        panic!("root should remain a split");
    };
    assert_eq!(root.children, vec![TileId(2), TileId(3), TileId(4)]);
    assert_eq!(root.shares, vec![2.0, 1.0, 1.0]);
    assert!(tree.node(TileId(1)).is_none());
}

#[test]
fn restore_snapshot_repairs_only_when_requested() {
    let snapshot = LayoutSnapshot {
        schema_version: 1,
        tree: TileTree::from_raw_parts_for_test(
            TileId(0),
            Revision(0),
            vec![Some(TileNode::Tabs(TabNode {
                panes: vec![PaneId(1)],
                active: 99,
                placement: TabBarPlacement::Top,
            }))],
        ),
        active_pane: Some(PaneId(1)),
        closed_panes: Vec::new(),
    };

    let restored = restore_snapshot(
        snapshot.clone(),
        RestoreOptions {
            repair_missing_panes: false,
            drop_unknown_panes: false,
            normalize: false,
        },
    )
    .unwrap();
    let Some(TileNode::Tabs(tabs)) = restored.node(TileId(0)) else {
        panic!("root should remain tabs");
    };
    assert_eq!(tabs.active, 99);

    let restored = restore_snapshot(
        snapshot,
        RestoreOptions {
            repair_missing_panes: false,
            drop_unknown_panes: false,
            normalize: true,
        },
    )
    .unwrap();
    let Some(TileNode::Tabs(tabs)) = restored.node(TileId(0)) else {
        panic!("root should remain tabs");
    };
    assert_eq!(tabs.active, 0);
}

#[cfg(feature = "serde")]
#[test]
fn serde_covers_public_data_types() {
    fn assert_serde<T>()
    where
        T: serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
    }

    assert_serde::<TileId>();
    assert_serde::<PaneId>();
    assert_serde::<SurfaceId>();
    assert_serde::<Revision>();

    assert_serde::<Axis>();
    assert_serde::<Placement>();
    assert_serde::<TabBarPlacement>();
    assert_serde::<LayoutConstraints>();
    assert_serde::<SplitConstraints>();
    assert_serde::<TileNode>();
    assert_serde::<SplitNode>();
    assert_serde::<TabNode>();
    assert_serde::<PaneNode>();
    assert_serde::<SurfaceKind>();
    assert_serde::<TileSurface>();
    assert_serde::<LayoutInput>();
    assert_serde::<TileTree>();

    assert_serde::<LayoutFrame>();
    assert_serde::<FrameDiff>();
    assert_serde::<FrameItemDiff>();
    assert_serde::<FrameChange>();
    assert_serde::<FrameTransitionHint>();
    assert_serde::<PaneFrame>();
    assert_serde::<TabBarFrame>();
    assert_serde::<TabFrame>();
    assert_serde::<SplitChildFrame>();
    assert_serde::<SplitHandleFrame>();
    assert_serde::<FrameItemId>();
    assert_serde::<HitRegion>();
    assert_serde::<HitKind>();

    assert_serde::<DockTarget>();
    assert_serde::<TileOp>();
    assert_serde::<TileError>();
    assert_serde::<LayoutSnapshot>();
    assert_serde::<RestoreOptions>();
    assert_serde::<RepairReport>();
    assert_serde::<RepairAction>();

    assert_serde::<PaneCapabilities>();
    assert_serde::<EdgeSet>();
    assert_serde::<ZoneSet>();
    assert_serde::<DockPolicyData>();
    assert_serde::<ValidatedProposal>();

    assert_serde::<DragIntent>();
    assert_serde::<InteractionState>();
    assert_serde::<InteractionUpdate>();
    assert_serde::<DragSession>();
    assert_serde::<PendingDrag>();
    assert_serde::<DragSubject>();
    assert_serde::<DragSource>();
    assert_serde::<ResizeSession>();
    assert_serde::<InteractionFrame>();
    assert_serde::<OverlayFrame>();
    assert_serde::<OverlayHitRegion>();
    assert_serde::<DropTargetId>();
    assert_serde::<DropTargetFrame>();
    assert_serde::<GhostFrame>();
    assert_serde::<GhostKind>();
    assert_serde::<DraggedFrame>();
    assert_serde::<Proposal>();
    assert_serde::<DockProposal>();
    assert_serde::<ResizeProposal>();
    assert_serde::<DragUpdate>();
    assert_serde::<ResizeUpdate>();
    assert_serde::<CommitMode>();
    assert_serde::<ResizeOptions>();
    assert_serde::<DragOptions>();
    assert_serde::<InteractionOptions>();
}
