# Overstory Layout Hardening Plan

## Goal

Make Overstory measurement/layout feel like a real toolkit core instead of a fallback-heavy demo substrate.

## Non-goals

- Full flex/grid layout.
- Paragraph editing or a full text engine redesign.
- Compositor/layer work.

## First slice

1. Change the widget measure seam so widgets receive resolved style inputs.
2. Move built-in text-bearing widgets onto explicit intrinsic measurement.
3. Remove generic leaf-text measurement fallback from `scene.rs`.
4. Make horizontal container layout use measured child widths instead of “remaining width” guesses.

## Risks

- Public `Widget::measure` API break.
- Measurement/display drift if built-ins do not use the same text/padding inputs.
- Horizontal row layout regressions if fill and non-fill extents are mixed incorrectly.

## Success criteria

- `scene.rs` no longer guesses leaf text height from a generic widget-text helper.
- `Button`, `TextBlock`, `TextInput`, `Tooltip`, `Divider`, and `Spinner` all measure through the same widget seam.
- Row children without explicit widths use measured widths rather than consuming all remaining space.
