// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use understory_timing::{TimerDuration, TimerInstant};

use crate::{AnimatableValue, TimingFunction};

/// Converts milliseconds to the shared timer duration unit.
#[must_use]
pub const fn millis(value: u64) -> TimerDuration {
    value.saturating_mul(1_000_000)
}

/// A single-value transition clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition<T: AnimatableValue> {
    from: T,
    to: T,
    started_at: TimerInstant,
    duration: TimerDuration,
    timing: TimingFunction,
}

impl<T: AnimatableValue> Transition<T> {
    /// Creates a transition from `from` to `to`.
    #[must_use]
    pub const fn new(
        from: T,
        to: T,
        started_at: TimerInstant,
        duration: TimerDuration,
        timing: TimingFunction,
    ) -> Self {
        Self {
            from,
            to,
            started_at,
            duration,
            timing,
        }
    }

    /// Returns the transition start value.
    #[must_use]
    pub const fn from(&self) -> &T {
        &self.from
    }

    /// Returns the transition target value.
    #[must_use]
    pub const fn to(&self) -> &T {
        &self.to
    }

    /// Returns the transition start instant.
    #[must_use]
    pub const fn started_at(&self) -> TimerInstant {
        self.started_at
    }

    /// Returns the transition duration.
    #[must_use]
    pub const fn duration(&self) -> TimerDuration {
        self.duration
    }

    /// Returns the timing function.
    #[must_use]
    pub const fn timing(&self) -> TimingFunction {
        self.timing
    }

    /// Samples this transition at `now`.
    #[must_use]
    pub fn sample(&self, now: TimerInstant) -> T {
        let progress = self
            .timing
            .sample(normalized_progress(now, self.started_at, self.duration));
        self.from.interpolate(&self.to, progress)
    }

    /// Returns whether this transition has reached its target by `now`.
    #[must_use]
    pub fn is_complete(&self, now: TimerInstant) -> bool {
        now.saturating_sub(self.started_at) >= self.duration
    }

    /// Returns the instant this transition reaches its target.
    #[must_use]
    pub fn end_time(&self) -> TimerInstant {
        self.started_at.saturating_add(self.duration)
    }
}

/// Returns normalized progress through a duration.
#[must_use]
pub fn normalized_progress(
    now: TimerInstant,
    started_at: TimerInstant,
    duration: TimerDuration,
) -> f64 {
    if duration == 0 {
        return 1.0;
    }
    let elapsed = now.saturating_sub(started_at);
    duration_progress(elapsed, duration)
}

fn duration_progress(elapsed: TimerDuration, duration: TimerDuration) -> f64 {
    if duration == 0 || elapsed >= duration {
        1.0
    } else {
        elapsed as f64 / duration as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_samples_value_with_timing_function() {
        let transition = Transition::new(
            0.0_f32,
            10.0,
            10,
            100,
            TimingFunction::cubic_bezier(0.215, 0.61, 0.355, 1.0),
        );

        assert_eq!(transition.sample(10), 0.0);
        assert!(transition.sample(60) > 5.0);
        assert_eq!(transition.sample(110), 10.0);
        assert!(transition.is_complete(110));
        assert_eq!(transition.end_time(), 110);
    }

    #[test]
    fn normalized_progress_clamps_to_unit_interval() {
        assert_eq!(normalized_progress(0, 10, 100), 0.0);
        assert_eq!(normalized_progress(10, 10, 0), 1.0);
        assert_eq!(normalized_progress(110, 10, 100), 1.0);
    }
}
