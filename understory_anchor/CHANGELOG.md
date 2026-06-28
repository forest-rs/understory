<!-- Instructions

This changelog follows the patterns described here: <https://keepachangelog.com/en/>.

Subheadings to categorize changes are `added, changed, deprecated, removed, fixed, security`.

-->

# Changelog

Understory Anchor has not had a published release yet.

## [Unreleased]

### Added

- Added the initial `understory_anchor` crate with `no_std` anchored geometry
  resolution for overlays, popovers, menus, tooltips, combobox popups, and
  selection/caret anchored surfaces.
- Added placement, anchor, policy, constraint, candidate, diagnostic, arrow,
  collision report, and previous-frame hysteresis APIs built on `kurbo`
  geometry types.
- Added first-class position options so CSS-like adapters can vary placement,
  constraints, ordering, and stable previous-frame identity per fallback.
- Added pure-data regression tests covering fallback placement, multi-rect
  anchors, collision shifting, size shrink/rejection, arrows, transform
  origins, detachment, per-option constraints, fallback ordering, and
  hysteresis.

[Unreleased]: https://github.com/forest-rs/understory/compare/HEAD

[MSRV]: README.md#minimum-supported-rust-version-msrv
