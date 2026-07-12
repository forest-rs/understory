// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;

use crate::snapshot::RepairAction;
use crate::*;

fn input() -> LayoutInput {
    LayoutInput {
        bounds: Rect::new(0.0, 0.0, 300.0, 200.0),
        tab_bar_thickness: 20.0,
        split_handle_thickness: 10.0,
        min_pane_size: Size::new(20.0, 20.0),
    }
}

fn tab_tree(panes: impl IntoIterator<Item = PaneId>) -> TileTree {
    TileTree::from_spec(TileSpec::tabs(panes)).unwrap()
}

fn projected(tree: &TileTree, pane: PaneId) -> LayoutFrame {
    let mut frame = tree.layout(input());
    frame
        .apply_projection(FrameProjectionInput::Zoom {
            focus: pane,
            projected_rect: input().bounds,
        })
        .unwrap();
    frame
}

fn interaction_options() -> InteractionOptions {
    InteractionOptions {
        drag_threshold: 0.0,
        ..InteractionOptions::from_layout_input(input())
    }
}

fn interaction_options_with_drag(drag: DragOptions) -> InteractionOptions {
    InteractionOptions {
        drag,
        ..interaction_options()
    }
}

fn pointer_update(
    tree: &TileTree,
    frame: &LayoutFrame,
    origin: Point,
    point: Point,
    options: &InteractionOptions,
) -> InteractionUpdate {
    let mut state = begin_interaction(frame, origin, options);
    update_interaction(tree, frame, &mut state, point, options)
}

fn dock_proposal(update: &InteractionUpdate) -> Option<&DockProposal> {
    match &update.proposal {
        Some(Proposal::Dock(proposal)) => Some(proposal),
        _ => None,
    }
}

fn resize_proposal(update: &InteractionUpdate) -> Option<&ResizeProposal> {
    match &update.proposal {
        Some(Proposal::Resize(proposal)) => Some(proposal),
        _ => None,
    }
}

fn diff_item(diff: &FrameDiff, item: FrameItemId) -> &FrameItemDiff {
    diff.items
        .iter()
        .find(|candidate| candidate.item == item)
        .unwrap_or_else(|| panic!("missing diff item {item:?}"))
}

#[test]
fn single_pane_fills_bounds() {
    let tree = TileTree::single_pane(PaneId(1));
    let frame = tree.layout(input());
    assert_eq!(frame.panes.len(), 1);
    assert_eq!(frame.panes[0].rect, input().bounds);
}

#[test]
fn semantic_panes_include_leaves_and_inactive_tabs() {
    let single = TileTree::single_pane(PaneId(1));
    assert_eq!(single.panes().collect::<Vec<_>>(), vec![PaneId(1)]);

    let tabs = TileTree::from_spec(TileSpec::tabs_with(
        [PaneId(2), PaneId(3), PaneId(4)],
        1,
        TabBarPlacement::Top,
    ))
    .unwrap();
    assert_eq!(
        tabs.panes().collect::<Vec<_>>(),
        vec![PaneId(2), PaneId(3), PaneId(4)]
    );
}

fn representative_spec() -> TileSpec {
    TileSpec::split(
        Axis::Horizontal,
        [
            TileSpec::tabs([PaneId(1), PaneId(2)]),
            TileSpec::tabs_with([PaneId(3), PaneId(4)], 1, TabBarPlacement::Bottom),
        ],
        [0.6, 0.4],
    )
}

#[test]
fn tree_spec_builds_independent_tab_groups_in_semantic_order() {
    let tree = TileTree::from_spec(representative_spec()).unwrap();

    assert_eq!(tree.root(), TileId(0));
    assert_eq!(
        tree.panes().collect::<Vec<_>>(),
        vec![PaneId(1), PaneId(2), PaneId(3), PaneId(4)]
    );
    let Some(TileNode::Split(split)) = tree.node(tree.root()) else {
        panic!("root should be a split");
    };
    assert_eq!(split.children, vec![TileId(1), TileId(2)]);
    assert_eq!(split.shares, vec![0.6, 0.4]);

    let frame = tree.layout(input());
    assert_eq!(frame.tab_bars.len(), 2);
    assert_eq!(frame.tabs.len(), 4);
}

#[test]
fn tree_spec_preserves_deep_semantic_order() {
    let tree = TileTree::from_spec(TileSpec::split(
        Axis::Horizontal,
        [
            TileSpec::pane(PaneId(1)),
            TileSpec::split(
                Axis::Vertical,
                [
                    TileSpec::tabs([PaneId(2), PaneId(3)]),
                    TileSpec::pane(PaneId(4)),
                ],
                [1.0, 1.0],
            ),
        ],
        [1.0, 2.0],
    ))
    .unwrap();

    assert_eq!(
        tree.panes().collect::<Vec<_>>(),
        vec![PaneId(1), PaneId(2), PaneId(3), PaneId(4)]
    );
}

#[test]
fn tree_spec_rejects_duplicate_and_invalid_tab_data() {
    let duplicate = TileSpec::equal_split(
        Axis::Horizontal,
        [
            TileSpec::tabs([PaneId(1), PaneId(2)]),
            TileSpec::pane(PaneId(2)),
        ],
    );
    assert_eq!(
        TileTree::from_spec(duplicate).unwrap_err(),
        TileSpecError::DuplicatePane(PaneId(2))
    );
    assert_eq!(
        TileTree::from_spec(TileSpec::tabs([])).unwrap_err(),
        TileSpecError::EmptyTabs
    );
    assert_eq!(
        TileTree::from_spec(TileSpec::tabs_with(
            [PaneId(1), PaneId(2)],
            2,
            TabBarPlacement::Left,
        ))
        .unwrap_err(),
        TileSpecError::InvalidActiveTab {
            active: 2,
            panes: 2,
        }
    );
}

#[test]
fn tree_spec_rejects_invalid_split_data() {
    assert_eq!(
        TileTree::from_spec(TileSpec::equal_split(Axis::Horizontal, [])).unwrap_err(),
        TileSpecError::TooFewSplitChildren { children: 0 }
    );
    assert_eq!(
        TileTree::from_spec(TileSpec::equal_split(
            Axis::Horizontal,
            [TileSpec::pane(PaneId(1))],
        ))
        .unwrap_err(),
        TileSpecError::TooFewSplitChildren { children: 1 }
    );
    assert_eq!(
        TileTree::from_spec(TileSpec::split(
            Axis::Horizontal,
            [TileSpec::pane(PaneId(1)), TileSpec::pane(PaneId(2))],
            [1.0],
        ))
        .unwrap_err(),
        TileSpecError::ShareCountMismatch {
            children: 2,
            shares: 1,
        }
    );
    assert_eq!(
        TileTree::from_spec(TileSpec::split(
            Axis::Horizontal,
            [TileSpec::pane(PaneId(1)), TileSpec::pane(PaneId(2))],
            [1.0, f64::NAN],
        ))
        .unwrap_err(),
        TileSpecError::InvalidShare { index: 1 }
    );

    let constrained = TileSpec::Split {
        axis: Axis::Horizontal,
        children: vec![TileSpec::pane(PaneId(1)), TileSpec::pane(PaneId(2))],
        shares: vec![1.0, 1.0],
        constraints: SplitConstraints {
            min_major: vec![10.0, f64::INFINITY],
        },
    };
    assert_eq!(
        TileTree::from_spec(constrained).unwrap_err(),
        TileSpecError::InvalidSplitConstraint { index: 1 }
    );
}

