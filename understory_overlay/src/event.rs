// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::cmp::Reverse;

use kurbo::Point;

use crate::frame::{frame_z, ordered_entry_indexes};
use crate::grace::point_in_grace_shape;
use crate::{
    AnchorId, DismissRegion, DismissTriggers, OverlayFrame, OverlayId, OverlayOp, OverlayStack,
};

/// Overlay event key understood by [`resolve_event`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OverlayKey {
    /// Escape key.
    Escape,
}

/// Focus target used by focus-outside dismissal.
///
/// Hosts provide these in [`OverlayEvent::FocusChanged`] after their own focus
/// system resolves the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FocusTarget {
    /// Focus moved to an overlay.
    Overlay(OverlayId),
    /// Focus moved to an anchor.
    Anchor(AnchorId),
    /// Focus moved somewhere outside the overlay system.
    Other(u64),
}

/// Input event consumed by overlay event resolution.
///
/// Pass these to [`resolve_event`] to get proposed operations, or to
/// [`OverlayStack::handle_event`](crate::OverlayStack::handle_event) to apply
/// the operations immediately.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OverlayEvent {
    /// Pointer button went down at a scene-space point.
    PointerDown {
        /// Scene-space pointer point.
        point: Point,
    },
    /// Pointer moved to a scene-space point.
    PointerMove {
        /// Scene-space pointer point.
        point: Point,
    },
    /// Pointer button went up at a scene-space point.
    PointerUp {
        /// Scene-space pointer point.
        point: Point,
    },
    /// Focus moved to a new target.
    FocusChanged {
        /// New focus target.
        target: FocusTarget,
    },
    /// Key went down.
    KeyDown {
        /// Key that went down.
        key: OverlayKey,
    },
    /// An anchor blurred.
    AnchorBlur {
        /// Anchor that blurred.
        anchor: AnchorId,
    },
}

/// Result of resolving an [`OverlayEvent`].
///
/// You get this from [`resolve_event`] or
/// [`OverlayStack::handle_event`](crate::OverlayStack::handle_event).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OverlayEventResult {
    /// Operations proposed for the event.
    pub ops: Vec<OverlayOp>,
    /// Whether overlay handling consumed the event.
    pub consumed: bool,
}

/// Resolves one event against the current stack and frame.
///
/// The function is pure: it returns proposed operations and never mutates the
/// stack. Pointer points are expected to be finite; debug builds assert this
/// at entry.
#[must_use]
pub fn resolve_event(
    stack: &OverlayStack,
    frame: &OverlayFrame,
    event: OverlayEvent,
) -> OverlayEventResult {
    match event {
        OverlayEvent::PointerDown { point } => {
            debug_assert!(point.is_finite(), "pointer point must be finite");
            pointer_outside_result(stack, frame, point, DismissTriggers::POINTER_DOWN_OUTSIDE)
        }
        OverlayEvent::PointerUp { point } => {
            debug_assert!(point.is_finite(), "pointer point must be finite");
            pointer_outside_result(stack, frame, point, DismissTriggers::POINTER_UP_OUTSIDE)
        }
        OverlayEvent::PointerMove { point } => {
            debug_assert!(point.is_finite(), "pointer point must be finite");
            OverlayEventResult::default()
        }
        OverlayEvent::FocusChanged { target } => focus_changed_result(stack, frame, target),
        OverlayEvent::KeyDown {
            key: OverlayKey::Escape,
        } => escape_result(stack, frame),
        OverlayEvent::AnchorBlur { anchor } => anchor_blur_result(stack, frame, anchor),
    }
}

fn escape_result(stack: &OverlayStack, _frame: &OverlayFrame) -> OverlayEventResult {
    for entry_index in ordered_entry_indexes(stack).into_iter().rev() {
        let entry = &stack.entries()[entry_index];
        if entry.dismiss.escape_key {
            return close_result(entry.id);
        }
    }
    OverlayEventResult::default()
}

fn pointer_outside_result(
    stack: &OverlayStack,
    frame: &OverlayFrame,
    point: Point,
    trigger: DismissTriggers,
) -> OverlayEventResult {
    let mut outside = Vec::new();
    for region in &frame.dismiss_regions {
        if region.triggers.contains(trigger) && !point_inside_dismiss_region(point, region) {
            outside.push(region.overlay);
        }
    }
    let Some(overlay) = choose_close_target(stack, frame, outside) else {
        return OverlayEventResult::default();
    };
    close_result(overlay)
}

fn focus_changed_result(
    stack: &OverlayStack,
    frame: &OverlayFrame,
    target: FocusTarget,
) -> OverlayEventResult {
    let mut outside = Vec::new();
    for region in &frame.dismiss_regions {
        if region.triggers.contains(DismissTriggers::FOCUS_OUTSIDE)
            && !focus_inside_overlay(stack, region.overlay, target)
        {
            outside.push(region.overlay);
        }
    }
    let Some(overlay) = choose_close_target(stack, frame, outside) else {
        return OverlayEventResult::default();
    };
    close_result(overlay)
}

