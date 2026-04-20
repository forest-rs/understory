# Understory Display

Small retained display list primitives between higher-level retained UI/runtime
layers and renderer-facing paint backends.

`understory_display` owns:

- stable display item ids,
- retained display items with z/bounds/provenance,
- a calm display-op vocabulary for common 2D draws,
- a tiny builder API.

It does not own:

- widget/runtime policy,
- text shaping,
- renderer backends,
- compositor policy.

## First slice

The current crate intentionally starts small:

- `DisplayList`
- `DisplayItem`
- `DisplayOp`
- `DisplayListBuilder`
- `ItemId`
- `SemanticId`

The initial op set is enough to pressure-test Overstory and imaging without
pretending the text/presentation problem is already solved:

- filled rects
- stroked rects
- filled rounded rects
- stroked rounded rects

## Example integration

See the workspace `understory_examples` crate for:

- lowering `overstory::SceneSnapshot` into `understory_display::DisplayList`
- lowering `DisplayList` into `imaging::record::Scene`
- rendering the result in `overstory_visual_demo.rs`