#[test]
fn snapshot_restore_preserves_semantic_panes() {
    let tree = TileTree::from_spec(representative_spec()).unwrap();
    let expected = tree.panes().collect::<Vec<_>>();
    let restored = restore_snapshot(LayoutSnapshot::new(tree)).unwrap();

    assert_eq!(restored.tree.panes().collect::<Vec<_>>(), expected);
    assert!(restored.repairs.is_empty());
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

    assert_eq!(
        diff_item(&diff, FrameItemId::Pane(PaneId(1))).change,
        FrameChange::Resized
    );
    assert!(matches!(
        diff_item(&diff, FrameItemId::Pane(PaneId(2))),
        FrameItemDiff {
            change: FrameChange::Added,
            motion: FrameMotion::Enter {
                from: Some(rect),
                anchor: Some(FrameItemId::Pane(PaneId(1))),
            },
            cause: FrameCause::Relayout,
            ..
        } if *rect == input().bounds
    ));
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
fn frame_diff_reports_resized_split_handles_and_children() {
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

    assert_eq!(
        diff_item(&diff, FrameItemId::Pane(PaneId(2))).change,
        FrameChange::MovedAndResized
    );
    assert_eq!(
        diff_item(
            &diff,
            FrameItemId::SplitHandle {
                split: tree.root(),
                handle: 0,
            },
        )
        .change,
        FrameChange::Moved
    );
    assert_eq!(
        diff_item(
            &diff,
            FrameItemId::SplitChild {
                split: tree.root(),
                child: before.split_children[1].child,
            },
        )
        .change,
        FrameChange::MovedAndResized
    );
}

#[test]
fn frame_diff_reports_removed_items_with_exit_hints() {
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

    tree.apply(TileOp::ClosePane { pane: PaneId(2) }).unwrap();
    let after = tree.layout(input());

    let diff = diff_frames(&before, &after);

    assert!(matches!(
        diff_item(&diff, FrameItemId::Pane(PaneId(2))),
        FrameItemDiff {
            change: FrameChange::Removed,
            before: Some(_),
            after: None,
            motion: FrameMotion::Exit {
                to: Some(rect),
                anchor: Some(FrameItemId::Pane(PaneId(1))),
            },
            cause: FrameCause::Relayout,
            ..
        } if *rect == after.panes[0].rect
    ));
    assert!(diff.items.iter().any(|item| matches!(
        item,
        FrameItemDiff {
            item: FrameItemId::SplitHandle { handle: 0, .. },
            change: FrameChange::Removed,
            ..
        }
    )));
}

#[test]
fn zoomed_layout_projects_visible_pane_to_bounds() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();

    let normal = tree.layout(input());
    let source = normal
        .panes
        .iter()
        .find(|pane| pane.pane == PaneId(2))
        .copied()
        .expect("pane 2 should be visible before zoom");
    let zoomed = projected(&tree, PaneId(2));

    let projection = zoomed.projection.as_ref().expect("projection metadata");
    assert_eq!(projection.kind, ProjectionKind::Zoom);
    assert_eq!(projection.focus, PaneId(2));
    assert_eq!(projection.focus_tile, source.tile);
    assert_eq!(projection.source_rect, source.rect);
    assert_eq!(projection.projected_rect, input().bounds);
    assert!(
        projection
            .hidden_items
            .iter()
            .any(|hidden| hidden.item == FrameItemId::Pane(PaneId(1)))
    );
    assert!(
        projection
            .hidden_items
            .iter()
            .any(|hidden| matches!(hidden.item, FrameItemId::SplitHandle { handle: 0, .. }))
    );
    assert_eq!(zoomed.panes.len(), 1);
    assert_eq!(zoomed.panes[0].pane, PaneId(2));
    assert_eq!(zoomed.panes[0].rect, input().bounds);
    assert!(zoomed.tab_bars.is_empty());
    assert!(zoomed.tabs.is_empty());
    assert!(zoomed.split_children.is_empty());
    assert!(zoomed.split_handles.is_empty());
    assert_eq!(zoomed.focus_order, vec![PaneId(2)]);
    assert_eq!(
        hit_test(&zoomed, Point::new(10.0, 10.0)),
        Some(HitKind::Pane { pane: PaneId(2) })
    );
}

#[test]
fn invalid_projection_is_transactional() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();

    let mut frame = tree.layout(input());
    let original = frame.clone();
    assert_eq!(
        frame.apply_projection(FrameProjectionInput::Zoom {
            focus: PaneId(99),
            projected_rect: input().bounds,
        }),
        Err(FrameProjectionError::UnknownPane(PaneId(99)))
    );

    assert!(frame.projection.is_none());
    assert_eq!(frame.panes, original.panes);
    assert_eq!(frame.split_handles, original.split_handles);
}

#[test]
fn projection_records_resolved_tab_geometry() {
    let tree = tab_tree([PaneId(1), PaneId(2)]);
    let group = tree.root();
    let mut frame = tree.layout(input());
    frame
        .apply_tab_group_geometry(&[resolved_tabs(group)])
        .unwrap();
    frame
        .apply_projection(FrameProjectionInput::Zoom {
            focus: PaneId(1),
            projected_rect: input().bounds,
        })
        .unwrap();

    let projection = frame.projection().unwrap();
    assert!(projection.hidden_items.iter().any(|hidden| {
        hidden.item
            == FrameItemId::Tab {
                group,
                pane: PaneId(1),
            }
            && hidden.rect == Rect::new(0.0, 0.0, 200.0, 20.0)
    }));
}

#[test]
fn frame_diff_reports_zoom_and_restore_hints() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();

    let normal = tree.layout(input());
    let zoomed = projected(&tree, PaneId(2));
    let source = zoomed
        .projection
        .as_ref()
        .expect("projection metadata")
        .source_rect;
    let zoom_diff = diff_frames(&normal, &zoomed);

    // The focus pane is a stable item; its own before rect carries the origin.
    assert!(matches!(
        diff_item(&zoom_diff, FrameItemId::Pane(PaneId(2))),
        FrameItemDiff {
            change: FrameChange::MovedAndResized,
            before: Some(rect),
            motion: FrameMotion::Stable,
            cause: FrameCause::Projection {
                kind: ProjectionKind::Zoom,
                focus: PaneId(2),
                event: ProjectionEvent::Project,
            },
            ..
        } if *rect == source
    ));
    assert!(matches!(
        diff_item(&zoom_diff, FrameItemId::Pane(PaneId(1))),
        FrameItemDiff {
            change: FrameChange::Removed,
            motion: FrameMotion::Exit {
                to: Some(rect),
                anchor: Some(FrameItemId::Pane(PaneId(2))),
            },
            cause: FrameCause::Projection {
                kind: ProjectionKind::Zoom,
                focus: PaneId(2),
                event: ProjectionEvent::Hide,
            },
            ..
        } if *rect == source
    ));

    let restore_diff = diff_frames(&zoomed, &normal);
    assert!(matches!(
        diff_item(&restore_diff, FrameItemId::Pane(PaneId(2))),
        FrameItemDiff {
            change: FrameChange::MovedAndResized,
            after: Some(rect),
            motion: FrameMotion::Stable,
            cause: FrameCause::Projection {
                kind: ProjectionKind::Zoom,
                focus: PaneId(2),
                event: ProjectionEvent::Restore,
            },
            ..
        } if *rect == source
    ));
    assert!(matches!(
        diff_item(&restore_diff, FrameItemId::Pane(PaneId(1))),
        FrameItemDiff {
            change: FrameChange::Added,
            motion: FrameMotion::Enter {
                from: Some(rect),
                anchor: Some(FrameItemId::Pane(PaneId(2))),
            },
            cause: FrameCause::Projection {
                kind: ProjectionKind::Zoom,
                focus: PaneId(2),
                event: ProjectionEvent::Reveal,
            },
            ..
        } if *rect == source
    ));
    assert!(restore_diff.items.iter().any(|item| matches!(
        item,
        FrameItemDiff {
            item: FrameItemId::SplitHandle { handle: 0, .. },
            change: FrameChange::Added,
            motion: FrameMotion::Enter {
                from: Some(rect),
                anchor: Some(FrameItemId::Pane(PaneId(2))),
            },
            cause: FrameCause::Projection {
                kind: ProjectionKind::Zoom,
                focus: PaneId(2),
                event: ProjectionEvent::Reveal,
            },
            ..
        } if *rect == source
    )));
}