fn anchor_blur_result(
    stack: &OverlayStack,
    frame: &OverlayFrame,
    anchor: AnchorId,
) -> OverlayEventResult {
    let outside = stack
        .entries()
        .iter()
        .filter(|entry| entry.anchor == Some(anchor) && entry.dismiss.anchor_blur)
        .map(|entry| entry.id)
        .collect();
    let roots = collapse_descendant_targets(stack, outside);
    if roots.is_empty() {
        return OverlayEventResult::default();
    }
    let mut ops = roots
        .into_iter()
        .map(|overlay| OverlayOp::Close { overlay })
        .collect::<Vec<_>>();
    ops.sort_by_key(|op| Reverse(op_z(frame, op)));
    OverlayEventResult {
        ops,
        consumed: true,
    }
}

fn close_result(overlay: OverlayId) -> OverlayEventResult {
    OverlayEventResult {
        ops: alloc::vec![OverlayOp::Close { overlay }],
        consumed: true,
    }
}

fn point_inside_dismiss_region(point: Point, region: &DismissRegion) -> bool {
    region.inside.iter().any(|rect| rect.contains(point))
        || region
            .grace
            .iter()
            .copied()
            .any(|shape| point_in_grace_shape(point, shape))
}

fn focus_inside_overlay(stack: &OverlayStack, overlay: OverlayId, target: FocusTarget) -> bool {
    match target {
        FocusTarget::Overlay(target) => {
            target == overlay || stack.is_descendant_of(target, overlay)
        }
        FocusTarget::Anchor(anchor) => stack.entries().iter().any(|entry| {
            (entry.id == overlay || stack.is_descendant_of(entry.id, overlay))
                && entry.anchor == Some(anchor)
        }),
        FocusTarget::Other(_) => false,
    }
}

fn choose_close_target(
    stack: &OverlayStack,
    frame: &OverlayFrame,
    targets: Vec<OverlayId>,
) -> Option<OverlayId> {
    let roots = collapse_descendant_targets(stack, targets);
    roots
        .into_iter()
        .max_by_key(|overlay| frame_z(frame, *overlay).unwrap_or(i32::MIN))
}

fn collapse_descendant_targets(stack: &OverlayStack, targets: Vec<OverlayId>) -> Vec<OverlayId> {
    let mut roots = Vec::new();
    for target in targets.iter().copied() {
        if targets
            .iter()
            .any(|other| *other != target && stack.is_descendant_of(target, *other))
        {
            continue;
        }
        if !roots.contains(&target) {
            roots.push(target);
        }
    }
    roots
}

fn op_z(frame: &OverlayFrame, op: &OverlayOp) -> i32 {
    match op {
        OverlayOp::Close { overlay } => frame_z(frame, *overlay).unwrap_or(i32::MIN),
        OverlayOp::Open { .. }
        | OverlayOp::CloseSubtree { .. }
        | OverlayOp::CloseDescendants { .. }
        | OverlayOp::CloseLayer { .. }
        | OverlayOp::CloseAll
        | OverlayOp::BringToFront { .. }
        | OverlayOp::SetParent { .. } => i32::MIN,
    }
}

#[cfg(test)]
mod tests {
    use super::{FocusTarget, OverlayEvent, OverlayKey, resolve_event};
    use crate::{
        AnchorId, DismissPolicy, Modality, OverlayBehavior, OverlayEntry, OverlayFrameInput,
        OverlayGeometry, OverlayId, OverlayLayer, OverlayOp, OverlayStack, build_overlay_frame,
    };
    use kurbo::{Point, Rect};

    fn entry(id: u64, behavior: OverlayBehavior) -> OverlayEntry {
        OverlayEntry::new(OverlayId(id), OverlayLayer::Popover, behavior)
    }

    fn frame_for(stack: &OverlayStack) -> crate::OverlayFrame {
        let geometries = [
            OverlayGeometry::new(OverlayId(1), Rect::new(0.0, 0.0, 100.0, 100.0))
                .with_anchor_rect(Rect::new(0.0, -20.0, 100.0, 0.0)),
            OverlayGeometry::new(OverlayId(2), Rect::new(120.0, 0.0, 220.0, 100.0)),
        ];
        build_overlay_frame(
            stack,
            OverlayFrameInput::new(Rect::new(0.0, 0.0, 400.0, 300.0), &geometries)
                .with_pointer(Point::new(110.0, 40.0)),
        )
    }

