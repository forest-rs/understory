// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{AnimationTimeline, TimelineTime};

/// Seekable timeline for deterministic tests, devtools, and app-controlled
/// playback.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ManualTimeline {
    time: Option<TimelineTime>,
}

impl ManualTimeline {
    /// Creates an inactive manual timeline.
    #[must_use]
    pub const fn new() -> Self {
        Self { time: None }
    }

    /// Creates a manual timeline at `time`.
    #[must_use]
    pub const fn at(time: TimelineTime) -> Self {
        Self { time: Some(time) }
    }

    /// Seeks the timeline to `time`.
    pub fn seek(&mut self, time: TimelineTime) {
        self.time = Some(time);
    }

    /// Clears the current timeline time, making the timeline inactive.
    pub fn clear(&mut self) {
        self.time = None;
    }

    /// Returns the currently stored time.
    #[must_use]
    pub const fn time(self) -> Option<TimelineTime> {
        self.time
    }
}

impl AnimationTimeline for ManualTimeline {
    fn current_time(&self, _context: &()) -> Option<TimelineTime> {
        self.time
    }
}
