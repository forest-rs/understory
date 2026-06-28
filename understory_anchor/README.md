<div align="center">

# Understory Anchor

**Headless anchored overlay geometry resolution**

[![Latest published version.](https://img.shields.io/crates/v/understory_anchor.svg)](https://crates.io/crates/understory_anchor)
[![Documentation build status.](https://img.shields.io/docsrs/understory_anchor.svg)](https://docs.rs/understory_anchor)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/forest-rs/understory/ci.yml?logo=github&label=CI)](https://github.com/forest-rs/understory/actions)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=understory_anchor --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Headless anchored overlay geometry resolution.

`understory_anchor` owns deterministic geometry for floating surfaces that
are attached to an anchor: popovers, menus, tooltips, combobox popups,
completion lists, hover cards, context menus, inspector bubbles, and
selection/caret anchored affordances. It deliberately does not own overlay
lifecycle, modality, dismissal, focus movement, rendering, animation,
platform windows, or accessibility integration.

The resolver is pure, but not memoryless. Callers pass lightweight
[`PreviousAnchorFrame`] state back through [`AnchorInput::previous`] so the
scorer can keep a viable incumbent placement when two candidates are nearly
tied. This avoids visible flip-flop near viewport or scroll-container edges
while keeping the resolver itself stateless.

## Fence

This crate owns anchored geometry resolution, candidate diagnostics,
collision response, arrows, transform origins, and previous-frame placement
stability; it explicitly does not own overlay lifecycle, input events,
dismissal, focus, rendering, animation, or platform integration.

## Core flow

Resolution is intentionally explicit:

1. [`AnchorInput`] describes the current scene facts.
2. [`AnchorPolicy`] describes preferred and fallback
   [`AnchorPositionOption`] values, scoring, ordering, and hysteresis.
3. [`resolve_anchor`] returns an [`AnchorFrame`] containing the chosen
   geometry plus every generated [`AnchorCandidate`] in a
   [`CollisionReport`].

This is a useful foundation for CSS Anchor Positioning adapters, but it is
not a CSS implementation. A CSS layer should resolve anchor lookup, cascade,
writing mode, logical axes, `position-area`, `anchor()`, `anchor-size()`, and
`@position-try` rules into physical rectangles and [`AnchorPositionOption`]
values before calling this crate.

## Minimal example

```rust
use kurbo::{Insets, Rect, Size};
use understory_anchor::{
    Anchor, AnchorConstraint, AnchorInput, AnchorPolicy, AnchorPositionOption,
    Placement, resolve_anchor,
};

let constraints = [
    AnchorConstraint::Offset {
        main_axis: 8.0,
        cross_axis: 0.0,
    },
    AnchorConstraint::Shift {
        padding: Insets::uniform(8.0),
    },
    AnchorConstraint::Arrow {
        size: Size::new(12.0, 6.0),
        padding: 8.0,
    },
];
let fallbacks = [
    AnchorPositionOption::new(Placement::TOP).with_constraints(&constraints),
    AnchorPositionOption::new(Placement::RIGHT).with_constraints(&constraints),
    AnchorPositionOption::new(Placement::LEFT).with_constraints(&constraints),
];

let input = AnchorInput {
    anchor: Anchor::Rect(Rect::new(100.0, 100.0, 180.0, 132.0)),
    floating_size: Size::new(260.0, 180.0),
    viewport: Rect::new(0.0, 0.0, 1200.0, 800.0),
    boundary: Rect::new(0.0, 0.0, 1200.0, 800.0),
    previous: None,
};

let policy = AnchorPolicy::new(
    AnchorPositionOption::new(Placement::BOTTOM).with_constraints(&constraints),
    &fallbacks,
);

let frame = resolve_anchor(input, policy);
assert_eq!(frame.placement, Placement::BOTTOM);
assert!(frame.visible);
```

## Multi-rect anchors

Wrapped selections and text carets often expose more than one rectangle.
[`Anchor::Rects`] lets callers choose whether placement should use the
bounding box, first rect, last rect, primary rect, focus rect, or largest
rect.

```rust
use kurbo::{Rect, Size};
use understory_anchor::{
    Anchor, AnchorInput, AnchorPolicy, AnchorRects, Placement, RectReference,
    resolve_anchor,
};

let selection = [
    Rect::new(100.0, 100.0, 240.0, 118.0),
    Rect::new(80.0, 120.0, 260.0, 138.0),
    Rect::new(80.0, 140.0, 140.0, 158.0),
];

let frame = resolve_anchor(
    AnchorInput {
        anchor: Anchor::Rects {
            rects: AnchorRects {
                rects: &selection,
                primary: Some(0),
                focus: Some(2),
            },
            reference: RectReference::Focus,
        },
        floating_size: Size::new(160.0, 80.0),
        viewport: Rect::new(0.0, 0.0, 400.0, 300.0),
        boundary: Rect::new(0.0, 0.0, 400.0, 300.0),
        previous: None,
    },
    AnchorPolicy::placement(Placement::BOTTOM_START),
);

assert_eq!(frame.reference_rect, selection[2]);
```

The crate is `no_std` and uses `alloc` when built without the `std`
feature. The default `libm` feature forwards Kurbo's libm-backed geometry
support for ordinary `no_std` builds. Enable `std` when an application wants
Kurbo's standard-library support, and enable `serde` to serialize policy,
option, frame, candidate, and diagnostic data.

There is no math-free build of this crate: if you disable default features,
enable either `libm` or `std` on `understory_anchor` so Kurbo and its geometry
dependencies have floating-point math support.

<!-- cargo-rdme end -->

[`Anchor::Rects`]: https://docs.rs/understory_anchor/latest/understory_anchor/enum.Anchor.html#variant.Rects
[`AnchorCandidate`]: https://docs.rs/understory_anchor/latest/understory_anchor/struct.AnchorCandidate.html
[`AnchorFrame`]: https://docs.rs/understory_anchor/latest/understory_anchor/struct.AnchorFrame.html
[`AnchorInput`]: https://docs.rs/understory_anchor/latest/understory_anchor/struct.AnchorInput.html
[`AnchorInput::previous`]: https://docs.rs/understory_anchor/latest/understory_anchor/struct.AnchorInput.html#structfield.previous
[`AnchorPolicy`]: https://docs.rs/understory_anchor/latest/understory_anchor/struct.AnchorPolicy.html
[`AnchorPositionOption`]: https://docs.rs/understory_anchor/latest/understory_anchor/struct.AnchorPositionOption.html
[`CollisionReport`]: https://docs.rs/understory_anchor/latest/understory_anchor/struct.CollisionReport.html
[`resolve_anchor`]: https://docs.rs/understory_anchor/latest/understory_anchor/fn.resolve_anchor.html

## Minimum supported Rust Version (MSRV)

This crate has been verified to compile with **Rust 1.88** and later.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE] or <http://www.apache.org/licenses/LICENSE-2.0>), or
- MIT license ([LICENSE-MIT] or <http://opensource.org/licenses/MIT>),

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you,
as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Contribution

Contributions are welcome by pull request. The [Rust code of conduct] applies.
Please feel free to add your name to the [AUTHORS] file in any substantive pull request.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you,
as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

[LICENSE-APACHE]: https://github.com/forest-rs/understory/blob/main/LICENSE-APACHE
[LICENSE-MIT]: https://github.com/forest-rs/understory/blob/main/LICENSE-MIT
[Rust code of conduct]: https://www.rust-lang.org/policies/code-of-conduct
[AUTHORS]: https://github.com/forest-rs/understory/blob/main/AUTHORS
