# Understory Display List (render tree) — MVP

Status: Planned (this document is a design/implementation checklist).

Owner: understory maintainers.

Scope

- Define a POD display list crate with per-op stable ids and a diff engine.
- Provide groups and stacking contexts with z and optional opacity and blend.
- Integrate coarse damage input from the box tree.
- Carry an optional `SemanticRegionId` on each op for provenance and a11y mapping.

Design notes

- `OpHeader`: `id: OpId`, `group: GroupId`, `z: i32`, `semantic_id: Option<SemanticRegionId>`, `bounds: Rect`.
- Ops:
  - `FillPath`, `StrokePath`, `GlyphRun { run: RunId, origin, paint }`, `Image`,
  - `PushClip`, `PopClip`,
  - `Group { opacity: f32, blend: peniko::BlendMode }` (re-exported as `understory_display::BlendMode`).
- Diff: per-op ids + stable order support insert, move, replace, remove.
- Damage: accept dirty rects and allow adapters to cull intersecting ops.
- `ResourceSnapshot`: runs and images referenced this frame.

Current status

- A first `understory_display` crate now exists in this repository with a
  deliberately small retained display list, rect/rounded-rect ops, and a
  first Parley-backed retained `GlyphRun` slice behind the `std` feature.
- `understory_display` should stay renderer-agnostic and headless in this repository.
- The current Overstory visual demo now shapes labels through
  `understory_display`, then lowers glyph outlines into `imaging` in the
  examples crate.
- Imaging/effects concerns now live in `../imaging`, and layer/compositor management belongs above this crate (for example in `../subduction`).
- Next step is to expand the retained vocabulary carefully without letting the
  crate collapse into either widget/runtime policy or backend-specific paint
  semantics.

Open questions

- Whether to add higher-level “painter” helpers on top of `DisplayListBuilder` (similar to AnyRender’s `PaintScene`) to smooth over common patterns.
- How much more text specialization to bake in beyond the current `GlyphRun`
  slice (stay at glyph runs vs add higher-level paragraph/block ops).
- How aggressive any optional “optimizer” pass should be (e.g. geometry batching, path sharing) before handing off to backends.

Next steps

- Document the coordinate-space expectation explicitly (current examples use physical pixels; clarify how logical coordinates should be handled).
- Decide whether the next higher-level layer is a small `Painter` /
  `PaintScene`-style trait over `DisplayListBuilder`, or a more structured
  `understory_display` block/tree vocabulary shared by presentation nodes,
  SVG, and widgets.
- Tighten the interaction between `Group` / `PushClip` / `PopClip` and backends (see `issue_render_vello_adapter.md`), including semantics of clip vs blend groups.
- Make the resource/layer boundary explicit so `../imaging` and `../subduction` can consume `understory_display` without pulling compositor policy back into this repo.
