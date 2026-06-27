<!-- Instructions

This changelog follows the patterns described here: <https://keepachangelog.com/en/>.

Subheadings to categorize changes are `added, changed, deprecated, removed, fixed, security`.

-->

# Changelog

Understory Tiling has not had a published release yet.

## [Unreleased]

### Added

- Added the initial `understory_tiling` crate with `no_std` headless tiling,
  docking, layout frame, hit testing, operation, proposal, and interaction
  primitives.
- Added the persistent `TileTree` model with n-ary splits, tab groups, pane
  leaves, normalization, semantic operations, and flattened layout solving.
- Added basic drag/drop and resize proposal generation, validation and commit
  helpers, snapshot and repair shells, and pure-data regression tests.

[Unreleased]: https://github.com/forest-rs/understory/compare/HEAD

[MSRV]: README.md#minimum-supported-rust-version-msrv
