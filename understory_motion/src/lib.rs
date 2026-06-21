// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![cfg_attr(not(feature = "std"), no_std)]

//! Renderer-neutral motion primitives.
//!
//! This crate owns value interpolation, easing, color interpolation,
//! single-value transitions, decomposed transform data, and basic physics
//! sampling. It explicitly does not own UI elements, dependency properties,
//! style resolution, invalidation, layout, or composition backends.
//!
//! Use this crate when an animation, transition, or gesture recognizer needs
//! pure value math without taking a dependency on a renderer or UI runtime.
//! The core traits are [`Interpolate`] for pairwise interpolation and
//! [`AnimatableValue`] for values that can also participate in additive or
//! accumulative animation stacks.
//!
//! ## Minimal example
//!
//! ```rust
//! use understory_motion::{TimingFunction, Transition};
//!
//! let transition = Transition::new(0.0_f64, 10.0, 0, 100, TimingFunction::LINEAR);
//! let eased = transition.sample(25);
//!
//! assert_eq!(eased, 2.5);
//! ```
//!
//! ## Boundary
//!
//! `understory_motion` treats durations, colors, transforms, and scalar
//! values as already chosen by a higher-level system. It does not resolve
//! style, store properties, drive frame clocks, interpret input events, or
//! submit renderer commands.
//!
//! This crate is `no_std` by default. The default `libm` feature forwards to
//! dependent math and color crates so ordinary builds work without `std`.
//! Enable the `std` feature when an application wants standard-library
//! support instead.

#[cfg(test)]
extern crate std;

mod color;
mod easing;
mod physics;
mod transform;
mod transition;
mod value;

pub use color::{ColorInterpolation, ColorTransition};
pub use easing::TimingFunction;
pub use physics::{Decay, DecaySample, Spring, SpringSample};
pub use transform::Transform2d;
pub use transition::{Transition, millis, normalized_progress};
pub use value::{AnimatableValue, Interpolate};
