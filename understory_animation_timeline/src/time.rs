// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use understory_timing::TimerDuration;

/// Monotonic animation timeline time.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct TimelineTime(TimerDuration);

impl TimelineTime {
    /// Zero timeline time.
    pub const ZERO: Self = Self(0);

    /// Creates a timeline time from a timer duration.
    #[must_use]
    pub const fn from_duration(duration: TimerDuration) -> Self {
        Self(duration)
    }

    /// Returns the underlying timer duration.
    #[must_use]
    pub const fn duration(self) -> TimerDuration {
        self.0
    }

    /// Returns a time `duration` after `self`, saturating on overflow.
    #[must_use]
    pub const fn saturating_add(self, duration: TimerDuration) -> Self {
        Self(self.0.saturating_add(duration))
    }

    /// Returns the duration from `earlier` to `self`, saturating at zero.
    #[must_use]
    pub const fn saturating_duration_since(self, earlier: Self) -> TimerDuration {
        self.0.saturating_sub(earlier.0)
    }
}
