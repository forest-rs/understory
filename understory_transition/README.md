<div align="center">

# Understory Transition

**Style-driven property transitions with retargeting for Understory**

[![Latest published version.](https://img.shields.io/crates/v/understory_transition.svg)](https://crates.io/crates/understory_transition)
[![Documentation build status.](https://img.shields.io/docsrs/understory_transition.svg)](https://docs.rs/understory_transition)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/forest-rs/understory/ci.yml?logo=github&label=CI)](https://github.com/forest-rs/understory/actions)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=understory_transition --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Understory Transition: style-driven property transitions with retargeting.

This crate owns the policy that turns a resolved property value change —
typically a style or pseudo-class state flip — into a smooth transition
written through the animation slot of an `understory_property` store. It
owns per-target transition declarations (duration, delay, easing, color
interpolation), the retargeting/interruption state machine, resolution of
late-bound declaration fields at transition start, host-time sampling, and
the animation-slot write plan.

It deliberately does **not** own property storage, style matching,
selector or pseudo-class state detection, theme/resource storage, keyframe
animations, frame scheduling, clocks, widget trees, or renderer writes.

## Overview

Goal:
own the "transition on style change" recipe that hosts previously had to
hand-write around `understory_motion`: when a resolved value changes,
interpolate from the presented value to the new target, write each sample
into the animation slot, and clear that slot on completion.

Non-goals:
provide a property store, style engine, animation timeline, theme system,
scheduler, or renderer bridge.

## Concepts and glossary

- [`TransitionTiming`]: duration, delay, and easing literals for one
  target.
- [`TransitionDeclaration`]: one per-target declaration with the value
  type and interpolation captured; fields may be literals or
  resource-deferred.
- [`TransitionDeclarations`]: an ordered declaration list that is itself a
  plain property value, so hosts can author transitions in style rules.
- [`TransitionResourceKey`] / [`LateBound`]: a host-defined resource token
  and the literal-or-resource wrapper for declaration fields.
- [`TransitionResources`]: host adapter resolving resource tokens at
  transition start ([`NoTransitionResources`] when everything is literal).
- [`TransitionRunner`]: retained state for many `(owner, target)` pairs;
  owns retargeting and produces write plans.
- [`TransitionWrite`] / [`TransitionWriteOp`]: one animation-slot write —
  set a sampled value, or clear the slot on completion.
- [`TargetUpdate`] / [`TargetOutcome`]: the immediate write and outcome
  returned by [`TransitionRunner::set_target`].
- [`ActiveTransitionInfo`] / [`TransitionPhase`]: read-only snapshots of
  in-flight transitions from [`TransitionRunner::active`].

## Styled transitions

Transitions are meant to be **authored in styles**, the way CSS authors
`transition` declarations. The pattern for a host runtime:

1. Register a "Transitions" dependency property whose value type is
   [`TransitionDeclarations`]. It is a plain `Clone + PartialEq` value, so
   it cascades like any other property value.
2. Author declaration sets in style rules. The winning rule's whole value
   applies; declaration sets are **not merged across rules**.
3. On a state change, re-resolve styled values. For each property whose
   resolved value changed, resolve the "Transitions" property from the
   after-change state and call [`TransitionRunner::set_target`] with the
   old value, the new value, and the resolved declaration set.
4. Apply the immediate write returned by [`TransitionRunner::set_target`]
   before presenting that update, then keep sampling while the runner is
   not idle. Apply every returned write plan in order.

The runner never reads a clock and never schedules work, following the
host-driven time pattern of `understory_animation` and `understory_timing`.

## Consuming write plans

Each write targets the animation slot of the owner's property store.
There are two consumption modes:

- **Direct**: when transitions are the only animation source for a
  target, apply each write with [`TransitionWrite::apply`] (or route the
  erased value through a host adapter). While a transition runs, the slot
  masks the already-updated base value; on completion the runner emits a
  clear so the base value shows through and the store carries no leftover
  animation entry.
- **Composed**: when keyframe effects from `understory_animation` can
  animate the same target, feed the transition's sampled value in as the
  *underlying value* of that target's effect stack (the replace-composite
  base that `TargetStack::sample` composites over), and write the stack's
  output to the animation slot instead. Transitions conceptually sit
  *below* animations, and this ordering preserves that precedence.

If a transition and a keyframe animation both write the shared animation
slot directly, they conflict and the last writer wins; a dedicated
transition stage below the animation slot is possible future work.

## Retargeting and reversing

Interruption is the reason this crate exists. If a transition to value B
is midway and the target changes to C, the replacement starts from the
*current animated value* — never a snap, never a restart from the old
base. When the new target reverses the in-flight transition (hover-off
during hover-on), the replacement's duration is additionally scaled by the
CSS reversing shortening factor, so a transition interrupted at 25% takes
25% of the declared duration to return. Repeated interruptions compose the
factor the same way CSS Transitions §"Faster reversing of interrupted
transitions" does.

Zero combined duration (duration plus delay) means the value applies
immediately, and retargeting to the value currently on screen cancels the
transition and clears the slot. A transition interrupted during its delay
has not moved yet, so reversing it snaps straight back.

## Late-bound resources

Any timing or color-interpolation field of a declaration can defer to a
[`TransitionResourceKey`] instead of a literal. Deferred fields resolve
through the host's [`TransitionResources`] adapter when the transition
**starts** (a retarget starts a replacement transition, so it resolves
again), matching CSS Transitions, which resolves transition parameters
from the after-change style at the moment a transition begins. The payoff:
a theme swap retunes motion for every subsequent transition with zero
rebuild of styles or declaration sets. This crate defines only the token;
hosts map it onto their theme lookup, following the
`understory_property_expression` resource-key precedent.

## Minimal example

```rust
use understory_property::{
    ErasedValue, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
};
use understory_transition::{
    NoTransitionResources, TransitionDeclarations, TransitionRunner, TransitionTiming, millis,
};

let mut registry = PropertyRegistry::new();
let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

let mut transitions = TransitionDeclarations::new();
transitions.declare(width, TransitionTiming::new(millis(100)));

let mut store = PropertyStore::<u32>::new(1);
store.set_local(width, 10.0);

// A style flip resolved Width from 0.0 to 10.0; the base is already 10.0.
let mut runner = TransitionRunner::new();
let update = runner
    .set_target(
        store.owner(),
        width.id(),
        &transitions,
        &NoTransitionResources,
        &ErasedValue::new(0.0_f64),
        ErasedValue::new(10.0_f64),
        0,
    )
    .unwrap();
if let Some(write) = update.into_immediate_write() {
    write.apply(&mut store);
}

for write in runner.sample(millis(50)) {
    write.apply(&mut store);
}
assert_eq!(store.get_effective_local(width, &registry), 5.0);

for write in runner.sample(millis(100)) {
    write.apply(&mut store);
}
assert!(!store.has_animation(width));
assert_eq!(store.get_effective_local(width, &registry), 10.0);
```

## Gotchas and risks

- The host must apply the new resolved value to the base/styled layer when
  it calls [`TransitionRunner::set_target`]; the transition only masks the
  jump, and the completion clear assumes the base already equals the
  target.
- Durations and instants are shared `understory_timing` ticks. Hosts
  conventionally use nanoseconds; [`millis`] converts from milliseconds.
- A missing or mistyped resource makes the affected field fall back to its
  initial value (zero duration/delay, linear easing, default color
  interpolation); a zero resolved duration means the value snaps.
- [`TransitionRunner::set_target`], [`TransitionRunner::sample`],
  [`TransitionRunner::cancel`], and [`TransitionRunner::cancel_owner`]
  return their writes directly. Hosts should apply those writes before
  rendering the resolved tree; the runner does not retain writes for a
  later drain.
- Value types are checked when a target is set, against the type captured
  by the declaration; a mismatch returns [`TransitionTypeMismatch`] and
  leaves the runner unchanged.
- The color-interpolation field is consulted only by declarations created
  with the color constructors ([`TransitionDeclaration::of_color`] /
  [`TransitionDeclaration::for_color_target`]); a plain capture of a color
  type uses the default interpolation policy.

This crate is `no_std` and uses `alloc` for transition storage. The
default `libm` feature forwards to `understory_motion` so easing math
works without `std`; enable the `std` feature when an application wants
standard-library support instead.

<!-- cargo-rdme end -->

[`TransitionTiming`]: https://docs.rs/understory_transition/latest/understory_transition/struct.TransitionTiming.html
[`TransitionDeclaration`]: https://docs.rs/understory_transition/latest/understory_transition/struct.TransitionDeclaration.html
[`TransitionDeclaration::of_color`]: https://docs.rs/understory_transition/latest/understory_transition/struct.TransitionDeclaration.html#method.of_color
[`TransitionDeclaration::for_color_target`]: https://docs.rs/understory_transition/latest/understory_transition/struct.TransitionDeclaration.html#method.for_color_target
[`TransitionDeclarations`]: https://docs.rs/understory_transition/latest/understory_transition/struct.TransitionDeclarations.html
[`TransitionResourceKey`]: https://docs.rs/understory_transition/latest/understory_transition/struct.TransitionResourceKey.html
[`LateBound`]: https://docs.rs/understory_transition/latest/understory_transition/enum.LateBound.html
[`TransitionResources`]: https://docs.rs/understory_transition/latest/understory_transition/trait.TransitionResources.html
[`NoTransitionResources`]: https://docs.rs/understory_transition/latest/understory_transition/struct.NoTransitionResources.html
[`TransitionRunner`]: https://docs.rs/understory_transition/latest/understory_transition/struct.TransitionRunner.html
[`TransitionRunner::set_target`]: https://docs.rs/understory_transition/latest/understory_transition/struct.TransitionRunner.html#method.set_target
[`TransitionRunner::sample`]: https://docs.rs/understory_transition/latest/understory_transition/struct.TransitionRunner.html#method.sample
[`TransitionRunner::active`]: https://docs.rs/understory_transition/latest/understory_transition/struct.TransitionRunner.html#method.active
[`TransitionWrite`]: https://docs.rs/understory_transition/latest/understory_transition/struct.TransitionWrite.html
[`TransitionWrite::apply`]: https://docs.rs/understory_transition/latest/understory_transition/struct.TransitionWrite.html#method.apply
[`TransitionWriteOp`]: https://docs.rs/understory_transition/latest/understory_transition/enum.TransitionWriteOp.html
[`TargetOutcome`]: https://docs.rs/understory_transition/latest/understory_transition/enum.TargetOutcome.html
[`ActiveTransitionInfo`]: https://docs.rs/understory_transition/latest/understory_transition/struct.ActiveTransitionInfo.html
[`TransitionPhase`]: https://docs.rs/understory_transition/latest/understory_transition/enum.TransitionPhase.html
[`TransitionTypeMismatch`]: https://docs.rs/understory_transition/latest/understory_transition/struct.TransitionTypeMismatch.html
[`millis`]: https://docs.rs/understory_motion/latest/understory_motion/fn.millis.html

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
