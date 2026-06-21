// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use understory_animation_timeline::TimelineTime;
use understory_timing::TimerDuration;

/// Playback state for a retained animation instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnimationPlayState {
    /// The animation is not associated with active playback.
    Idle,
    /// The animation advances when its timeline advances.
    Running,
    /// The animation keeps its current time until resumed or explicitly
    /// sought.
    Paused,
    /// The animation has reached its natural end.
    Finished,
}

/// Event kind produced by playback state transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnimationPlaybackEventKind {
    /// Playback entered the running state from an inactive state.
    Started,
    /// Playback reached a natural or explicit endpoint.
    Finished,
    /// Playback was explicitly canceled and returned to idle.
    Canceled,
}

/// Retained playback controller for one animation instance.
///
/// The controller maps absolute timeline time into local animation time. It
/// does not own effects, target keys, event queues, property storage, or frame
/// scheduling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimationPlayback {
    state: AnimationPlayState,
    anchor_timeline_time: TimelineTime,
    anchor_local_time: TimelineTime,
    playback_rate: f64,
}

impl AnimationPlayback {
    /// Creates an idle playback controller at local time zero.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AnimationPlayState::Idle,
            anchor_timeline_time: TimelineTime::ZERO,
            anchor_local_time: TimelineTime::ZERO,
            playback_rate: 1.0,
        }
    }

    /// Creates a running playback controller starting at `timeline_time`.
    #[must_use]
    pub const fn running_at(timeline_time: TimelineTime) -> Self {
        Self {
            state: AnimationPlayState::Running,
            anchor_timeline_time: timeline_time,
            anchor_local_time: TimelineTime::ZERO,
            playback_rate: 1.0,
        }
    }

    /// Returns the current playback state.
    #[must_use]
    pub const fn state(self) -> AnimationPlayState {
        self.state
    }

    /// Returns the playback rate.
    #[must_use]
    pub const fn playback_rate(self) -> f64 {
        self.playback_rate
    }

    /// Returns whether this controller is running.
    #[must_use]
    pub const fn is_running(self) -> bool {
        matches!(self.state, AnimationPlayState::Running)
    }

    /// Returns the current local animation time at `timeline_time`.
    ///
    /// Idle animations produce no local time. Running playback treats timeline
    /// times before the current anchor as zero elapsed time; callers that need
    /// arbitrary scrubbing should use [`AnimationPlayback::seek`] to establish
    /// a new local-time anchor.
    #[must_use]
    pub fn current_time(self, timeline_time: TimelineTime) -> Option<TimelineTime> {
        match self.state {
            AnimationPlayState::Idle => None,
            AnimationPlayState::Paused | AnimationPlayState::Finished => {
                Some(self.anchor_local_time)
            }
            AnimationPlayState::Running => Some(self.running_time(timeline_time)),
        }
    }

    /// Starts or resumes playback at `timeline_time`.
    ///
    /// Starting from [`AnimationPlayState::Idle`] or
    /// [`AnimationPlayState::Finished`] begins at local time zero. If the
    /// stored playback rate is negative or zero, it is normalized to a positive
    /// finite rate so a fresh start does not immediately clamp at zero.
    /// To play backward from an endpoint, seek to that endpoint and then call
    /// [`AnimationPlayback::reverse`].
    pub fn play(&mut self, timeline_time: TimelineTime) -> Option<AnimationPlaybackEventKind> {
        match self.state {
            AnimationPlayState::Running => None,
            AnimationPlayState::Idle | AnimationPlayState::Finished => {
                self.anchor_timeline_time = timeline_time;
                self.anchor_local_time = TimelineTime::ZERO;
                self.playback_rate = inactive_start_rate(self.playback_rate);
                self.state = AnimationPlayState::Running;
                Some(AnimationPlaybackEventKind::Started)
            }
            AnimationPlayState::Paused => {
                self.anchor_timeline_time = timeline_time;
                self.state = AnimationPlayState::Running;
                None
            }
        }
    }

    /// Pauses playback, preserving the sampled local time at `timeline_time`.
    pub fn pause(&mut self, timeline_time: TimelineTime) {
        if self.state == AnimationPlayState::Running {
            self.anchor_local_time = self.running_time(timeline_time);
            self.anchor_timeline_time = timeline_time;
            self.state = AnimationPlayState::Paused;
        }
    }

    /// Seeks to `local_time`, preserving the current play state.
    ///
    /// Seeking an idle controller makes it paused at `local_time`, which gives
    /// deterministic manual sampling without implicitly starting playback.
    pub fn seek(&mut self, local_time: TimelineTime, timeline_time: TimelineTime) {
        self.anchor_local_time = local_time;
        self.anchor_timeline_time = timeline_time;
        if self.state == AnimationPlayState::Idle {
            self.state = AnimationPlayState::Paused;
        }
    }

    /// Reverses playback direction while preserving local continuity.
    ///
    /// A zero playback rate is treated as `-1.0` so reverse always produces
    /// motion when playback is running.
    pub fn reverse(&mut self, timeline_time: TimelineTime) {
        let local_time = self
            .current_time(timeline_time)
            .unwrap_or(TimelineTime::ZERO);
        self.anchor_local_time = local_time;
        self.anchor_timeline_time = timeline_time;
        self.playback_rate = if self.playback_rate == 0.0 {
            -1.0
        } else {
            -self.playback_rate
        };
        if self.state != AnimationPlayState::Idle {
            self.state = AnimationPlayState::Running;
        }
    }

    /// Sets the playback rate while preserving local continuity.
    ///
    /// Non-finite rates are ignored.
    pub fn set_playback_rate(&mut self, playback_rate: f64, timeline_time: TimelineTime) {
        if !playback_rate.is_finite() {
            return;
        }
        if let Some(local_time) = self.current_time(timeline_time) {
            self.anchor_local_time = local_time;
            self.anchor_timeline_time = timeline_time;
        }
        self.playback_rate = playback_rate;
    }

    /// Finishes playback at `local_time`.
    pub fn finish(&mut self, local_time: TimelineTime) -> Option<AnimationPlaybackEventKind> {
        if matches!(
            self.state,
            AnimationPlayState::Idle | AnimationPlayState::Finished
        ) {
            return None;
        }
        self.anchor_local_time = local_time;
        self.state = AnimationPlayState::Finished;
        Some(AnimationPlaybackEventKind::Finished)
    }

    /// Cancels playback and returns to the idle state.
    pub fn cancel(&mut self) -> Option<AnimationPlaybackEventKind> {
        if self.state == AnimationPlayState::Idle {
            return None;
        }
        self.state = AnimationPlayState::Idle;
        self.anchor_local_time = TimelineTime::ZERO;
        self.anchor_timeline_time = TimelineTime::ZERO;
        Some(AnimationPlaybackEventKind::Canceled)
    }

    /// Samples local time and finishes when `finite_duration` is reached.
    ///
    /// `finite_duration` is the finite local playback duration to clamp to,
    /// usually [`AnimationTiming::total_duration`](crate::AnimationTiming::total_duration).
    /// Pass `None` for indefinite effects.
    ///
    /// Returns any event produced by the sampling step.
    pub fn sample_with_completion(
        &mut self,
        timeline_time: TimelineTime,
        finite_duration: Option<TimerDuration>,
    ) -> (Option<TimelineTime>, Option<AnimationPlaybackEventKind>) {
        let Some(local_time) = self.current_time(timeline_time) else {
            return (None, None);
        };

        let Some(duration) = finite_duration else {
            return (Some(local_time), None);
        };

        if self.state != AnimationPlayState::Running {
            return (Some(local_time), None);
        }

        if self.playback_rate >= 0.0 && local_time.duration() >= duration {
            let local_time = TimelineTime::from_duration(duration);
            return (Some(local_time), self.finish(local_time));
        }

        if self.playback_rate < 0.0 && local_time == TimelineTime::ZERO {
            return (Some(TimelineTime::ZERO), self.finish(TimelineTime::ZERO));
        }

        (Some(local_time), None)
    }

    fn running_time(self, timeline_time: TimelineTime) -> TimelineTime {
        let elapsed = timeline_time.saturating_duration_since(self.anchor_timeline_time);
        let scaled = scale_duration(elapsed, self.playback_rate.abs());
        if self.playback_rate >= 0.0 {
            self.anchor_local_time.saturating_add(scaled)
        } else {
            TimelineTime::from_duration(self.anchor_local_time.duration().saturating_sub(scaled))
        }
    }
}

