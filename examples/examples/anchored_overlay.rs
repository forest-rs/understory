// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Compose anchored geometry with overlay lifecycle state.
//!
//! This example keeps the boundary explicit:
//!
//! 1. `understory_anchor` resolves floating rectangles from scene geometry.
//! 2. The host wraps those rectangles as `OverlayGeometry`.
//! 3. `understory_overlay` derives hit regions, dismissal regions, focus
//!    scopes, underlays, and hover grace from the open overlay stack.
//!
//! Run:
//! - `cargo run -p understory_examples --example anchored_overlay`

use kurbo::{Insets, Point, Rect, Size};
use understory_anchor::{
    Anchor, AnchorConstraint, AnchorFrame, AnchorInput, AnchorPolicy, AnchorPositionOption,
    Placement, resolve_anchor,
};
use understory_overlay::{
    AnchorId, FocusTarget, OverlayBehavior, OverlayEntry, OverlayEvent, OverlayFrame,
    OverlayFrameInput, OverlayGeometry, OverlayId, OverlayKey, OverlayLayer, OverlayOp,
    OverlayStack, build_overlay_frame,
};

fn main() -> Result<(), understory_overlay::OverlayError> {
    let viewport = Rect::new(0.0, 0.0, 420.0, 280.0);
    let control = Rect::new(148.0, 226.0, 272.0, 258.0);

    let popover_constraints = [
        AnchorConstraint::Offset {
            main_axis: 10.0,
            cross_axis: 0.0,
        },
        AnchorConstraint::Arrow {
            size: Size::new(14.0, 7.0),
            padding: 10.0,
        },
    ];
    let popover_fallbacks = [
        AnchorPositionOption::new(Placement::TOP).with_constraints(&popover_constraints),
        AnchorPositionOption::new(Placement::RIGHT).with_constraints(&popover_constraints),
        AnchorPositionOption::new(Placement::LEFT).with_constraints(&popover_constraints),
    ];
    let popover_frame = resolve_anchor(
        AnchorInput {
            anchor: Anchor::Rect(control),
            floating_size: Size::new(220.0, 124.0),
            viewport,
            boundary: viewport,
            previous: None,
        },
        AnchorPolicy::new(
            AnchorPositionOption::new(Placement::BOTTOM).with_constraints(&popover_constraints),
            &popover_fallbacks,
        ),
    );

    let mut stack = OverlayStack::new();
    stack.apply(OverlayOp::Open {
        entry: OverlayEntry::new(
            OverlayId(1),
            OverlayLayer::Popover,
            OverlayBehavior::Popover,
        )
        .with_anchor(AnchorId(10)),
    })?;

    let popover_geometry = [OverlayGeometry::from_anchor_frame(
        OverlayId(1),
        &popover_frame,
    )];
    let popover_overlay_frame = build_overlay_frame(
        &stack,
        OverlayFrameInput::new(viewport, &popover_geometry)
            .with_pointer(Point::new(control.center().x, control.y0)),
    );

    println!("== anchored popover ==");
    println!("  control      {}", fmt_rect(control));
    print_anchor("popover", &popover_frame);
    print_overlay_frame(&popover_overlay_frame);

    let inside_result = understory_overlay::resolve_event(
        &stack,
        &popover_overlay_frame,
        OverlayEvent::PointerDown {
            point: control.center(),
        },
    );
    println!(
        "  pointer on anchor: consumed={} ops={:?}",
        inside_result.consumed, inside_result.ops
    );

    let outside_result = understory_overlay::resolve_event(
        &stack,
        &popover_overlay_frame,
        OverlayEvent::PointerDown {
            point: Point::new(24.0, 24.0),
        },
    );
    println!(
        "  pointer outside: consumed={} ops={:?}",
        outside_result.consumed, outside_result.ops
    );

    println!();
    println!("== context menu with submenu grace ==");
    let menu_anchor = Point::new(72.0, 72.0);
    let menu_frame = resolve_menu(menu_anchor, Size::new(156.0, 132.0), viewport);
    let submenu_anchor = Rect::new(
        menu_frame.rect.x1 - 4.0,
        menu_frame.rect.y0 + 44.0,
        menu_frame.rect.x1,
        menu_frame.rect.y0 + 72.0,
    );
    let submenu_frame = resolve_submenu(submenu_anchor, Size::new(168.0, 112.0), viewport);

    let mut menu_stack = OverlayStack::new();
    menu_stack.apply(OverlayOp::Open {
        entry: OverlayEntry::new(
            OverlayId(20),
            OverlayLayer::Menu,
            OverlayBehavior::ContextMenu,
        )
        .with_anchor(AnchorId(20)),
    })?;
    menu_stack.apply(OverlayOp::Open {
        entry: OverlayEntry::new(OverlayId(21), OverlayLayer::Menu, OverlayBehavior::Submenu)
            .with_parent(OverlayId(20))
            .with_anchor(AnchorId(21)),
    })?;

    let menu_geometries = [
        OverlayGeometry::from_anchor_frame(OverlayId(20), &menu_frame),
        OverlayGeometry::from_anchor_frame(OverlayId(21), &submenu_frame),
    ];
    let menu_overlay_frame = build_overlay_frame(
        &menu_stack,
        OverlayFrameInput::new(viewport, &menu_geometries).with_pointer(Point::new(
            menu_frame.rect.x1 - 12.0,
            submenu_anchor.center().y,
        )),
    );

    print_anchor("menu", &menu_frame);
    print_anchor("submenu", &submenu_frame);
    print_overlay_frame(&menu_overlay_frame);

    let grace_result = understory_overlay::resolve_event(
        &menu_stack,
        &menu_overlay_frame,
        OverlayEvent::PointerDown {
            point: Point::new(menu_frame.rect.x1 + 12.0, submenu_anchor.center().y),
        },
    );
    println!(
        "  pointer through grace: consumed={} ops={:?}",
        grace_result.consumed, grace_result.ops
    );

    let escape_result = menu_stack.clone().handle_event(
        &menu_overlay_frame,
        OverlayEvent::KeyDown {
            key: OverlayKey::Escape,
        },
    )?;
    println!(
        "  escape closes topmost: consumed={} ops={:?}",
        escape_result.consumed, escape_result.ops
    );

    let focus_result = understory_overlay::resolve_event(
        &menu_stack,
        &menu_overlay_frame,
        OverlayEvent::FocusChanged {
            target: FocusTarget::Other(99),
        },
    );
    println!(
        "  focus outside: consumed={} ops={:?}",
        focus_result.consumed, focus_result.ops
    );

    Ok(())
}