#[test]
fn restore_diff_does_not_mark_new_items_as_revealed_by_restore() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let zoomed = projected(&tree, PaneId(2));

    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Vertical,
        new_pane: PaneId(3),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let restored_with_new_pane = tree.layout(input());
    let diff = diff_frames(&zoomed, &restored_with_new_pane);

    assert!(matches!(
        diff_item(&diff, FrameItemId::Pane(PaneId(3))),
        FrameItemDiff {
            change: FrameChange::Added,
            motion: FrameMotion::Enter { .. },
            cause: FrameCause::Relayout,
            ..
        }
    ));
}

#[test]
fn zoom_diff_does_not_mark_unrelated_removals_as_hidden_by_zoom() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Vertical,
        new_pane: PaneId(3),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let normal = tree.layout(input());

    tree.apply(TileOp::ClosePane { pane: PaneId(3) }).unwrap();
    let zoomed_after_removal = projected(&tree, PaneId(2));
    let diff = diff_frames(&normal, &zoomed_after_removal);

    assert!(matches!(
        diff_item(&diff, FrameItemId::Pane(PaneId(3))),
        FrameItemDiff {
            change: FrameChange::Removed,
            motion: FrameMotion::Exit { .. },
            cause: FrameCause::Relayout,
            ..
        }
    ));
}

#[test]
fn zoom_switch_uses_each_pane_normal_rect() {
    let mut tree = TileTree::single_pane(PaneId(1));
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();

    let zoomed_one = projected(&tree, PaneId(1));
    let zoomed_two = projected(&tree, PaneId(2));
    let one_source = zoomed_one
        .projection
        .as_ref()
        .expect("first projection metadata")
        .source_rect;
    let two_source = zoomed_two
        .projection
        .as_ref()
        .expect("second projection metadata")
        .source_rect;
    let diff = diff_frames(&zoomed_one, &zoomed_two);

    // The outgoing focus pane restores to its own normal rectangle, …
    assert!(matches!(
        diff_item(&diff, FrameItemId::Pane(PaneId(1))),
        FrameItemDiff {
            change: FrameChange::Removed,
            motion: FrameMotion::Exit { to: Some(rect), anchor: None },
            cause: FrameCause::Projection {
                kind: ProjectionKind::Zoom,
                focus: PaneId(1),
                event: ProjectionEvent::Restore,
            },
            ..
        } if *rect == one_source
    ));
    // … while the incoming focus pane projects from its own normal rectangle.
    assert!(matches!(
        diff_item(&diff, FrameItemId::Pane(PaneId(2))),
        FrameItemDiff {
            change: FrameChange::Added,
            motion: FrameMotion::Enter { from: Some(rect), anchor: None },
            cause: FrameCause::Projection {
                kind: ProjectionKind::Zoom,
                focus: PaneId(2),
                event: ProjectionEvent::Project,
            },
            ..
        } if *rect == two_source
    ));
}

#[test]
fn frame_diff_tracks_tab_reorder_by_stable_tab_identity() {
    let mut tree = tab_tree([PaneId(1), PaneId(2), PaneId(3)]);
    let group = tree.root();
    let before = tree.layout(input());

    tree.apply(TileOp::ReorderTab {
        group,
        pane: PaneId(3),
        index: 0,
    })
    .unwrap();
    let after = tree.layout(input());

    let diff = diff_frames(&before, &after);
    let tab = FrameItemId::Tab {
        group,
        pane: PaneId(3),
    };

    assert_eq!(diff_item(&diff, tab).change, FrameChange::Moved);
    assert_eq!(diff_item(&diff, tab).before, Some(before.tabs[2].rect));
    assert_eq!(diff_item(&diff, tab).after, Some(after.tabs[0].rect));
}

#[test]
fn frame_diff_tracks_pane_moved_into_tab_group() {
    let mut tree = tab_tree([PaneId(1), PaneId(2)]);
    let group = tree.root();
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(3),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let before = tree.layout(input());

    tree.apply(TileOp::MovePane {
        pane: PaneId(3),
        target: DockTarget::TabInto {
            group,
            index: Some(1),
        },
    })
    .unwrap();
    let after = tree.layout(input());

    let diff = diff_frames(&before, &after);

    assert_eq!(
        diff_item(&diff, FrameItemId::Pane(PaneId(3))).change,
        FrameChange::MovedAndResized
    );
    assert_eq!(
        diff_item(
            &diff,
            FrameItemId::Tab {
                group,
                pane: PaneId(3),
            },
        )
        .change,
        FrameChange::Added
    );
    assert!(diff.items.iter().any(|item| matches!(
        item,
        FrameItemDiff {
            item: FrameItemId::SplitHandle { handle: 0, .. },
            change: FrameChange::Removed,
            ..
        }
    )));
}

#[test]
fn ids_and_errors_display_human_readable_labels() {
    assert_eq!(TileId(7).to_string(), "tile 7");
    assert_eq!(PaneId(9).to_string(), "pane 9");
    assert_eq!(TileError::StaleInteraction.to_string(), "stale interaction");
}

#[test]
fn tabs_emit_bar_tabs_and_active_pane() {
    let tree = tab_tree([PaneId(1), PaneId(2)]);
    let frame = tree.layout(input());
    assert_eq!(frame.tab_bars.len(), 1);
    assert_eq!(frame.tab_bars[0].viewport, frame.tab_bars[0].rect);
    assert_eq!(frame.tabs.len(), 2);
    assert_eq!(frame.panes.len(), 1);
    assert_eq!(frame.panes[0].pane, PaneId(1));
    assert_eq!(frame.panes[0].rect.y0, 20.0);
}

fn resolved_tabs(group: TileId) -> TabGroupGeometry {
    TabGroupGeometry {
        group,
        viewport: Rect::new(0.0, 0.0, 300.0, 20.0),
        tabs: vec![
            TabGeometry {
                pane: PaneId(1),
                rect: Rect::new(0.0, 0.0, 200.0, 20.0),
                interaction_regions: vec![Rect::new(0.0, 0.0, 180.0, 20.0)],
            },
            TabGeometry {
                pane: PaneId(2),
                rect: Rect::new(200.0, 0.0, 300.0, 20.0),
                interaction_regions: vec![Rect::new(200.0, 0.0, 280.0, 20.0)],
            },
        ],
        bar_drag_regions: vec![Rect::new(280.0, 0.0, 300.0, 20.0)],
    }
}

#[test]
fn resolved_tab_geometry_replaces_tabs_and_tab_hits_only() {
    let tree = tab_tree([PaneId(1), PaneId(2)]);
    let group = tree.root();
    let revision = tree.revision();
    let mut frame = tree.layout(input());
    let pane_hit = hit_test(&frame, Point::new(10.0, 30.0));

    frame
        .apply_tab_group_geometry(&[resolved_tabs(group)])
        .unwrap();

    assert_eq!(frame.tabs[0].rect, Rect::new(0.0, 0.0, 200.0, 20.0));
    assert_eq!(frame.tabs[1].rect, Rect::new(200.0, 0.0, 300.0, 20.0));
    assert_eq!(frame.tab_bars[0].viewport, resolved_tabs(group).viewport);
    assert_eq!(
        hit_test(&frame, Point::new(170.0, 10.0)),
        Some(HitKind::Tab {
            group,
            pane: PaneId(1)
        })
    );
    assert_eq!(hit_test(&frame, Point::new(190.0, 10.0)), None);
    assert_eq!(
        hit_test(&frame, Point::new(290.0, 10.0)),
        Some(HitKind::TabBar { group })
    );
    assert_eq!(hit_test(&frame, Point::new(10.0, 30.0)), pane_hit);
    assert_eq!(frame.revision, revision);
    assert_eq!(tree.revision(), revision);
}

