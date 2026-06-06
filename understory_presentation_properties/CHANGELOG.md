<!-- Instructions

This changelog follows the patterns described here: <https://keepachangelog.com/en/>.

Subheadings to categorize changes are `added, changed, deprecated, removed, fixed, security`.

-->

# Changelog

Understory Presentation Properties has not had a published release yet.

## [Unreleased]

### Added

- Added the initial `understory_presentation_properties` crate with `no_std`
  dependency-property integration for resolved presentation surfaces.
- Added `SurfaceProperties` for registering canonical surface background,
  border brush, border width, padding width, corner radius, and corner shape
  properties.
- Added `SurfacePropertyValues` and `SurfaceProperties::resolve_surface` for
  resolving property/style/theme values into `understory_presentation`
  `SurfacePrimitive` values.
- Added `StyleMatch` as the named style resolver input instead of exposing the
  raw `StyleCascade` and `MatchState` tuple in the happy path.
- Added invalidation channel metadata for distinguishing paint-only properties
  from geometry-and-paint surface properties.

[Unreleased]: https://github.com/forest-rs/understory/compare/HEAD

[MSRV]: README.md#minimum-supported-rust-version-msrv
