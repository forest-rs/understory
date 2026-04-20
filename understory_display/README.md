# Understory Display

Small retained display-tree and retained command-stream primitives between higher-level
retained UI/runtime layers and renderer-facing paint backends.

`understory_display` owns:

- a retained display tree for local measure/place,
- stable display item ids,
- retained display entries as a flat lowering target,
- retained display items with z/bounds/provenance inside that command stream,
- a calm display-op vocabulary for common 2D draws,
- and, with the `std` feature, Parley-backed retained glyph runs,
- a tiny builder API.

It does not own:

- widget/runtime policy,
- rich text or paragraph semantics,
- renderer backends,
- compositor policy.

## First slice

The current crate intentionally starts small:

- `DisplayTree`
- `DisplayNode`
- `BoxConstraints`
- `Insets`
- `DisplayList`
- `DisplayItem`
- `DisplayOp`
- `DisplayListBuilder`
- `ItemId`
- `SemanticId`

The initial retained-node set is enough to pressure-test Overstory and imaging
without pretending the full text/presentation problem is already solved:

- stacks / padding / alignment / offsets / fixed frames
- rectangular clips / opacity scopes / transforms
- filled rects
- stroked rects
- filled rounded rects
- stroked rounded rects
- and, with the `std` feature, retained glyph runs shaped with Parley

## Example integration

See the workspace `understory_examples` crate for:

- lowering `overstory::SceneSnapshot` into a retained `understory_display::DisplayTree`
- lowering the retained tree into a retained `understory_display::DisplayList`
- lowering `DisplayList` into `imaging::record::Scene`
- rendering the result in `overstory_visual_demo.rs` with real glyph runs
