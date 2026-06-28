// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use understory_animation_timeline::TimelineTime;
use understory_motion::TimingFunction;
use understory_timing::TimerDuration;

use crate::math::{ceil, floor};
use crate::{FillMode, PlaybackDirection};

/// Complete timing parameters for an animation effect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimationTiming {
    /// Delay before the active interval starts.
    pub delay: TimerDuration,
    /// Delay after the active interval ends.
    pub end_delay: TimerDuration,
    /// Duration of one iteration.
    pub duration: TimerDuration,
    /// Number of iterations. `f64::INFINITY` represents indefinite playback.
    pub iterations: f64,
    /// Direction used for each iteration.
    pub direction: PlaybackDirection,
    /// Fill behavior outside the active interval.
    pub fill: FillMode,
    /// Easing applied after direction is resolved.
    pub easing: TimingFunction,
}

impl AnimationTiming {
    /// Creates a single-iteration linear timing.
    #[must_use]
    pub const fn new(duration: TimerDuration) -> Self {
        Self {
            delay: 0,
            end_delay: 0,
            duration,
            iterations: 1.0,
            direction: PlaybackDirection::Normal,
            fill: FillMode::None,
            easing: TimingFunction::LINEAR,
        }
    }

    /// Samples this timing at `time`, where `time` is local to the animation
    /// start.
    #[must_use]
    pub fn sample(self, time: TimelineTime) -> TimingSample {
        let local = time.duration();
        if local < self.delay {
            return self.before_sample();
        }

        let active_duration = self.active_duration();
        let active_elapsed = local.saturating_sub(self.delay);

        match active_duration {
            Some(0) => self.after_sample(),
            Some(duration) if active_elapsed >= duration => self.after_sample(),
            Some(_) | None => self.active_sample(active_elapsed),
        }
    }

    fn before_sample(self) -> TimingSample {
        if self.fill.fills_backwards() {
            TimingSample::with_progress(
                TimingPhase::Before,
                Some(0),
                self.directed_progress(0, 0.0),
                self.easing,
            )
        } else {
            TimingSample::empty(TimingPhase::Before)
        }
    }

    fn after_sample(self) -> TimingSample {
        if self.fill.fills_forwards() {
            let (iteration, progress) = self.final_iteration_progress();
            TimingSample::with_progress(
                TimingPhase::After,
                Some(iteration),
                self.directed_progress(iteration, progress),
                self.easing,
            )
        } else {
            TimingSample::empty(TimingPhase::After)
        }
    }

    fn active_sample(self, active_elapsed: TimerDuration) -> TimingSample {
        if self.duration == 0
            || (!self.iterations.is_finite() && self.iterations.is_sign_negative())
        {
            return TimingSample::empty(TimingPhase::After);
        }

        let simple_progress = active_elapsed as f64 / self.duration as f64;
        let iteration = f64_floor_to_u64(simple_progress);
        let iteration_progress = simple_progress - iteration as f64;
        TimingSample::with_progress(
            TimingPhase::Active,
            Some(iteration),
            self.directed_progress(iteration, iteration_progress),
            self.easing,
        )
    }

    /// Returns the duration of the active interval, or `None` for indefinite
    /// positive iterations.
    #[must_use]
    pub fn active_duration(self) -> Option<TimerDuration> {
        if self.duration == 0 || self.iterations <= 0.0 {
            return Some(0);
        }
        if self.iterations.is_infinite() && self.iterations.is_sign_positive() {
            return None;
        }
        if !self.iterations.is_finite() {
            return Some(0);
        }
        Some(f64_ceil_to_duration(self.duration as f64 * self.iterations))
    }

    /// Returns the full finite effect duration including delay and end delay.
    ///
    /// Returns `None` for indefinite positive iterations.
    #[must_use]
    pub fn total_duration(self) -> Option<TimerDuration> {
        let active = self.active_duration()?;
        Some(
            self.delay
                .saturating_add(active)
                .saturating_add(self.end_delay),
        )
    }

    /// Returns the final iteration index reached by finite active playback.
    ///
    /// This is useful for retained playback bookkeeping at a natural finish,
    /// where the ordinary timing sample may be empty because forwards fill is
    /// disabled.
    #[must_use]
    pub fn completion_iteration(self) -> Option<u64> {
        if self.active_duration()? == 0 {
            return None;
        }
        Some(self.final_iteration_progress().0)
    }

    fn final_iteration_progress(self) -> (u64, f64) {
        if self.iterations <= 0.0 || !self.iterations.is_finite() {
            return (0, 0.0);
        }

        let floor_value = floor(self.iterations);
        let floor_iteration = f64_floor_to_u64(floor_value);
        let fraction = self.iterations - floor_value;
        if fraction == 0.0 {
            (floor_iteration.saturating_sub(1), 1.0)
        } else {
            (floor_iteration, fraction)
        }
    }

    fn directed_progress(self, iteration: u64, progress: f64) -> f64 {
        match self.direction {
            PlaybackDirection::Normal => progress,
            PlaybackDirection::Reverse => 1.0 - progress,
            PlaybackDirection::Alternate => {
                if iteration.is_multiple_of(2) {
                    progress
                } else {
                    1.0 - progress
                }
            }
            PlaybackDirection::AlternateReverse => {
                if iteration.is_multiple_of(2) {
                    1.0 - progress
                } else {
                    progress
                }
            }
        }
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "finite non-negative animation iteration indexes saturate before conversion"
)]
fn f64_floor_to_u64(value: f64) -> u64 {
    if !value.is_finite() || value <= 0.0 {
        0
    } else if value >= u64::MAX as f64 {
        u64::MAX
    } else {
        floor(value) as u64
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "finite non-negative animation durations saturate before conversion"
)]
fn f64_ceil_to_duration(value: f64) -> TimerDuration {
    if !value.is_finite() || value <= 0.0 {
        0
    } else if value >= TimerDuration::MAX as f64 {
        TimerDuration::MAX
    } else {
        ceil(value) as TimerDuration
    }
}

impl Default for AnimationTiming {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Phase of the animation's local time relative to its active interval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimingPhase {
    /// Local time is before the active interval.
    Before,
    /// Local time is inside the active interval.
    Active,
    /// Local time is after the active interval.
    After,
}

/// Normalized timing sample for one local time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimingSample {
    /// Phase of the sampled local time.
    pub phase: TimingPhase,
    /// Current iteration index, when the animation is producing a value.
    pub iteration: Option<u64>,
    /// Direction-adjusted progress before easing, when the animation is
    /// producing a value.
    pub progress: Option<f64>,
    /// Direction-adjusted and eased progress, when the animation is producing a
    /// value.
    pub eased_progress: Option<f64>,
}

impl TimingSample {
    pub(crate) fn empty(phase: TimingPhase) -> Self {
        Self {
            phase,
            iteration: None,
            progress: None,
            eased_progress: None,
        }
    }

    fn with_progress(
        phase: TimingPhase,
        iteration: Option<u64>,
        progress: f64,
        easing: TimingFunction,
    ) -> Self {
        Self {
            phase,
            iteration,
            progress: Some(progress),
            eased_progress: Some(easing.sample(progress)),
        }
    }
}
