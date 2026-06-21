// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::TimelineTime;

/// Source of current animation timeline time.
pub trait AnimationTimeline<Context = ()> {
    /// Returns the current timeline time, or `None` when the timeline is
    /// inactive for `context`.
    fn current_time(&self, context: &Context) -> Option<TimelineTime>;
}
