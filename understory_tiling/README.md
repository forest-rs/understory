<div align="center">

# Understory Tiling

**Headless tiling, docking, and layout interaction primitives**

[![Latest published version.](https://img.shields.io/crates/v/understory_tiling.svg)](https://crates.io/crates/understory_tiling)
[![Documentation build status.](https://img.shields.io/docsrs/understory_tiling.svg)](https://docs.rs/understory_tiling)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/forest-rs/understory/ci.yml?logo=github&label=CI)](https://github.com/forest-rs/understory/actions)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=understory_tiling --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Understory Tiling: headless tiling, docking, and layout interaction primitives.

This crate provides a small, renderer-agnostic core for persistent tiling
trees and flattened layout frames. It is intended to sit underneath docking
widgets and workbench shells without knowing about any particular renderer,
widget system, document model, or window system.

The core concepts are:

- [`TileTree`]: persistent semantic tree of splits, tab groups, and panes.
- [`LayoutFrame`]: flattened solved geometry for rendering and hit testing.
- [`TileOp`]: semantic committed mutations such as split, move, activate,
  reorder, and resize.
- [`DockProposal`] and [`ResizeProposal`]: uncommitted interaction results
  that can be previewed before they mutate the tree.
- [`PaneId`] and [`TileId`]: opaque ids used to integrate with an embedding
  application.

This crate deliberately does **not** know about:

- pane contents,
- tab or button drawing,
- themes or animation,
- document save prompts,
- native windows,
- accessibility backends,
- Overstory-specific integration.

## Fence

This crate owns semantic layout structure, layout solving, flattened frames,
hit regions, semantic mutation operations, and proposal plumbing; it
explicitly does not own pane contents, chrome drawing, app policy, document
lifecycle, renderer integration, or widget behavior.

## Minimal example

```rust
use kurbo::{Point, Rect, Size};
use understory_tiling::{
    hit_test, Axis, LayoutInput, PaneId, Placement, TileOp, TileTree,
};

let mut tree = TileTree::single_pane(PaneId(1));
tree.apply(TileOp::SplitPane {
    pane: PaneId(1),
    axis: Axis::Horizontal,
    new_pane: PaneId(2),
    placement: Placement::After,
    share: 0.5,
})?;

let frame = tree.layout(LayoutInput {
    bounds: Rect::new(0.0, 0.0, 800.0, 600.0),
    tab_bar_thickness: 28.0,
    split_handle_thickness: 6.0,
    min_pane_size: Size::new(80.0, 80.0),
});

assert_eq!(frame.panes().len(), 2);
assert!(hit_test(&frame, Point::new(10.0, 10.0)).is_some());
```

## Compositional construction and semantic panes

Use [`TileSpec`] to describe nested initial layouts without assigning
[`TileId`] values. [`TileTree::panes`] then traverses every application pane,
including inactive tabs, in deterministic split-child and tab order.

```rust
use understory_tiling::{Axis, PaneId, TileSpec, TileTree};

let tree = TileTree::from_spec(TileSpec::split(
    Axis::Horizontal,
    [
        TileSpec::tabs([PaneId(1), PaneId(2)]),
        TileSpec::tabs([PaneId(3), PaneId(4)]),
    ],
    [0.6, 0.4],
))?;

assert_eq!(
    tree.panes().collect::<Vec<_>>(),
    [PaneId(1), PaneId(2), PaneId(3), PaneId(4)],
);
```

## Interaction Path

```rust
use kurbo::{Point, Rect, Size};
use understory_tiling::{
    Axis, DockPolicy, InteractionOptions, LayoutInput, PaneId, Placement,
    TileOp, TileTree, begin_interaction, commit_proposal,
    update_interaction, validate_interaction_update,
};

let mut tree = TileTree::single_pane(PaneId(1));
tree.apply(TileOp::SplitPane {
    pane: PaneId(1),
    axis: Axis::Horizontal,
    new_pane: PaneId(2),
    placement: Placement::After,
    share: 0.5,
})?;

let layout_input = LayoutInput {
    bounds: Rect::new(0.0, 0.0, 800.0, 600.0),
    tab_bar_thickness: 28.0,
    split_handle_thickness: 6.0,
    min_pane_size: Size::new(80.0, 80.0),
};
let frame = tree.layout(layout_input);
let options = InteractionOptions::from_layout_input(layout_input);
let policy = DockPolicy::default();

let mut state = begin_interaction(&frame, Point::new(20.0, 20.0), &options);
let update = update_interaction(
    &tree,
    &frame,
    &mut state,
    Point::new(760.0, 20.0),
    &options,
);

let _overlay = &update.overlay;
let _preview = &update.preview;

if update.proposal.is_some() {
    let validated =
        validate_interaction_update(&tree, &frame, &update, &policy, &options)?;
    commit_proposal(&mut tree, validated)?;
}
```

## Externally resolved tab geometry

[`TileTree::layout`] emits deterministic equal-sized tab rectangles and a
[`TabBarFrame::viewport`] equal to the complete reserved bar. A retained UI
toolkit can arrange richer headers and then call
[`LayoutFrame::apply_tab_group_geometry`] so rendering, hit testing, and
reorder insertion consume its measured rectangles. The method validates the
complete pane set before changing the frame.

Presentation transforms compose in an explicit order: solve a base frame,
apply resolved tab geometry, then call [`LayoutFrame::apply_projection`] if
needed. Interaction preview frames are also base frames; finalize them with
the same presentation transforms before display.

This crate supports `no_std` + `alloc` builds when default features are
disabled and the `libm` feature is enabled for Kurbo geometry math. Enable
the `serde` feature to persist valid layout trees and snapshots and to
serialize renderer-independent frame, policy, and interaction output data.

<!-- cargo-rdme end -->

[`DockProposal`]: https://docs.rs/understory_tiling/latest/understory_tiling/enum.DockProposal.html
[`LayoutFrame`]: https://docs.rs/understory_tiling/latest/understory_tiling/struct.LayoutFrame.html
[`LayoutFrame::apply_projection`]: https://docs.rs/understory_tiling/latest/understory_tiling/struct.LayoutFrame.html#method.apply_projection
[`PaneId`]: https://docs.rs/understory_tiling/latest/understory_tiling/struct.PaneId.html
[`ResizeProposal`]: https://docs.rs/understory_tiling/latest/understory_tiling/struct.ResizeProposal.html
[`TileId`]: https://docs.rs/understory_tiling/latest/understory_tiling/struct.TileId.html
[`TileOp`]: https://docs.rs/understory_tiling/latest/understory_tiling/enum.TileOp.html
[`TileSpec`]: https://docs.rs/understory_tiling/latest/understory_tiling/enum.TileSpec.html
[`TileTree`]: https://docs.rs/understory_tiling/latest/understory_tiling/struct.TileTree.html
[`TileTree::panes`]: https://docs.rs/understory_tiling/latest/understory_tiling/struct.TileTree.html#method.panes

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
