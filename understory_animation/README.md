<div align="center">

# Understory Animation

**Typed animation timing, effects, and target-stack primitives**

[![Latest published version.](https://img.shields.io/crates/v/understory_animation.svg)](https://crates.io/crates/understory_animation)
[![Documentation build status.](https://img.shields.io/docsrs/understory_animation.svg)](https://docs.rs/understory_animation)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/forest-rs/understory/ci.yml?logo=github&label=CI)](https://github.com/forest-rs/understory/actions)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=understory_animation --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Typed animation timing, effects, and target-stack primitives.

This crate owns playback timing, keyframe sampling, composite operations,
and typed target-stack reduction. It explicitly does not own UI elements,
dependency-property storage, invalidation, frame scheduling, or renderer
writes.

A host runtime is expected to provide timeline time, choose which effects
belong to a target, and write sampled values back into its own property
system. `understory_animation` keeps the pure animation pieces small:
[`AnimationTiming`] maps local time into normalized progress,
[`KeyframeEffect`] samples typed values, and [`TargetStack`] composites
ordered effects over an underlying value.

## Minimal example

```rust
use understory_animation::{
    AnimationTiming, KeyframeEffect, StackEffect, TargetStack,
};
use understory_animation_timeline::TimelineTime;

const MS: u64 = 1_000_000;

let effect = KeyframeEffect::from_values(vec![0.0_f64, 10.0]);
let stack_effect = StackEffect::new(effect, AnimationTiming::new(100 * MS));
let mut stack = TargetStack::new();
stack.push(stack_effect);

let sample = stack.sample(&100.0, TimelineTime::from_duration(25 * MS));

assert_eq!(sample.value, 2.5);
assert_eq!(sample.active_effects, 1);
```

## Boundary

The crate treats time as already resolved into
[`TimelineTime`](understory_animation_timeline::TimelineTime) values and treats
animated values as already chosen by the host. It does not perform
dependency-property lookup, style cascade resolution, invalidation, frame
scheduling, target lookup, or renderer command emission.

This crate is `no_std` by default and uses `alloc` for effect storage.
The default `libm` feature forwards to dependent math crates so ordinary
builds work without `std`. Enable the `std` feature when an application
wants standard-library support instead.

<!-- cargo-rdme end -->

[`AnimationTiming`]: https://docs.rs/understory_animation/latest/understory_animation/struct.AnimationTiming.html
[`KeyframeEffect`]: https://docs.rs/understory_animation/latest/understory_animation/struct.KeyframeEffect.html
[`TargetStack`]: https://docs.rs/understory_animation/latest/understory_animation/struct.TargetStack.html
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
