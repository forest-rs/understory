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
    assert_eq!(frame.split_handles.len(), 1);
    assert_eq!(frame.panes[0].rect.width(), 145.0);
    assert_eq!(frame.split_handles[0].rect.width(), 10.0);
    assert_eq!(frame.panes[1].rect.width(), 145.0);
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

    let result = validate_proposal(
        &tree,
        Proposal::Dock(DockProposal::ReorderTab {
            group,
            pane: PaneId(2),
            index: 0,
        }),
        &policy,
    );

    assert_eq!(result.unwrap_err(), TileError::PolicyRejected);
}

#[test]
fn validate_proposal_rejects_invalid_target() {
    let tree = TileTree::single_pane(PaneId(1));

    let result = validate_proposal(
        &tree,
        Proposal::Dock(DockProposal::MovePane {
            pane: PaneId(1),
            target: DockTarget::TabInto {
                group: TileId(999),
                index: None,
            },
        }),
        &DockPolicyData::default(),
    );

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

    let result = validate_proposal(
        &tree,
        Proposal::Dock(DockProposal::MovePane {
            pane: PaneId(2),
            target: DockTarget::TabInto { group, index: None },
        }),
        &policy,
    );

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

    let result = validate_proposal(
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
    );

    assert_eq!(result.unwrap_err(), TileError::PolicyRejected);
}

#[test]
fn commit_proposal_applies_validated_operation() {
    let mut tree = TileTree::new(TileNode::tabs(vec![PaneId(1), PaneId(2)]));
    let group = tree.root();

    let proposal = validate_proposal(
        &tree,
        Proposal::Dock(DockProposal::ReorderTab {
            group,
            pane: PaneId(2),
            index: 0,
        }),
        &DockPolicyData::default(),
    )
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
