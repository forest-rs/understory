<!-- Instructions

This changelog follows the patterns described here: <https://keepachangelog.com/en/>.

Subheadings to categorize changes are `added, changed, deprecated, removed, fixed, security`.

-->

# Changelog

Understory Box Decoration has not had a published release yet.

## [Unreleased]

### Added

- Added the initial `understory_box_decoration` crate with `no_std` resolved
  geometry primitives for CSS-style box decorations.
- Added `Edges`, `Corners`, `CornerRadii`, `CornerShape`, `Superellipse`,
  `BoxContour`, and `BoxDecorationGeometry` for resolved physical edge widths,
  fitted elliptical corner radii, shaped corner contours, derived
  border/padding/content geometry, and on-demand Kurbo path writing.
- Added `Side`, `BoxArea`, and `BorderSideGeometry` for physical box-area
  selection and central border-side regions.
- Added CSS specification baseline documentation for CSS Backgrounds and
  Borders Level 3 contours, plus initial CSS Borders and Box Decorations Level
  4 `corner-shape` / superellipse coverage and a roadmap for broader Level 4
  features.

### Changed

- Made the default feature set `libm` instead of `std` so the crate remains
  `no_std`-friendly by default while still forwarding Kurbo floating-point
  support.

[Unreleased]: https://github.com/forest-rs/understory/compare/HEAD

[MSRV]: README.md#minimum-supported-rust-version-msrv