#[test]
fn resolved_tab_hit_precedence_uses_semantic_order() {
    let tree = tab_tree([PaneId(1), PaneId(2)]);
    let group = tree.root();
    let mut geometry = resolved_tabs(group);
    geometry.tabs[0].rect = Rect::new(0.0, 0.0, 200.0, 20.0);
    geometry.tabs[1].rect = Rect::new(100.0, 0.0, 300.0, 20.0);
    geometry.tabs[0].interaction_regions = vec![Rect::new(100.0, 0.0, 180.0, 20.0)];
    geometry.tabs[1].interaction_regions = vec![Rect::new(100.0, 0.0, 180.0, 20.0)];
    let mut reversed = geometry.clone();
    reversed.tabs.reverse();
    let mut semantic_frame = tree.layout(input());
    let mut reversed_frame = semantic_frame.clone();

    semantic_frame
        .apply_tab_group_geometry(&[geometry])
        .unwrap();
    reversed_frame
        .apply_tab_group_geometry(&[reversed])
        .unwrap();

    let point = Point::new(120.0, 10.0);
    assert_eq!(
        hit_test(&semantic_frame, point),
        hit_test(&reversed_frame, point)
    );
    assert_eq!(
        hit_test(&reversed_frame, point),
        Some(HitKind::Tab {
            group,
            pane: PaneId(1)
        })
    );
}

#[test]
fn resolved_scrolled_tab_only_hits_its_visible_regions() {
    let tree = tab_tree([PaneId(1), PaneId(2)]);
    let group = tree.root();
    let mut frame = tree.layout(input());
    let geometry = TabGroupGeometry {
        group,
        viewport: Rect::new(20.0, 0.0, 280.0, 20.0),
        tabs: vec![
            TabGeometry {
                pane: PaneId(1),
                rect: Rect::new(-40.0, 0.0, 80.0, 20.0),
                interaction_regions: vec![Rect::new(20.0, 0.0, 80.0, 20.0)],
            },
            TabGeometry {
                pane: PaneId(2),
                rect: Rect::new(80.0, 0.0, 200.0, 20.0),
                interaction_regions: vec![Rect::new(80.0, 0.0, 200.0, 20.0)],
            },
        ],
        bar_drag_regions: vec![],
    };

    frame.apply_tab_group_geometry(&[geometry]).unwrap();

    assert_eq!(hit_test(&frame, Point::new(10.0, 10.0)), None);
    assert!(matches!(
        hit_test(&frame, Point::new(30.0, 10.0)),
        Some(HitKind::Tab {
            pane: PaneId(1),
            ..
        })
    ));

    let options = interaction_options_with_drag(DragOptions {
        allow_split: false,
        ..DragOptions::default()
    });
    let update = pointer_update(
        &tree,
        &frame,
        Point::new(100.0, 10.0),
        Point::new(10.0, 10.0),
        &options,
    );
    assert!(dock_proposal(&update).is_none());
}

#[test]
fn invalid_resolved_tab_geometry_is_atomic() {
    let tree = tab_tree([PaneId(1), PaneId(2)]);
    let group = tree.root();
    let mut frame = tree.layout(input());
    let original = frame.clone();
    let mut geometry = resolved_tabs(group);
    geometry.tabs[1].pane = PaneId(1);

    assert_eq!(
        frame.apply_tab_group_geometry(&[geometry]),
        Err(TabGeometryError::DuplicatePane {
            group,
            pane: PaneId(1)
        })
    );
    assert_eq!(frame.tabs, original.tabs);
    assert_eq!(frame.hit_regions, original.hit_regions);

    let mut geometry = resolved_tabs(group);
    geometry.tabs.pop();
    assert_eq!(
        frame.apply_tab_group_geometry(&[geometry]),
        Err(TabGeometryError::MissingPane {
            group,
            pane: PaneId(2)
        })
    );

    let mut geometry = resolved_tabs(group);
    geometry.tabs[1].pane = PaneId(99);
    assert_eq!(
        frame.apply_tab_group_geometry(&[geometry]),
        Err(TabGeometryError::UnknownPane {
            group,
            pane: PaneId(99)
        })
    );
}

fn two_tab_groups() -> TileTree {
    TileTree::from_raw_parts_for_test(
        TileId(0),
        Revision(0),
        vec![
            Some(TileNode::Split(SplitNode {
                axis: Axis::Horizontal,
                children: vec![TileId(1), TileId(2)],
                shares: vec![1.0, 1.0],
                constraints: SplitConstraints::default(),
            })),
            Some(TileNode::Tabs(TabNode {
                panes: vec![PaneId(1), PaneId(2)],
                active: 0,
                placement: TabBarPlacement::Top,
            })),
            Some(TileNode::Tabs(TabNode {
                panes: vec![PaneId(3), PaneId(4)],
                active: 0,
                placement: TabBarPlacement::Top,
            })),
        ],
    )
}

fn geometry_for_bar(frame: &LayoutFrame, group: TileId) -> TabGroupGeometry {
    let bar = frame
        .tab_bars
        .iter()
        .find(|bar| bar.group == group)
        .unwrap();
    let tabs: Vec<_> = frame
        .tabs
        .iter()
        .filter(|tab| tab.group == group)
        .map(|tab| TabGeometry {
            pane: tab.pane,
            rect: tab.rect,
            interaction_regions: vec![tab.rect],
        })
        .collect();
    TabGroupGeometry {
        group,
        viewport: bar.rect,
        tabs,
        bar_drag_regions: vec![],
    }
}

#[test]
fn resolved_tab_geometry_validation_is_atomic_across_groups() {
    let tree = two_tab_groups();
    let mut frame = tree.layout(input());
    let original = frame.clone();
    let first = geometry_for_bar(&frame, TileId(1));
    let mut second = geometry_for_bar(&frame, TileId(2));
    second.tabs.pop();

    assert_eq!(
        frame.apply_tab_group_geometry(&[first, second]),
        Err(TabGeometryError::MissingPane {
            group: TileId(2),
            pane: PaneId(4),
        })
    );
    assert_eq!(frame.tabs, original.tabs);
    assert_eq!(frame.tab_bars, original.tab_bars);
    assert_eq!(frame.hit_regions, original.hit_regions);
}

#[test]
fn resolved_tab_geometry_reapplication_removes_old_hits() {
    let tree = tab_tree([PaneId(1), PaneId(2)]);
    let group = tree.root();
    let mut frame = tree.layout(input());
    frame
        .apply_tab_group_geometry(&[resolved_tabs(group)])
        .unwrap();
    let mut shifted = resolved_tabs(group);
    shifted.tabs[0].rect = Rect::new(20.0, 0.0, 220.0, 20.0);
    shifted.tabs[0].interaction_regions = vec![Rect::new(20.0, 0.0, 180.0, 20.0)];

    frame.apply_tab_group_geometry(&[shifted]).unwrap();

    assert_eq!(hit_test(&frame, Point::new(10.0, 10.0)), None);
    assert!(matches!(
        hit_test(&frame, Point::new(30.0, 10.0)),
        Some(HitKind::Tab {
            pane: PaneId(1),
            ..
        })
    ));
}

#[test]
fn resolved_tab_geometry_reports_group_and_region_errors() {
    let tree = tab_tree([PaneId(1), PaneId(2)]);
    let group = tree.root();
    let frame = tree.layout(input());

    assert_eq!(
        frame
            .clone()
            .apply_tab_group_geometry(&[resolved_tabs(TileId(99))]),
        Err(TabGeometryError::UnknownGroup(TileId(99)))
    );
    assert_eq!(
        frame
            .clone()
            .apply_tab_group_geometry(&[resolved_tabs(group), resolved_tabs(group)]),
        Err(TabGeometryError::DuplicateGroup(group))
    );

    let mut invalid_viewport = resolved_tabs(group);
    invalid_viewport.viewport = Rect::new(-1.0, 0.0, 300.0, 20.0);
    assert_eq!(
        frame.clone().apply_tab_group_geometry(&[invalid_viewport]),
        Err(TabGeometryError::InvalidRect { group, pane: None })
    );

    let mut invalid_bar_region = resolved_tabs(group);
    invalid_bar_region.bar_drag_regions = vec![Rect::new(290.0, 0.0, 301.0, 20.0)];
    assert_eq!(
        frame
            .clone()
            .apply_tab_group_geometry(&[invalid_bar_region]),
        Err(TabGeometryError::InvalidInteractionRegion { group, pane: None })
    );
}

