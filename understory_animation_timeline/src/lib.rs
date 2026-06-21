// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Timeline abstractions for animation runtimes.
//!
//! This crate owns the source-of-time boundary for animations. It explicitly
//! does not own animation effects, target stacks, property storage, scroll
//! widgets, host frame scheduling, or UI invalidation.
//!
//! A host runtime can implement [`AnimationTimeline`] for frame clocks,
//! scroll timelines, view transitions, or deterministic tests. The timeline
//! returns an optional [`TimelineTime`], which lets a runtime distinguish an
//! inactive source from a source at zero time without involving animation
//! effect state.
//!
//! ## Minimal example
//!
//! ```rust
//! use understory_animation_timeline::{
//!     AnimationTimeline, ManualTimeline, TimelineTime,
//! };
//!
//! let mut timeline = ManualTimeline::new();
//! assert_eq!(timeline.current_time(&()), None);
//!
//! timeline.seek(TimelineTime::from_duration(40));
//! assert_eq!(
//!     timeline.current_time(&()).map(TimelineTime::duration),
//!     Some(40),
//! );
//! ```
//!
//! This crate is `#![no_std]`.

#![no_std]

mod manual;
mod time;
mod timeline;

pub use manual::ManualTimeline;
pub use time::TimelineTime;
pub use timeline::AnimationTimeline;

#[cfg(test)]
mod tests;
