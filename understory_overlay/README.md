<div align="center">

# Understory Overlay

**Headless overlay lifecycle and interaction state**

[![Latest published version.](https://img.shields.io/crates/v/understory_overlay.svg)](https://crates.io/crates/understory_overlay)
[![Documentation build status.](https://img.shields.io/docsrs/understory_overlay.svg)](https://docs.rs/understory_overlay)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/forest-rs/understory/ci.yml?logo=github&label=CI)](https://github.com/forest-rs/understory/actions)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=understory_overlay --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Headless overlay lifecycle and interaction state.

`understory_overlay` owns deterministic state for floating UI surfaces:
popovers, menus, submenus, tooltips, hover cards, combobox popups, context
menus, and dialogs. It deliberately does not own anchored placement math,
rendering, real focus movement, animation, platform windows, accessibility
backends, or application command policy.

Use [`understory_anchor`] to resolve each overlay's rectangle, then pass
those rectangles into [`build_overlay_frame`]. The resulting
[`OverlayFrame`] gives an embedding toolkit enough information to render
surfaces and underlays, route hits, keep focus contained, and turn input
events into deterministic [`OverlayOp`] values.

## Fence

This crate owns overlay lifecycle, parent/child stack mutation, derived
overlay frames, modality underlays, dismissal regions, focus scope metadata,
hover grace geometry, and event-to-operation resolution; it explicitly does
not own anchor geometry math, rendering, real focus movement, animation,
platform windows, accessibility integration, or app command policy.

## Minimal example

```rust
use kurbo::{Point, Rect};
use understory_overlay::{
    AnchorId, OverlayBehavior, OverlayEntry, OverlayEvent, OverlayFrameInput,
    OverlayGeometry, OverlayId, OverlayLayer, OverlayOp, OverlayStack,
    build_overlay_frame, resolve_event,
};

let mut stack = OverlayStack::new();
stack.apply(OverlayOp::Open {
    entry: OverlayEntry::new(
        OverlayId(1),
        OverlayLayer::Popover,
        OverlayBehavior::Popover,
    )
    .with_anchor(AnchorId(10)),
})?;

let geometries = [OverlayGeometry::new(
    OverlayId(1),
    Rect::new(120.0, 88.0, 320.0, 220.0),
)
.with_anchor_rect(Rect::new(120.0, 56.0, 180.0, 80.0))];
let frame = build_overlay_frame(
    &stack,
    OverlayFrameInput::new(Rect::new(0.0, 0.0, 800.0, 600.0), &geometries)
        .with_pointer(Point::new(16.0, 16.0)),
);

let result = resolve_event(
    &stack,
    &frame,
    OverlayEvent::PointerDown {
        point: Point::new(16.0, 16.0),
    },
);
assert_eq!(result.ops.len(), 1);
```

## Integration shape

A host toolkit typically keeps one [`OverlayStack`] for a scene or window.
Opening a popup pushes an [`OverlayEntry`] with an app-owned [`OverlayId`],
optional [`AnchorId`], behavior, modality, focus policy, and dismiss policy.
Each layout pass resolves anchored geometry elsewhere and calls
[`build_overlay_frame`]. Each input event can then be passed to
[`resolve_event`] or [`OverlayStack::handle_event`] to produce close and
z-order operations.

Hover grace is current-frame geometry. Hosts configure it through
[`OverlayFrameInput::with_grace_policy`]; this crate does not keep timers or
long-lived pointer travel state.

The crate is `no_std` and uses `alloc` when built without the `std` feature.
The default `libm` feature forwards Kurbo's libm-backed geometry support for
ordinary `no_std` builds. Enable `std` when an application wants Kurbo's
standard-library support, and enable `serde` to serialize ids, policies,
stack snapshots, frames, events, and operations.

Geometry and pointer values passed to public entry points must be finite.
Debug builds assert this contract at ingress; release builds rely on callers
to uphold it.

There is no math-free build of this crate: if you disable default features,
enable either `libm` or `std` on `understory_overlay` so Kurbo and its
geometry dependencies have floating-point math support.

<!-- cargo-rdme end -->

## MSRV

This crate is tested with Rust 1.88 and newer.

## License

Dual-licensed under Apache-2.0 and MIT.