#[test]
fn tab_geometry_errors_have_human_readable_diagnostics() {
    assert_eq!(
        TabGeometryError::MissingPane {
            group: TileId(2),
            pane: PaneId(7),
        }
        .to_string(),
        "tab group tile 2 is missing pane 7"
    );
    assert_eq!(
        TabGeometryError::InvalidInteractionRegion {
            group: TileId(2),
            pane: None,
        }
        .to_string(),
        "tab group tile 2 has an invalid bar interaction region"
    );
}

#[test]
fn frame_diff_observes_resolved_tab_geometry() {
    let tree = tab_tree([PaneId(1), PaneId(2)]);
    let group = tree.root();
    let before = tree.layout(input());
    let mut after = before.clone();
    after
        .apply_tab_group_geometry(&[resolved_tabs(group)])
        .unwrap();

    assert_eq!(
        diff_item(
            &diff_frames(&before, &after),
            FrameItemId::Tab {
                group,
                pane: PaneId(1)
            }
        )
        .change,
        FrameChange::Resized
    );
}

#[test]
fn reorder_insertion_uses_resolved_widths() {
    let tree = tab_tree([PaneId(1), PaneId(2)]);
    let group = tree.root();
    let mut frame = tree.layout(input());
    frame
        .apply_tab_group_geometry(&[resolved_tabs(group)])
        .unwrap();
    let options = interaction_options_with_drag(DragOptions {
        allow_split: false,
        ..DragOptions::default()
    });

    let update = pointer_update(
        &tree,
        &frame,
        Point::new(250.0, 10.0),
        Point::new(90.0, 10.0),
        &options,
    );
    assert!(matches!(
        dock_proposal(&update),
        Some(DockProposal::ReorderTab { index: 0, .. })
    ));
}

#[test]
fn vertical_reorder_insertion_uses_bar_placement() {
    let tree = TileTree::from_spec(TileSpec::tabs_with(
        [PaneId(1), PaneId(2)],
        0,
        TabBarPlacement::Left,
    ))
    .unwrap();
    let group = tree.root();
    let mut frame = tree.layout(input());
    frame
        .apply_tab_group_geometry(&[TabGroupGeometry {
            group,
            viewport: Rect::new(0.0, 0.0, 20.0, 200.0),
            tabs: vec![
                TabGeometry {
                    pane: PaneId(1),
                    rect: Rect::new(0.0, 0.0, 20.0, 140.0),
                    interaction_regions: vec![Rect::new(0.0, 0.0, 20.0, 140.0)],
                },
                TabGeometry {
                    pane: PaneId(2),
                    rect: Rect::new(0.0, 140.0, 20.0, 200.0),
                    interaction_regions: vec![Rect::new(0.0, 140.0, 20.0, 200.0)],
                },
            ],
            bar_drag_regions: vec![],
        }])
        .unwrap();
    let options = interaction_options_with_drag(DragOptions {
        allow_split: false,
        ..DragOptions::default()
    });

    let update = pointer_update(
        &tree,
        &frame,
        Point::new(10.0, 170.0),
        Point::new(10.0, 60.0),
        &options,
    );
    assert!(matches!(
        dock_proposal(&update),
        Some(DockProposal::ReorderTab { index: 0, .. })
    ));
}

