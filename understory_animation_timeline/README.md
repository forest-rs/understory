<div align="center">

# Understory Animation Timeline

**Timeline abstractions for animation runtimes**

[![Latest published version.](https://img.shields.io/crates/v/understory_animation_timeline.svg)](https://crates.io/crates/understory_animation_timeline)
[![Documentation build status.](https://img.shields.io/docsrs/understory_animation_timeline.svg)](https://docs.rs/understory_animation_timeline)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/forest-rs/understory/ci.yml?logo=github&label=CI)](https://github.com/forest-rs/understory/actions)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=understory_animation_timeline --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Timeline abstractions for animation runtimes.

This crate owns the source-of-time boundary for animations. It explicitly
does not own animation effects, target stacks, property storage, scroll
widgets, host frame scheduling, or UI invalidation.

A host runtime can implement [`AnimationTimeline`] for frame clocks,
scroll timelines, view transitions, or deterministic tests. The timeline
returns an optional [`TimelineTime`], which lets a runtime distinguish an
inactive source from a source at zero time without involving animation
effect state.

## Minimal example

```rust
use understory_animation_timeline::{
    AnimationTimeline, ManualTimeline, TimelineTime,
};

let mut timeline = ManualTimeline::new();
assert_eq!(timeline.current_time(&()), None);

timeline.seek(TimelineTime::from_duration(40));
assert_eq!(
    timeline.current_time(&()).map(TimelineTime::duration),
    Some(40),
);
```

This crate is `#![no_std]`.

<!-- cargo-rdme end -->

[`AnimationTimeline`]: https://docs.rs/understory_animation_timeline/latest/understory_animation_timeline/trait.AnimationTimeline.html
[`TimelineTime`]: https://docs.rs/understory_animation_timeline/latest/understory_animation_timeline/struct.TimelineTime.html

## Minimum supported Rust Version (MSRV)

This crate has been verified to compile with **Rust 1.88** and later.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE] or <http://www.apache.org/licenses/LICENSE-2.0>), or
- MIT license ([LICENSE-MIT] or <http://opensource.org/licenses/MIT>),

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you,
as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Contribution

Contributions are welcome by pull request. The [Rust code of conduct] applies.
Please feel free to add your name to the [AUTHORS] file in any substantive pull request.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you,
as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

[LICENSE-APACHE]: https://github.com/forest-rs/understory/blob/main/LICENSE-APACHE
[LICENSE-MIT]: https://github.com/forest-rs/understory/blob/main/LICENSE-MIT
[Rust code of conduct]: https://www.rust-lang.org/policies/code-of-conduct
[AUTHORS]: https://github.com/forest-rs/understory/blob/main/AUTHORS
