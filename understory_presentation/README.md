<div align="center">

# Understory Presentation

**Retained resolved drawing primitives for Understory presentation trees**

[![Latest published version.](https://img.shields.io/crates/v/understory_presentation.svg)](https://crates.io/crates/understory_presentation)
[![Documentation build status.](https://img.shields.io/docsrs/understory_presentation.svg)](https://docs.rs/understory_presentation)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/forest-rs/understory/ci.yml?logo=github&label=CI)](https://github.com/forest-rs/understory/actions)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=understory_presentation --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Understory Presentation: retained, resolved drawing intent.

This crate stores the "what to draw" layer that sits between a widget tree
and paint. A toolkit writes already-resolved drawing primitives into a
[`PresentationStore`], keyed by its own geometry ids. Paint can then walk the
geometry tree, look up presentation entries, and lower primitives without
reading properties or running style cascade resolution.

This crate deliberately does **not** own layout bounds, scene traversal
order, global transforms/clips, hit testing, style cascade, property
storage, behavior dispatch, templates, or renderer command emission.

## Fence

This crate owns retained, resolved drawing intent keyed by caller-owned
geometry ids; it explicitly does not own layout/scene geometry,
property/style resolution, behavior, or paint command emission.

## Concepts and glossary

- [`PresentationStore`]: flat keyed cache of presentation nodes plus dirty
  tracking.
- [`PresentationNode`]: source back-reference and primitive list for one
  drawable geometry node.
- [`Primitive`]: resolved drawing primitive stored on a presentation node.
- [`SurfacePrimitive`]: resolved surface fill/border intent.
- [`TextPrimitive`]: umbrella for resolved text drawing intent.
- [`PlainTextPrimitive`]: resolved plain-text content, foreground brush, and
  `parlance`-based single-run style.

## Model

The store is generic over two ids:

- `NodeKey`: the caller's geometry key, often an `understory_box_tree`
  node id.
- `SourceKey`: the caller's widget, element, template part, or diagnostic
  key, used for back-references.

The presentation store is intentionally flat. It stores no parent/child
structure and no layout/scene geometry. Structural truth and traversal
order belong to the caller's geometry tree. Individual primitives may still
own local drawing geometry, such as future path data.

Mutating store operations mark the affected `NodeKey` dirty. Dirty keys are
deduplicated and drained in first-dirty order with
[`PresentationStore::take_dirty`].

## Feature flags

- `default`: enables `libm` so the crate builds as `no_std` by default.
- `libm`: forwards `peniko/libm` for `no_std` float math.
- `std`: forwards `peniko/std` and `parlance/std`.

If default features are disabled, callers must enable either `libm` or
`std`.

```sh
cargo check -p understory_presentation --no-default-features --features libm
cargo check -p understory_presentation --no-default-features --features std
```

## Minimal example

```rust
use understory_presentation::{
    Brush, Color, PresentationStore, Primitive, RoundedRectRadii, TextContent,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceKey {
    widget: u32,
    role: &'static str,
}

let root = 1_u32;
let label = 2_u32;
let source_background = SourceKey { widget: 10, role: "background" };
let source_content = SourceKey { widget: 10, role: "content" };

let mut store: PresentationStore<u32, SourceKey> = PresentationStore::new();
store.insert(root, source_background);
store.insert(label, source_content);

let surface = store.surface_mut(root).unwrap();
surface.set_background(Color::from_rgb8(38, 92, 142));
surface.corner_radii = RoundedRectRadii::from_single_radius(6.0);

let text = store.plain_text_mut(label).unwrap();
text.content = TextContent::plain("Run");
text.foreground = Some(Brush::from(Color::WHITE));

let dirty: Vec<_> = store.take_dirty().collect();
assert_eq!(dirty, vec![root, label]);

let label_node = store.node(label).unwrap();
assert_eq!(label_node.source().role, "content");
assert!(matches!(label_node.primitives()[0], Primitive::Text(_)));
```

<!-- cargo-rdme end -->

[`PresentationNode`]: https://docs.rs/understory_presentation/latest/understory_presentation/struct.PresentationNode.html
[`PresentationStore`]: https://docs.rs/understory_presentation/latest/understory_presentation/struct.PresentationStore.html
[`PresentationStore::take_dirty`]: https://docs.rs/understory_presentation/latest/understory_presentation/struct.PresentationStore.html#method.take_dirty
[`PlainTextPrimitive`]: https://docs.rs/understory_presentation/latest/understory_presentation/struct.PlainTextPrimitive.html
[`Primitive`]: https://docs.rs/understory_presentation/latest/understory_presentation/enum.Primitive.html
[`SurfacePrimitive`]: https://docs.rs/understory_presentation/latest/understory_presentation/struct.SurfacePrimitive.html
[`TextPrimitive`]: https://docs.rs/understory_presentation/latest/understory_presentation/enum.TextPrimitive.html

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

[AUTHORS]: https://github.com/forest-rs/understory/blob/main/AUTHORS
[LICENSE-APACHE]: https://github.com/forest-rs/understory/blob/main/LICENSE-APACHE
[LICENSE-MIT]: https://github.com/forest-rs/understory/blob/main/LICENSE-MIT
[Rust code of conduct]: https://www.rust-lang.org/policies/code-of-conduct