fn resolve_menu(anchor: Point, size: Size, viewport: Rect) -> AnchorFrame {
    let constraints = [
        AnchorConstraint::Offset {
            main_axis: 4.0,
            cross_axis: 0.0,
        },
        AnchorConstraint::Shift {
            padding: Insets::uniform(8.0),
        },
    ];
    let fallbacks = [
        AnchorPositionOption::new(Placement::TOP_START).with_constraints(&constraints),
        AnchorPositionOption::new(Placement::RIGHT_START).with_constraints(&constraints),
    ];
    resolve_anchor(
        AnchorInput {
            anchor: Anchor::Point(anchor),
            floating_size: size,
            viewport,
            boundary: viewport,
            previous: None,
        },
        AnchorPolicy::new(
            AnchorPositionOption::new(Placement::BOTTOM_START).with_constraints(&constraints),
            &fallbacks,
        ),
    )
}

fn resolve_submenu(anchor: Rect, size: Size, viewport: Rect) -> AnchorFrame {
    let constraints = [
        AnchorConstraint::Offset {
            main_axis: 2.0,
            cross_axis: -4.0,
        },
        AnchorConstraint::Shift {
            padding: Insets::uniform(8.0),
        },
    ];
    let fallbacks = [
        AnchorPositionOption::new(Placement::LEFT_START).with_constraints(&constraints),
        AnchorPositionOption::new(Placement::BOTTOM_START).with_constraints(&constraints),
    ];
    resolve_anchor(
        AnchorInput {
            anchor: Anchor::Rect(anchor),
            floating_size: size,
            viewport,
            boundary: viewport,
            previous: None,
        },
        AnchorPolicy::new(
            AnchorPositionOption::new(Placement::RIGHT_START).with_constraints(&constraints),
            &fallbacks,
        ),
    )
}

fn print_anchor(label: &str, frame: &AnchorFrame) {
    println!(
        "  {label:<8} placement={:?} rect={} reference={} visible={} clipped={} candidates={} hysteresis={}",
        frame.placement,
        fmt_rect(frame.rect),
        fmt_rect(frame.reference_rect),
        frame.visible,
        frame.clipped,
        frame.collision.candidates.len(),
        frame.collision.hysteresis_applied
    );
    if let Some(arrow) = frame.arrow {
        println!(
            "           arrow side={:?} rect={} tip=({:.0}, {:.0}) clamped={}",
            arrow.side,
            fmt_rect(arrow.rect),
            arrow.tip.x,
            arrow.tip.y,
            arrow.clamped
        );
    }
}

fn print_overlay_frame(frame: &OverlayFrame) {
    println!("  overlays:");
    for overlay in &frame.overlays {
        println!(
            "    {:?} layer={:?} parent={:?} z={} rect={}",
            overlay.id,
            overlay.layer,
            overlay.parent,
            overlay.z,
            fmt_rect(overlay.rect)
        );
    }
    println!(
        "  derived: hit_regions={} dismiss_regions={} grace_regions={} focus_scopes={}",
        frame.hit_regions.len(),
        frame.dismiss_regions.len(),
        frame.grace_regions.len(),
        frame.focus_scopes.len()
    );
    for scope in &frame.focus_scopes {
        println!(
            "    focus {:?} overlay={:?} contain={} restore={} rect={}",
            scope.id,
            scope.overlay,
            scope.contain,
            scope.restore_on_close,
            fmt_rect(scope.rect)
        );
    }
    for grace in &frame.grace_regions {
        println!(
            "    grace parent={:?} child={:?} shape={:?}",
            grace.parent, grace.child, grace.shape
        );
    }
}

fn fmt_rect(rect: Rect) -> String {
    format!(
        "[{:.0},{:.0} -> {:.0},{:.0}]",
        rect.x0, rect.y0, rect.x1, rect.y1
    )
}