fn inactive_start_rate(playback_rate: f64) -> f64 {
    if playback_rate.is_finite() && playback_rate != 0.0 {
        playback_rate.abs()
    } else {
        1.0
    }
}

impl Default for AnimationPlayback {
    fn default() -> Self {
        Self::new()
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "finite non-negative playback durations saturate before conversion"
)]
fn scale_duration(duration: TimerDuration, scale: f64) -> TimerDuration {
    if !scale.is_finite() || scale <= 0.0 || duration == 0 {
        0
    } else {
        let scaled = duration as f64 * scale;
        if scaled >= TimerDuration::MAX as f64 {
            TimerDuration::MAX
        } else {
            scaled as TimerDuration
        }
    }
}

/// Direction used to map iteration progress into effect progress.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackDirection {
    /// Every iteration plays from start to end.
    Normal,
    /// Every iteration plays from end to start.
    Reverse,
    /// Even iterations play forward; odd iterations play backward.
    Alternate,
    /// Even iterations play backward; odd iterations play forward.
    AlternateReverse,
}

/// Fill behavior outside an animation's active interval.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FillMode {
    /// The animation produces no value before or after its active interval.
    None,
    /// The animation produces its first active value before it starts.
    Backwards,
    /// The animation produces its final active value after it ends.
    Forwards,
    /// The animation fills both before and after its active interval.
    Both,
}

impl FillMode {
    pub(crate) fn fills_backwards(self) -> bool {
        matches!(self, Self::Backwards | Self::Both)
    }

    pub(crate) fn fills_forwards(self) -> bool {
        matches!(self, Self::Forwards | Self::Both)
    }
}
