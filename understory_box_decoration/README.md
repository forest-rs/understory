<div align="center">

# Understory Box Decoration

**Renderer-neutral box decoration geometry primitives**

[![Latest published version.](https://img.shields.io/crates/v/understory_box_decoration.svg)](https://crates.io/crates/understory_box_decoration)
[![Documentation build status.](https://img.shields.io/docsrs/understory_box_decoration.svg)](https://docs.rs/understory_box_decoration)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/forest-rs/understory/ci.yml?logo=github&label=CI)](https://github.com/forest-rs/understory/actions)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=understory_box_decoration --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Renderer-neutral box decoration geometry primitives.

`understory_box_decoration` owns resolved geometry for painted boxes:
physical edge widths, box-area contours, corner shapes, side regions, and
on-demand path emission. It deliberately leaves style cascade, CSS parsing,
layout, brushes, images, hit policy, and renderer command emission to
higher-level crates.

The first implemented contour family covers CSS-style box contours with
elliptical radii and shaped corners. It supports round, square, bevel, and
superellipse-based corner shapes, scales adjacent radii with the CSS
smallest factor rule when they would overlap a side, and derives padding
and content contours from concrete border and padding widths.

## Specification baseline

The current API is based on the box contour pieces of
[CSS Backgrounds and Borders Module Level 3], specifically the
`border-radius` model, elliptical corner radii, inner edge derivation, and
the radius overlap reduction rule for adjacent corners.

[CSS Borders and Box Decorations Module Level 4] informs the contour model,
especially `corner-shape` and superellipse corners. Larger Level 4 features
such as `border-shape`, partial borders, and richer shadow controls remain
roadmap material. This crate is intended to grow toward those features
while keeping style parsing and renderer lowering outside the crate
boundary.

## Minimal example

```rust
use kurbo::{BezPath, Rect, Size};
use understory_box_decoration::{
    BoxArea, BoxDecorationGeometry, CornerRadii, CornerShape, CornerShapes,
    Edges,
};

let geometry = BoxDecorationGeometry::from_border_box(
    Rect::new(0.0, 0.0, 120.0, 80.0),
    Edges::all(4.0),
    Edges::all(8.0),
    CornerRadii::all(Size::new(18.0, 12.0)),
    CornerShapes::all(CornerShape::squircle()),
);

assert_eq!(geometry.padding_box, Rect::new(4.0, 4.0, 116.0, 76.0));
assert_eq!(geometry.content_box, Rect::new(12.0, 12.0, 108.0, 68.0));

// A renderer can reuse path storage while asking for the concrete paths it
// needs for a fill, clip, border, shadow, or hit region.
let mut clip_path = BezPath::new();
geometry.write_background_clip(BoxArea::Padding, &mut clip_path);

let mut border_path = BezPath::new();
geometry.write_border_ring_path(&mut border_path);
assert!(!clip_path.is_empty());
assert!(!border_path.is_empty());
```

## Boundary and invariants

This crate treats inputs as already resolved into local coordinate units.
Constructors harden those inputs for geometry consumers:

- rectangles are normalized to non-negative width and height;
- negative or non-finite border widths become zero;
- negative or non-finite padding widths become zero;
- negative or non-finite radii become zero;
- border-edge radii are scaled so top, right, bottom, and left side pairs
  fit;
- padding and content edge radii are derived from the previous contour's
  radii and then scaled to fit their own boxes.

The crate itself is `#![no_std]`. The default `libm` feature forwards to
Kurbo's libm-backed floating point helpers so ordinary builds remain
`no_std`-friendly. Enable the `std` feature when an application wants
Kurbo's standard-library support.

## Roadmap

Near-term work should add resolved length-percentage radii so CSS parsing
layers can defer percentage resolution until the border box is known. After
that, the natural coverage expansion is corner transition regions,
`box-shadow` spread geometry, and richer background painting areas. Level 4
`border-shape` should probably consume a separate CSS-shapes value crate
rather than making this crate own every shape syntax.

[CSS Backgrounds and Borders Module Level 3]: https://www.w3.org/TR/css-backgrounds-3/#border-radius
[CSS Borders and Box Decorations Module Level 4]: https://drafts.csswg.org/css-borders-4/

<!-- cargo-rdme end -->

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
