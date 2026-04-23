## Goal

Make Overstory widget/container authoring explicit and typed so the primary
authoring path is no longer `append_child(...)` plus scattered `set_local(...)`
and `widget_mut::<...>(...)` calls.

## Non-goals

- Replace the retained tree/property escape hatch.
- Solve all widget composition ergonomics in one pass.
- Redesign layout again.

## First slice

1. Add a generic `Ui::append(parent, spec)` seam.
2. Add typed specs/builders for:
   - `Panel`
   - `Row`
   - `Column`
   - `Spacer`
   - built-in widgets that already exist (`Button`, `TextBlock`, `TextInput`,
     `ScrollView`, `Splitter`, `Divider`, `Spinner`, `Tooltip`)
3. Move shared mount-time element configuration into one reusable internal
   helper so widgets can express:
   - width/height
   - padding/gap
   - fill/visibility/pickability/focusability
   - display name
   - classes
   - style cascade
4. Convert the real authored surfaces first:
   - `overstory_transcript`
   - `examples/overstory_visual_demo.rs`

## Risks

- Builder state leaking into runtime widgets instead of being consumed at mount
  time.
- Making the API “typed” but still requiring too much follow-up mutation.
- Overlapping names between widget runtime types and authoring/container specs.

## Success criteria

- The demo and transcript surface use `Ui::append(...)` for most authored
  structure.
- `append_child(...)` remains available, but clearly lower-level.
- Built-in widget creation reads like intentional UI construction rather than
  tree surgery.