    #[test]
    fn escape_closes_topmost_eligible_overlay() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open {
                entry: entry(1, OverlayBehavior::Popover),
            })
            .expect("popover should open");
        stack
            .apply(OverlayOp::Open {
                entry: entry(2, OverlayBehavior::Popover),
            })
            .expect("second popover should open");
        let frame = frame_for(&stack);

        let result = resolve_event(
            &stack,
            &frame,
            OverlayEvent::KeyDown {
                key: OverlayKey::Escape,
            },
        );

        assert_eq!(
            result.ops,
            alloc::vec![OverlayOp::Close {
                overlay: OverlayId(2)
            }],
            "escape should close topmost eligible overlay",
        );
        assert!(result.consumed, "escape should be consumed");
    }

    #[test]
    fn escape_closes_topmost_eligible_overlay_without_geometry() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open {
                entry: entry(1, OverlayBehavior::Popover),
            })
            .expect("popover should open");
        stack
            .apply(OverlayOp::Open {
                entry: entry(2, OverlayBehavior::Popover),
            })
            .expect("second popover should open");
        let frame = build_overlay_frame(
            &stack,
            OverlayFrameInput::new(Rect::new(0.0, 0.0, 400.0, 300.0), &[]),
        );

        let result = resolve_event(
            &stack,
            &frame,
            OverlayEvent::KeyDown {
                key: OverlayKey::Escape,
            },
        );

        assert_eq!(
            result.ops,
            alloc::vec![OverlayOp::Close {
                overlay: OverlayId(2)
            }],
            "escape should close the topmost stack entry even before layout",
        );
        assert!(result.consumed, "escape should be consumed");
    }

    #[test]
    fn pointer_down_outside_closes_ancestor_once() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open {
                entry: OverlayEntry::new(OverlayId(1), OverlayLayer::Menu, OverlayBehavior::Menu),
            })
            .expect("menu should open");
        stack
            .apply(OverlayOp::Open {
                entry: OverlayEntry::new(
                    OverlayId(2),
                    OverlayLayer::Menu,
                    OverlayBehavior::Submenu,
                )
                .with_parent(OverlayId(1)),
            })
            .expect("submenu should open");
        let frame = frame_for(&stack);

        let result = resolve_event(
            &stack,
            &frame,
            OverlayEvent::PointerDown {
                point: Point::new(350.0, 250.0),
            },
        );

        assert_eq!(
            result.ops,
            alloc::vec![OverlayOp::Close {
                overlay: OverlayId(1)
            }],
            "outside both parent and child should close ancestor once",
        );
    }

    #[test]
    fn pointer_inside_parent_closes_child_only() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open {
                entry: OverlayEntry::new(OverlayId(1), OverlayLayer::Menu, OverlayBehavior::Menu),
            })
            .expect("menu should open");
        stack
            .apply(OverlayOp::Open {
                entry: OverlayEntry::new(
                    OverlayId(2),
                    OverlayLayer::Menu,
                    OverlayBehavior::Submenu,
                )
                .with_parent(OverlayId(1)),
            })
            .expect("submenu should open");
        let frame = frame_for(&stack);

        let result = resolve_event(
            &stack,
            &frame,
            OverlayEvent::PointerDown {
                point: Point::new(20.0, 20.0),
            },
        );

        assert_eq!(
            result.ops,
            alloc::vec![OverlayOp::Close {
                overlay: OverlayId(2)
            }],
            "inside parent but outside child should close child",
        );
    }

    #[test]
    fn pointer_up_is_distinct_from_pointer_down_policy() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open {
                entry: entry(1, OverlayBehavior::Popover).with_dismiss(DismissPolicy {
                    pointer_down_outside: true,
                    pointer_up_outside: false,
                    ..DismissPolicy::default()
                }),
            })
            .expect("popover should open");
        let frame = frame_for(&stack);

        let result = resolve_event(
            &stack,
            &frame,
            OverlayEvent::PointerUp {
                point: Point::new(350.0, 250.0),
            },
        );

        assert!(
            result.ops.is_empty(),
            "pointer up should not use pointer down policy",
        );
    }

    #[test]
    fn focus_outside_closes_overlay() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open {
                entry: entry(1, OverlayBehavior::Popover),
            })
            .expect("popover should open");
        let frame = frame_for(&stack);

        let result = resolve_event(
            &stack,
            &frame,
            OverlayEvent::FocusChanged {
                target: FocusTarget::Other(99),
            },
        );

        assert_eq!(
            result.ops,
            alloc::vec![OverlayOp::Close {
                overlay: OverlayId(1)
            }],
            "focus outside should close popover",
        );
    }

    #[test]
    fn anchor_blur_closes_attached_overlay() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open {
                entry: entry(1, OverlayBehavior::Popover)
                    .with_anchor(AnchorId(10))
                    .with_modality(Modality::NonModal),
            })
            .expect("popover should open");
        let frame = frame_for(&stack);

        let result = resolve_event(
            &stack,
            &frame,
            OverlayEvent::AnchorBlur {
                anchor: AnchorId(10),
            },
        );

        assert_eq!(
            result.ops,
            alloc::vec![OverlayOp::Close {
                overlay: OverlayId(1)
            }],
            "anchor blur should close attached overlay",
        );
    }
}
