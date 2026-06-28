// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Typed animation timing, effects, and target-stack primitives.
//!
//! This crate owns playback timing, keyframe sampling, composite operations,
//! and typed target-stack reduction. It explicitly does not own UI elements,
//! dependency-property storage, invalidation, frame scheduling, or renderer
//! writes.
//!
//! A host runtime is expected to provide timeline time, choose which effects
//! belong to a target, and write sampled values back into its own property
//! system. `understory_animation` keeps the pure animation pieces small:
//! [`AnimationTiming`] maps local time into normalized progress,
//! [`KeyframeEffect`] samples typed values, and [`TargetStack`] composites
//! ordered effects over an underlying value.
//!
//! ## Minimal example
//!
//! ```rust
//! use understory_animation::{
//!     AnimationTiming, KeyframeEffect, StackEffect, TargetStack,
//! };
//! use understory_animation_timeline::TimelineTime;
//!
//! const MS: u64 = 1_000_000;
//!
//! let effect = KeyframeEffect::from_values(vec![0.0_f64, 10.0]);
//! let stack_effect = StackEffect::new(effect, AnimationTiming::new(100 * MS));
//! let mut stack = TargetStack::new();
//! stack.push(stack_effect);
//!
//! let sample = stack.sample(&100.0, TimelineTime::from_duration(25 * MS));
//!
//! assert_eq!(sample.value, 2.5);
//! assert_eq!(sample.active_effects, 1);
//! ```
//!
//! ## Boundary
//!
//! The crate treats time as already resolved into
//! [`TimelineTime`](understory_animation_timeline::TimelineTime) values and treats
//! animated values as already chosen by the host. It does not perform
//! dependency-property lookup, style cascade resolution, invalidation, frame
//! scheduling, target lookup, or renderer command emission.
//!
//! This crate is `no_std` by default and uses `alloc` for effect storage.
//! The default `libm` feature enables local libm-backed floating point helpers
//! and forwards to dependent math crates so ordinary builds work without
//! `std`. Enable the `std` feature when an application wants standard-library
//! support instead.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod effect;
mod instance;
mod math;
mod playback;
mod stack;
mod timing;

pub use effect::{CompositeOperation, Keyframe, KeyframeEffect};
pub use instance::{AnimationIterationChange, RetainedAnimationEffect, RetainedAnimationInstance};
pub use playback::{
    AnimationPlayState, AnimationPlayback, AnimationPlaybackEventKind, FillMode, PlaybackDirection,
};
pub use stack::{StackEffect, TargetSample, TargetStack, sample_effects};
pub use timing::{AnimationTiming, TimingPhase, TimingSample};

#[cfg(test)]
mod tests;
