# understory_display Plan

## Fence

`understory_display` owns a small retained display/document recording layer between higher-level retained toolkits and renderer-facing paint backends. It does not own widget/runtime policy, text shaping, renderer backends, or compositor policy.

## Overview

Goal:
- Start a real `understory_display` crate with a calm retained display list and stable item ids.
- Prove the boundary by lowering Overstory's resolved scene into `understory_display`, then lowering that into `imaging` from the examples layer.
- Keep `overstory` renderer-agnostic and avoid letting the current `imaging` demo become the de facto display API.

Non-goals:
- Do not add text layout or paragraph modeling.
- Do not add renderer dependencies to `understory_display`.
- Do not take on widget semantics, event routing, or command/presentation policy.
- Do not overfit the API to the current Overstory example.

## Chosen first slice

Create a small `understory_display` crate with:
- stable `ItemId`,
- a retained `DisplayList`,
- a small `DisplayItem` record carrying bounds, z, and an optional semantic id,
- basic draw ops for filled/stroked rects and rounded rects,
- a tiny builder API.

Then add example-local support code in the `understory_examples` crate:
- lower `overstory::SceneSnapshot` -> `understory_display::DisplayList`,
- lower `understory_display::DisplayList` -> `imaging::record::Scene`.

## Why this shape

- It keeps the display seam visible and concrete without forcing a premature renderer dependency into the core crates.
- It gives Overstory a believable next consumer that is not just "draw directly into imaging".
- It lets the examples pressure-test both boundaries independently:
  - retained UI/runtime -> display recording
  - display recording -> imaging backend

## Planned steps

1. Add `understory_display` as a new workspace crate with crate docs, core ids/types, builder helpers, and tests.
2. Add an `understory_examples` support module for:
   - Overstory -> display lowering
   - display -> imaging lowering
3. Update `overstory_visual_demo` to use those helpers instead of recording directly from `SceneSnapshot` into `imaging`.
4. Tighten docs/README entries so the repo story reflects the new layering.

## Risks

- The first display list might be too low-level and just mirror imaging.
- The first display list might be too high-level and accidentally absorb widget/runtime concepts.
- Text is intentionally deferred, so the example must keep using placeholder bars for labels rather than pretending text is solved.