#[test]
fn activate_pane_changes_active_tab() {
    let mut tree = tab_tree([PaneId(1), PaneId(2)]);
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
    let mut tree = tab_tree([PaneId(1), PaneId(2), PaneId(3)]);
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
    let mut tree = tab_tree([PaneId(1)]);
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
fn set_split_shares_rejects_bad_input() {
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
    let revision = tree.revision();
    assert!(matches!(
        tree.apply(TileOp::SetSplitShares {
            split,
            shares: vec![f64::NAN],
        }),
        Err(TileError::InvalidOperation)
    ));
    let Some(TileNode::Split(node)) = tree.node(split) else {
        panic!("root should be split");
    };
    assert_eq!(node.shares, vec![0.5, 0.5]);
    assert_eq!(tree.revision(), revision);
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
fn split_pane_rejects_duplicate_pane_atomically() {
    let mut tree = tab_tree([PaneId(1), PaneId(2)]);
    let revision = tree.revision();

    assert_eq!(
        tree.apply(TileOp::SplitPane {
            pane: PaneId(1),
            axis: Axis::Horizontal,
            new_pane: PaneId(2),
            placement: Placement::After,
            share: 0.5,
        }),
        Err(TileError::DuplicatePane(PaneId(2)))
    );
    assert_eq!(tree.revision(), revision);
    assert_eq!(tree.panes().collect::<Vec<_>>(), vec![PaneId(1), PaneId(2)]);
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
    let update = pointer_update(
        &tree,
        &frame,
        Point::new(20.0, 20.0),
        Point::new(290.0, 50.0),
        &interaction_options(),
    );
    assert!(matches!(
        dock_proposal(&update),
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
    let options = interaction_options_with_drag(DragOptions::default());

    let update = pointer_update(
        &tree,
        &frame,
        Point::new(20.0, 20.0),
        Point::new(290.0, 50.0),
        &options,
    );

    let preview = update.preview.as_ref().unwrap();
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
    let options = interaction_options_with_drag(DragOptions::default());
    let update = pointer_update(
        &tree,
        &frame,
        Point::new(20.0, 20.0),
        Point::new(290.0, 50.0),
        &options,
    );
    let preview = update.preview.as_ref().unwrap();

    let policy = DockPolicy::default();
    let validated = validate_interaction_update(&tree, &frame, &update, &policy, &options).unwrap();
    commit_proposal(&mut tree, validated).unwrap();
    let committed = tree.layout(input());

    assert!(diff_frames(preview, &committed).items.is_empty());
    assert!(
        diff_frames(&frame, preview)
            .items
            .iter()
            .any(|item| matches!(
                (item.motion, item.cause),
                (FrameMotion::Enter { .. }, FrameCause::Relayout)
                    | (FrameMotion::Stable, FrameCause::Relayout)
            ))
    );
}

#[test]
fn pending_drag_waits_until_threshold_is_crossed() {
    let tree = TileTree::single_pane(PaneId(1));
    let frame = tree.layout(input());
    let options = InteractionOptions {
        drag_threshold: 5.0,
        ..InteractionOptions::from_layout_input(input())
    };
    let InteractionState::PendingDrag(pending) =
        begin_interaction(&frame, Point::new(20.0, 20.0), &options)
    else {
        panic!("pane hit should start a pending drag");
    };

    assert!(pending.update(Point::new(23.0, 23.0)).is_none());

    let drag = pending.update(Point::new(26.0, 20.0)).unwrap();
    assert_eq!(drag.origin, Point::new(20.0, 20.0));
    assert_eq!(drag.current, Point::new(26.0, 20.0));
    assert_eq!(drag.subject, DragSubject::Pane(PaneId(1)));
}

#[test]
fn begin_interaction_starts_resize_before_drag() {
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

    let state = begin_interaction(&frame, Point::new(150.0, 100.0), &interaction_options());

    assert!(matches!(state, InteractionState::Resize(_)));
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
    let options = InteractionOptions {
        drag_threshold: 5.0,
        ..InteractionOptions::from_layout_input(input())
    };
    let mut state = begin_interaction(&frame, Point::new(20.0, 20.0), &options);

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
    let preview_layout = options.preview_layout.unwrap();

    assert_eq!(preview_layout.bounds, layout.bounds);
    assert_eq!(
        preview_layout.split_handle_thickness,
        layout.split_handle_thickness
    );
    assert_eq!(options.resize.min_pane_size, layout.min_pane_size);
    assert_eq!(options.drag_intent, DragIntent::Move);
    assert_eq!(options.drag_threshold, 5.0);
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

    let update = pointer_update(
        &tree,
        &frame,
        Point::new(20.0, 100.0),
        Point::new(200.0, 5.0),
        &interaction_options(),
    );

    assert!(matches!(
        dock_proposal(&update),
        Some(DockProposal::MovePane {
            target: DockTarget::Split {
                tile,
                axis: Axis::Vertical,
                placement: Placement::Before,
                ..
            },
            ..
        }) if *tile == target_tile
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
    let options = interaction_options_with_drag(DragOptions {
        edge_zone_fraction: 0.5,
        ..DragOptions::default()
    });
    let cases = [
        (Point::new(228.0, 20.0), Axis::Vertical, Placement::Before),
        (Point::new(228.0, 180.0), Axis::Vertical, Placement::After),
        (
            Point::new(184.0, 100.0),
            Axis::Horizontal,
            Placement::Before,
        ),
        (Point::new(271.0, 100.0), Axis::Horizontal, Placement::After),
    ];

    for (point, expected_axis, expected_placement) in cases {
        let update = pointer_update(&tree, &frame, Point::new(20.0, 100.0), point, &options);

        assert!(matches!(
            dock_proposal(&update),
            Some(DockProposal::MovePane {
                target: DockTarget::Split {
                    tile,
                    axis,
                    placement,
                    ..
                },
                ..
            }) if *tile == target_tile && *axis == expected_axis && *placement == expected_placement
        ));
    }
}

#[test]
fn dragging_pane_to_its_own_edge_is_invalid() {
    let tree = TileTree::single_pane(PaneId(1));
    let frame = tree.layout(input());

    let update = pointer_update(
        &tree,
        &frame,
        Point::new(20.0, 20.0),
        Point::new(5.0, 100.0),
        &interaction_options(),
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
    let tree = tab_tree([PaneId(1), PaneId(2)]);
    let frame = tree.layout(input());

    let update = pointer_update(
        &tree,
        &frame,
        Point::new(20.0, 50.0),
        Point::new(5.0, 100.0),
        &interaction_options(),
    );

    assert!(matches!(
        dock_proposal(&update),
        Some(DockProposal::MovePane {
            pane: PaneId(1),
            target: DockTarget::Split {
                tile,
                axis: Axis::Horizontal,
                placement: Placement::Before,
                ..
            },
        }) if *tile == tree.root()
    ));
}

#[test]
fn drag_over_tab_group_body_proposes_tab_into_group() {
    let mut tree = tab_tree([PaneId(1), PaneId(2)]);
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

    let update = pointer_update(
        &tree,
        &frame,
        Point::new(250.0, 100.0),
        Point::new(80.0, 100.0),
        &interaction_options(),
    );

    assert!(matches!(
        dock_proposal(&update),
        Some(DockProposal::MovePane {
            pane: PaneId(3),
            target: DockTarget::TabInto { group: target, index: None },
        }) if *target == group
    ));
}

#[test]
fn tab_group_drag_validates_and_commits_split_move() {
    let mut tree = tab_tree([PaneId(1), PaneId(2)]);
    let group = tree.root();
    tree.apply(TileOp::ActivatePane { pane: PaneId(2) })
        .unwrap();
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(3),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let frame = tree.layout(input());
    let target_tile = frame
        .panes
        .iter()
        .find(|pane| pane.pane == PaneId(3))
        .unwrap()
        .tile;
    let drag = DragSession {
        subject: DragSubject::TabGroup(group),
        source: DragSource::TabBar { group },
        origin: Point::new(10.0, 10.0),
        current: Point::new(10.0, 10.0),
        base_revision: frame.revision,
        proposal: None,
    };
    let mut state = InteractionState::Drag(drag);

    let update = update_interaction(
        &tree,
        &frame,
        &mut state,
        Point::new(200.0, 5.0),
        &interaction_options(),
    );

    assert!(matches!(
        dock_proposal(&update),
        Some(DockProposal::MoveTabGroup {
            group: moved,
            target: DockTarget::Split {
                tile,
                axis: Axis::Vertical,
                placement: Placement::Before,
                ..
            },
        }) if *moved == group && *tile == target_tile
    ));
    assert_eq!(
        update.overlay.ghost_rects,
        vec![GhostFrame {
            rect: Rect::new(155.0, 0.0, 300.0, 100.0),
            kind: GhostKind::PreviewGroup,
        }]
    );

    let policy = DockPolicy::default();
    let proposal =
        validate_interaction_update(&tree, &frame, &update, &policy, &interaction_options())
            .unwrap();
    assert!(matches!(proposal.operation(), TileOp::MoveTabGroup { .. }));
    commit_proposal(&mut tree, proposal).unwrap();

    let Some(TileNode::Tabs(tabs)) = tree.node(group) else {
        panic!("moved group should remain a tab group");
    };
    assert_eq!(tabs.panes, vec![PaneId(1), PaneId(2)]);
    assert_eq!(tabs.active, 1);
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
    let mut state = begin_interaction(&frame, Point::new(150.0, 100.0), &interaction_options());

    let update = update_interaction(
        &tree,
        &frame,
        &mut state,
        Point::new(200.0, 100.0),
        &interaction_options(),
    );

    let proposal = resize_proposal(&update).unwrap();
    assert_eq!(proposal.delta, 50.0);
    assert_eq!(proposal.new_major_lengths, vec![195.0, 95.0]);

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
    let mut state = begin_interaction(&frame, Point::new(150.0, 100.0), &interaction_options());

    let update = update_interaction(
        &tree,
        &frame,
        &mut state,
        Point::new(500.0, 100.0),
        &interaction_options(),
    );

    let proposal = resize_proposal(&update).unwrap();
    assert_eq!(proposal.delta, 125.0);
    assert_eq!(proposal.new_major_lengths, vec![270.0, 20.0]);

    let preview = update.preview.unwrap();
    assert_eq!(preview.panes[0].rect.width(), 270.0);
    assert_eq!(preview.panes[1].rect.width(), 20.0);
}

#[test]
fn resize_preview_uses_provided_layout_input() {
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
    let preview_input = LayoutInput {
        bounds: Rect::new(0.0, 0.0, 600.0, 200.0),
        ..input()
    };
    let options = InteractionOptions {
        resize: ResizeOptions {
            min_pane_size: input().min_pane_size,
        },
        preview_layout: Some(preview_input),
        ..interaction_options()
    };
    let mut state = begin_interaction(&frame, Point::new(150.0, 100.0), &options);

    let update = update_interaction(
        &tree,
        &frame,
        &mut state,
        Point::new(200.0, 100.0),
        &options,
    );

    let preview = update.preview.unwrap();
    assert_eq!(preview.panes[1].rect.x1, 600.0);
    assert!(preview.panes[0].rect.width() > 300.0);
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
            new_major_lengths: vec![195.0, 95.0],
        }),
        &DockPolicy::default(),
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
                new_major_lengths: vec![280.0, 10.0],
            }),
            &DockPolicy::default(),
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
    let mut state = begin_interaction(&frame, Point::new(150.0, 100.0), &interaction_options());
    let update = update_interaction(
        &tree,
        &frame,
        &mut state,
        Point::new(200.0, 100.0),
        &interaction_options(),
    );

    let validated = validate_proposal(
        ProposalValidationInput::new(
            &tree,
            update.proposal.clone().unwrap(),
            &DockPolicy::default(),
        )
        .with_frame(&frame),
    )
    .unwrap();

    assert!(matches!(
        validated.operation(),
        TileOp::SetSplitShares { .. }
    ));
}

#[test]
fn tab_insert_threshold_controls_reorder_index() {
    let tree = tab_tree([PaneId(1), PaneId(2)]);
    let frame = tree.layout(input());
    let options = interaction_options_with_drag(DragOptions {
        allow_split: false,
        tab_insert_threshold: 0.75,
        ..DragOptions::default()
    });

    let update = pointer_update(
        &tree,
        &frame,
        Point::new(225.0, 10.0),
        Point::new(100.0, 10.0),
        &options,
    );
    assert!(matches!(
        dock_proposal(&update),
        Some(DockProposal::ReorderTab { index: 0, .. })
    ));

    let options = interaction_options_with_drag(DragOptions {
        allow_split: false,
        tab_insert_threshold: 0.25,
        ..DragOptions::default()
    });
    let update = pointer_update(
        &tree,
        &frame,
        Point::new(225.0, 10.0),
        Point::new(100.0, 10.0),
        &options,
    );
    assert!(matches!(
        dock_proposal(&update),
        Some(DockProposal::ReorderTab { index: 1, .. })
    ));
}

#[test]
fn stale_interaction_validation_is_rejected() {
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
    let options = interaction_options();
    let update = pointer_update(
        &tree,
        &frame,
        Point::new(20.0, 20.0),
        Point::new(290.0, 50.0),
        &options,
    );
    assert!(update.proposal.is_some());
    tree.apply(TileOp::SplitPane {
        pane: PaneId(2),
        axis: Axis::Horizontal,
        new_pane: PaneId(3),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    assert!(matches!(
        validate_interaction_update(&tree, &frame, &update, &DockPolicy::default(), &options),
        Err(TileError::StaleInteraction)
    ));
}

#[test]
fn stale_frame_cancels_interaction_before_generating_preview() {
    let mut tree = tab_tree([PaneId(1), PaneId(2)]);
    let frame = tree.layout(input());
    let mut state = begin_interaction(&frame, Point::new(20.0, 10.0), &interaction_options());
    tree.apply(TileOp::ActivatePane { pane: PaneId(2) })
        .unwrap();

    let update = update_interaction(
        &tree,
        &frame,
        &mut state,
        Point::new(100.0, 10.0),
        &interaction_options(),
    );

    assert!(matches!(state, InteractionState::None));
    assert!(update.proposal.is_none());
    assert!(update.preview.is_none());
    assert!(update.candidates.is_empty());
}

#[test]
fn exhausted_revision_rejects_mutation_atomically() {
    let mut tree = TileTree::from_raw_parts_for_test(
        TileId(0),
        Revision(u64::MAX),
        vec![Some(TileNode::tabs(vec![PaneId(1), PaneId(2)]))],
    );

    assert!(matches!(
        tree.apply(TileOp::ActivatePane { pane: PaneId(2) }),
        Err(TileError::RevisionExhausted)
    ));
    assert_eq!(tree.revision(), Revision(u64::MAX));
    let Some(TileNode::Tabs(tabs)) = tree.node(tree.root()) else {
        panic!("root should remain tabs");
    };
    assert_eq!(tabs.active, 0);
}

#[test]
fn validate_proposal_rejects_locked_layout() {
    let tree = tab_tree([PaneId(1), PaneId(2)]);
    let group = tree.root();
    let policy = DockPolicy {
        locked_layout: true,
        ..DockPolicy::default()
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
        &DockPolicy::default(),
    ));

    assert_eq!(result.unwrap_err(), TileError::InvalidTileId);
}

#[test]
fn validate_proposal_rejects_disallowed_tab_zone() {
    let mut tree = tab_tree([PaneId(1)]);
    let group = tree.root();
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(2),
        placement: Placement::After,
        share: 0.5,
    })
    .unwrap();
    let policy = DockPolicy {
        permissions: DockPermissions {
            allowed_zones: ZoneSet::SPLIT,
            ..DockPermissions::default()
        },
        ..DockPolicy::default()
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
    let tree = tab_tree([PaneId(1), PaneId(2)]);
    let group = tree.root();
    let policy = DockPolicy {
        permissions: DockPermissions {
            allowed_edges: EdgeSet::RIGHT,
            ..DockPermissions::default()
        },
        ..DockPolicy::default()
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
    let mut tree = tab_tree([PaneId(1), PaneId(2)]);
    let group = tree.root();

    let proposal = validate_proposal(ProposalValidationInput::new(
        &tree,
        Proposal::Dock(DockProposal::ReorderTab {
            group,
            pane: PaneId(2),
            index: 0,
        }),
        &DockPolicy::default(),
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
fn commit_proposal_rejects_tree_changed_after_validation() {
    let mut tree = tab_tree([PaneId(1), PaneId(2)]);
    let group = tree.root();
    let proposal = validate_proposal(ProposalValidationInput::new(
        &tree,
        Proposal::Dock(DockProposal::ReorderTab {
            group,
            pane: PaneId(2),
            index: 0,
        }),
        &DockPolicy::default(),
    ))
    .unwrap();
    tree.apply(TileOp::ActivatePane { pane: PaneId(2) })
        .unwrap();
    let panes_before = tree.panes().collect::<Vec<_>>();

    assert!(matches!(
        commit_proposal(&mut tree, proposal),
        Err(TileError::StaleInteraction)
    ));
    assert_eq!(tree.panes().collect::<Vec<_>>(), panes_before);
    let Some(TileNode::Tabs(tabs)) = tree.node(group) else {
        panic!("root should remain tabs");
    };
    assert_eq!(tabs.panes, vec![PaneId(1), PaneId(2)]);
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
    let options = InteractionOptions::from_layout_input(input());
    let mut state = begin_interaction(&frame, Point::new(150.0, 100.0), &options);

    let update = update_interaction(
        &tree,
        &frame,
        &mut state,
        Point::new(200.0, 100.0),
        &options,
    );
    assert!(matches!(update.proposal, Some(Proposal::Resize(_))));
    assert!(update.preview.is_some());

    let policy = DockPolicy::default();
    let proposal = validate_interaction_update(&tree, &frame, &update, &policy, &options).unwrap();
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
            &DockPolicy::default(),
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
    let actions = &report;

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
    let actions = &report;

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
fn restore_snapshot_always_repairs_before_returning() {
    let snapshot = LayoutSnapshot::new(TileTree::from_raw_parts_for_test(
        TileId(0),
        Revision(0),
        vec![Some(TileNode::Tabs(TabNode {
            panes: vec![PaneId(1), PaneId(1)],
            active: 99,
            placement: TabBarPlacement::Top,
        }))],
    ));

    let restored = restore_snapshot(snapshot).unwrap();
    let Some(TileNode::Tabs(tabs)) = restored.tree.node(TileId(0)) else {
        panic!("root should remain tabs");
    };
    assert_eq!(tabs.active, 0);
    assert_eq!(tabs.panes, vec![PaneId(1)]);
    assert!(
        restored
            .repairs
            .contains(&RepairAction::RepairedActiveTab(TileId(0)))
    );
    assert!(
        restored
            .repairs
            .contains(&RepairAction::RemovedDuplicatePane {
                pane: PaneId(1),
                tile: TileId(0),
            })
    );
}

#[test]
fn tree_restore_reports_repairs_and_advances_the_live_revision() {
    let snapshot = LayoutSnapshot::new(TileTree::from_raw_parts_for_test(
        TileId(0),
        Revision(99),
        vec![Some(TileNode::Tabs(TabNode {
            panes: vec![PaneId(1), PaneId(1)],
            active: 0,
            placement: TabBarPlacement::Top,
        }))],
    ));
    let mut tree = TileTree::single_pane(PaneId(2));

    let repairs = tree.restore(snapshot).unwrap();

    assert_eq!(tree.revision(), Revision(1));
    assert_eq!(tree.panes().collect::<Vec<_>>(), vec![PaneId(1)]);
    assert!(repairs.contains(&RepairAction::RemovedDuplicatePane {
        pane: PaneId(1),
        tile: TileId(0),
    }));
}

#[test]
fn restore_snapshot_bounds_cycles_and_rejects_empty_trees() {
    let cyclic = LayoutSnapshot::new(TileTree::from_raw_parts_for_test(
        TileId(0),
        Revision(0),
        vec![
            Some(TileNode::Split(SplitNode {
                axis: Axis::Horizontal,
                children: vec![TileId(0), TileId(1)],
                shares: vec![1.0, 1.0],
                constraints: SplitConstraints::default(),
            })),
            Some(TileNode::pane(PaneId(1))),
        ],
    ));
    let restored = restore_snapshot(cyclic).unwrap();
    assert_eq!(restored.tree.panes().collect::<Vec<_>>(), vec![PaneId(1)]);
    assert!(!restored.repairs.is_empty());

    let empty = LayoutSnapshot::new(TileTree::from_raw_parts_for_test(
        TileId(0),
        Revision(0),
        vec![Some(TileNode::tabs(vec![]))],
    ));
    assert!(matches!(
        restore_snapshot(empty),
        Err(TileError::InvalidTree)
    ));
}

#[test]
fn restore_snapshot_rejects_unknown_schema_version() {
    let snapshot = LayoutSnapshot::unsupported_for_test(99);

    assert!(matches!(
        restore_snapshot(snapshot),
        Err(TileError::UnsupportedSnapshotVersion(99))
    ));
}

#[cfg(feature = "serde")]
#[test]
fn snapshot_wire_rejects_version_before_decoding_tree_payload() {
    use serde::de::value::{Error, MapDeserializer};
    use serde::de::{Deserializer, IntoDeserializer, Visitor};

    struct WireValue;

    impl<'de> Deserializer<'de> for WireValue {
        type Error = Error;

        fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            visitor.visit_unit()
        }

        fn deserialize_enum<V>(
            self,
            _name: &'static str,
            _variants: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            visitor.visit_unit()
        }

        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
            bytes byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct
            map struct identifier ignored_any
        }
    }

    impl<'de> IntoDeserializer<'de, Error> for WireValue {
        type Deserializer = Self;

        fn into_deserializer(self) -> Self::Deserializer {
            self
        }
    }

    let wire = [("99", WireValue)];
    let deserializer = MapDeserializer::<_, Error>::new(wire.into_iter());
    let snapshot = <LayoutSnapshot as serde::Deserialize>::deserialize(deserializer).unwrap();

    assert!(matches!(
        restore_snapshot(snapshot),
        Err(TileError::UnsupportedSnapshotVersion(99))
    ));
}

#[cfg(feature = "serde")]
#[test]
fn snapshot_wire_rejects_multiple_version_entries() {
    use serde::de::value::{Error, MapDeserializer, UnitDeserializer};

    let wire = [
        ("99", UnitDeserializer::<Error>::new()),
        ("100", UnitDeserializer::<Error>::new()),
    ];
    let deserializer = MapDeserializer::<_, Error>::new(wire.into_iter());

    assert!(<LayoutSnapshot as serde::Deserialize>::deserialize(deserializer).is_err());
}

#[test]
fn normalization_preserves_active_pane_while_removing_duplicates() {
    let mut tree = TileTree::from_raw_parts_for_test(
        TileId(0),
        Revision(0),
        vec![Some(TileNode::Tabs(TabNode {
            panes: vec![PaneId(1), PaneId(1), PaneId(2)],
            active: 2,
            placement: TabBarPlacement::Top,
        }))],
    );

    let report = tree.repair();

    let Some(TileNode::Tabs(tabs)) = tree.node(TileId(0)) else {
        panic!("root should remain tabs");
    };
    assert_eq!(tabs.panes, vec![PaneId(1), PaneId(2)]);
    assert_eq!(tabs.active, 1);
    assert!(report.contains(&RepairAction::RemovedDuplicatePane {
        pane: PaneId(1),
        tile: TileId(0),
    }));
}

#[test]
fn split_constraints_follow_children_through_edits() {
    let mut tree = TileTree::from_spec(TileSpec::Split {
        axis: Axis::Horizontal,
        children: vec![TileSpec::pane(PaneId(1)), TileSpec::pane(PaneId(2))],
        shares: vec![1.0, 1.0],
        constraints: SplitConstraints {
            min_major: vec![100.0, 200.0],
        },
    })
    .unwrap();
    tree.apply(TileOp::SplitPane {
        pane: PaneId(1),
        axis: Axis::Horizontal,
        new_pane: PaneId(3),
        placement: Placement::Before,
        share: 0.5,
    })
    .unwrap();
    let Some(TileNode::Split(split)) = tree.node(tree.root()) else {
        panic!("root should remain a split");
    };
    assert_eq!(split.constraints.min_major, vec![0.0, 100.0, 200.0]);

    tree.apply(TileOp::ClosePane { pane: PaneId(1) }).unwrap();
    let Some(TileNode::Split(split)) = tree.node(tree.root()) else {
        panic!("root should remain a split");
    };
    assert_eq!(split.constraints.min_major, vec![0.0, 200.0]);
}

#[test]
fn normalization_flattens_constraints_only_when_semantics_are_preserved() {
    let nested = TileSpec::Split {
        axis: Axis::Horizontal,
        children: vec![TileSpec::pane(PaneId(1)), TileSpec::pane(PaneId(2))],
        shares: vec![1.0, 1.0],
        constraints: SplitConstraints {
            min_major: vec![10.0, 20.0],
        },
    };
    let flattened = TileTree::from_spec(TileSpec::equal_split(
        Axis::Horizontal,
        [nested.clone(), TileSpec::pane(PaneId(3))],
    ))
    .unwrap();
    let Some(TileNode::Split(split)) = flattened.node(flattened.root()) else {
        panic!("root should be a split");
    };
    assert_eq!(split.children.len(), 3);
    assert_eq!(split.constraints.min_major, vec![10.0, 20.0]);

    let preserved = TileTree::from_spec(TileSpec::Split {
        axis: Axis::Horizontal,
        children: vec![nested, TileSpec::pane(PaneId(3))],
        shares: vec![1.0, 1.0],
        constraints: SplitConstraints {
            min_major: vec![300.0],
        },
    })
    .unwrap();
    let Some(TileNode::Split(split)) = preserved.node(preserved.root()) else {
        panic!("root should be a split");
    };
    assert_eq!(split.children.len(), 2);
    assert!(matches!(
        preserved.node(split.children[0]),
        Some(TileNode::Split(_))
    ));
}

#[cfg(feature = "serde")]
#[test]
fn serde_covers_public_data_types() {
    fn assert_serde<T>()
    where
        T: serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
    }

    fn assert_serialize<T: serde::Serialize>() {}

    assert_serde::<TileId>();
    assert_serde::<PaneId>();
    assert_serde::<Revision>();

    assert_serde::<Axis>();
    assert_serde::<Placement>();
    assert_serde::<TabBarPlacement>();
    assert_serde::<SplitConstraints>();
    assert_serde::<TileNode>();
    assert_serde::<SplitNode>();
    assert_serde::<TabNode>();
    assert_serde::<PaneNode>();
    assert_serde::<LayoutInput>();
    assert_serde::<TileTree>();
    assert_serde::<TileSpec>();
    assert_serde::<TileSpecError>();

    assert_serialize::<LayoutFrame>();
    assert_serialize::<InteractionUpdate>();
    assert_serialize::<ValidatedProposal>();

    assert_serde::<FrameDiff>();
    assert_serde::<FrameItemDiff>();
    assert_serde::<FrameChange>();
    assert_serde::<FrameMotion>();
    assert_serde::<FrameCause>();
    assert_serde::<ProjectionEvent>();
    assert_serde::<ProjectionKind>();
    assert_serde::<PaneFrame>();
    assert_serde::<FrameProjection>();
    assert_serde::<ProjectionHiddenItem>();
    assert_serde::<TabBarFrame>();
    assert_serde::<TabFrame>();
    assert_serde::<TabGroupGeometry>();
    assert_serde::<TabGeometry>();
    assert_serde::<TabGeometryError>();
    assert_serde::<SplitChildFrame>();
    assert_serde::<SplitHandleFrame>();
    assert_serde::<FrameItemId>();
    assert_serde::<HitRegion>();
    assert_serde::<HitKind>();

    assert_serde::<DockTarget>();
    assert_serde::<TileOp>();
    assert_serde::<TileError>();
    assert_serde::<LayoutSnapshot>();
    assert_serde::<RepairAction>();

    assert_serde::<DockPermissions>();
    assert_serde::<EdgeSet>();
    assert_serde::<ZoneSet>();
    assert_serde::<DockPolicy>();

    assert_serde::<DragIntent>();
    assert_serde::<InteractionState>();
    assert_serde::<DragSession>();
    assert_serde::<PendingDrag>();
    assert_serde::<DragSubject>();
    assert_serde::<DragSource>();
    assert_serde::<ResizeSession>();
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
    assert_serde::<ResizeOptions>();
    assert_serde::<DragOptions>();
    assert_serde::<InteractionOptions>();
}
