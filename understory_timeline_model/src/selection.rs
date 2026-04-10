// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::ops::Range;

use crate::{TimelineError, TimelineItemKey, TimelineResult};

/// A normalized time range.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TimelineTimeRange {
    start: f64,
    end: f64,
}

impl TimelineTimeRange {
    /// Creates a normalized time range from two finite endpoints.
    pub fn try_new(a: f64, b: f64) -> TimelineResult<Self> {
        if !a.is_finite() || !b.is_finite() {
            return Err(TimelineError::NonFiniteSelection { start: a, end: b });
        }
        Ok(Self {
            start: a.min(b),
            end: a.max(b),
        })
    }

    /// Returns the normalized start time.
    #[must_use]
    pub fn start(&self) -> f64 {
        self.start
    }

    /// Returns the normalized end time.
    #[must_use]
    pub fn end(&self) -> f64 {
        self.end
    }

    /// Returns the selection duration.
    #[must_use]
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }

    /// Returns the selection as a standard range.
    #[must_use]
    pub fn range(&self) -> Range<f64> {
        self.start..self.end
    }

    pub(crate) fn translate_by(self, delta: f64) -> TimelineResult<Self> {
        if !delta.is_finite() {
            return Err(TimelineError::NonFiniteDelta { delta });
        }
        Self::try_new(self.start + delta, self.end + delta).map_err(|_| {
            TimelineError::NonFiniteEditedSelection {
                start: self.start + delta,
                end: self.end + delta,
            }
        })
    }

    pub(crate) fn with_start(self, start: f64) -> TimelineResult<Self> {
        Self::try_new(start, self.end).map_err(|_| TimelineError::NonFiniteEditedSelection {
            start,
            end: self.end,
        })
    }

    pub(crate) fn with_end(self, end: f64) -> TimelineResult<Self> {
        Self::try_new(self.start, end).map_err(|_| TimelineError::NonFiniteEditedSelection {
            start: self.start,
            end,
        })
    }
}

/// User selection state stored with a timeline document.
///
/// Time-range selection is useful for zooming or measuring. Span and marker
/// selections are stored by stable [`TimelineItemKey`] so they can survive
/// transactional content replacement when the rebuilt content still contains
/// the same key.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TimelineSelection {
    /// A normalized time range.
    TimeRange(TimelineTimeRange),
    /// A selected span by stable item key.
    Span(TimelineItemKey),
    /// A selected marker by stable item key.
    Marker(TimelineItemKey),
}

impl TimelineSelection {
    /// Creates a normalized time-range selection from two finite endpoints.
    pub fn time_range(a: f64, b: f64) -> TimelineResult<Self> {
        TimelineTimeRange::try_new(a, b).map(Self::TimeRange)
    }

    /// Returns the selected time range, if this is a time-range selection.
    #[must_use]
    pub const fn as_time_range(self) -> Option<TimelineTimeRange> {
        match self {
            Self::TimeRange(range) => Some(range),
            Self::Span(_) | Self::Marker(_) => None,
        }
    }
}
