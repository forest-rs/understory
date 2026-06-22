// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use understory_animation_timeline::TimelineTime;
use understory_timing::TimerDuration;

use crate::{AnimationPlayState, AnimationPlayback, TimingSample};

/// Timing metadata required by a retained animation instance.
///
/// This trait lets a host keep its own effect representation while reusing the
/// playback, completion, and iteration bookkeeping that every retained
/// animation instance needs.
///
/// The returned timing sample, iteration, and total duration must describe the
/// same local-time semantics used by the effect's actual value sampling. For
/// example, an effect with a delayed start should report no active iteration
/// before that start and include the delay in its finite total duration.
pub trait RetainedAnimationEffect {
    /// Returns the timing sample for `local_time`.
    fn timing_sample_at(&self, local_time: TimelineTime) -> Option<TimingSample>;

    /// Returns the iteration active at `local_time`, when the effect is
    /// producing one.
    fn iteration_at(&self, local_time: TimelineTime) -> Option<u64>;

    /// Returns the finite local duration at which playback naturally
    /// completes, or `None` for indefinite effects.
    fn total_duration(&self) -> Option<TimerDuration>;

    /// Returns the iteration baseline to record when playback naturally
    /// completes.
    ///
    /// The default asks for the iteration at [`RetainedAnimationEffect::total_duration`].
    /// Effects whose ordinary timing sample is empty at completion, such as
    /// non-forwards-filled keyframe effects, should override this with their
    /// last produced iteration.
    fn completion_iteration(&self) -> Option<u64> {
        self.total_duration()
            .and_then(|duration| self.iteration_at(TimelineTime::from_duration(duration)))
    }
}

/// Iteration progress produced by a retained animation instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnimationIterationChange {
    previous: Option<u64>,
    current: u64,
}

impl AnimationIterationChange {
    /// Creates an iteration change from a previous reported iteration and the
    /// current iteration.
    #[must_use]
    pub const fn new(previous: Option<u64>, current: u64) -> Self {
        Self { previous, current }
    }

    /// Returns the previously reported iteration, if any.
    #[must_use]
    pub const fn previous(self) -> Option<u64> {
        self.previous
    }

    /// Returns the current iteration.
    #[must_use]
    pub const fn current(self) -> u64 {
        self.current
    }

    /// Invokes `f` for each iteration boundary crossed by this change.
    ///
    /// Forward changes are reported in ascending order, excluding the previous
    /// iteration and including the current iteration. Reverse changes are
    /// reported in descending order, starting just below the previous iteration
    /// and ending at the current iteration. A missing previous iteration starts
    /// at iteration `1` and reports through the current iteration.
    pub fn for_each_crossed(self, mut f: impl FnMut(u64)) {
        match self.previous {
            Some(previous) if self.current > previous => {
                for iteration in previous.saturating_add(1)..=self.current {
                    f(iteration);
                }
            }
            Some(previous) if self.current < previous => {
                for iteration in (self.current..previous).rev() {
                    f(iteration);
                }
            }
            None if self.current > 0 => {
                for iteration in 1..=self.current {
                    f(iteration);
                }
            }
            Some(_) | None => {}
        }
    }
}

/// Generic retained animation instance.
///
/// The instance owns effect playback state, timeline identity, and iteration
/// bookkeeping. It deliberately does not own target keys, effect-stack ordering,
/// event queues, property storage, invalidation, or frame scheduling.
///
/// ```
/// use understory_animation::{
///     AnimationTiming, KeyframeEffect, RetainedAnimationInstance, StackEffect,
/// };
/// use understory_animation_timeline::TimelineTime;
///
/// const MS: u64 = 1_000_000;
///
/// let effect = StackEffect::new(
///     KeyframeEffect::from_values(vec![0.0_f64, 1.0]),
///     AnimationTiming::new(100 * MS),
/// );
/// let mut instance = RetainedAnimationInstance::new(
///     "fade",
///     "document",
///     effect,
///     TimelineTime::ZERO,
/// );
///
/// assert!(instance.is_running());
/// assert_eq!(
///     instance.current_time(TimelineTime::from_duration(25 * MS)),
///     Some(25 * MS),
/// );
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct RetainedAnimationInstance<Id, TimelineId, Effect> {
    id: Id,
    timeline: TimelineId,
    playback: AnimationPlayback,
    effect: Effect,
    last_reported_iteration: Option<u64>,
}

impl<Id, TimelineId, Effect> RetainedAnimationInstance<Id, TimelineId, Effect>
where
    Effect: RetainedAnimationEffect,
{
    /// Creates a running retained animation instance.
    #[must_use]
    pub fn new(id: Id, timeline: TimelineId, effect: Effect, started_at: TimelineTime) -> Self {
        Self {
            id,
            timeline,
            playback: AnimationPlayback::running_at(started_at),
            last_reported_iteration: effect.iteration_at(TimelineTime::ZERO),
            effect,
        }
    }

    /// Returns this instance's identity.
    #[must_use]
    pub const fn id(&self) -> &Id {
        &self.id
    }

    /// Returns this instance's timeline identity.
    #[must_use]
    pub const fn timeline(&self) -> &TimelineId {
        &self.timeline
    }

    /// Returns this instance's effect.
    #[must_use]
    pub const fn effect(&self) -> &Effect {
        &self.effect
    }

    /// Returns the current playback state.
    #[must_use]
    pub fn state(&self) -> AnimationPlayState {
        self.playback.state()
    }

    /// Returns whether this instance is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.playback.is_running()
    }

    /// Returns the current playback rate.
    #[must_use]
    pub fn playback_rate(&self) -> f64 {
        self.playback.playback_rate()
    }

    /// Returns the current local animation time at `timeline_time`.
    #[must_use]
    pub fn current_time(&self, timeline_time: TimelineTime) -> Option<TimerDuration> {
        self.local_time(timeline_time).map(TimelineTime::duration)
    }

    /// Returns the effect timing sample at `timeline_time`.
    #[must_use]
    pub fn timing_sample(&self, timeline_time: TimelineTime) -> Option<TimingSample> {
        self.local_time(timeline_time)
            .and_then(|time| self.effect.timing_sample_at(time))
    }

    /// Returns the local animation time at `timeline_time`.
    #[must_use]
    pub fn local_time(&self, timeline_time: TimelineTime) -> Option<TimelineTime> {
        self.playback.current_time(timeline_time)
    }

    /// Starts or resumes playback.
    ///
    /// Returns `true` when the call changed or resumed playback and `false`
    /// when playback was already running. A paused resume returns `true` even
    /// though it does not produce a new started event from the underlying
    /// playback controller.
    pub fn play(&mut self, timeline_time: TimelineTime) -> bool {
        if self.playback.is_running() {
            return false;
        }
        if self.playback.play(timeline_time).is_some() {
            self.last_reported_iteration = self.current_iteration(timeline_time);
        }
        true
    }

    /// Pauses playback.
    ///
    /// Returns `false` when playback was not running.
    pub fn pause(&mut self, timeline_time: TimelineTime) -> bool {
        if !self.playback.is_running() {
            return false;
        }
        self.playback.pause(timeline_time);
        true
    }

    /// Seeks to a local animation time.
    pub fn seek(&mut self, local_time: TimerDuration, timeline_time: TimelineTime) {
        self.playback
            .seek(TimelineTime::from_duration(local_time), timeline_time);
        self.last_reported_iteration = self.current_iteration(timeline_time);
    }

    /// Replaces the retained effect and resets iteration bookkeeping at
    /// `timeline_time`.
    ///
    /// The playback controller is preserved. Use this when a host retargets an
    /// animation instance without wanting stale iteration state from the
    /// previous effect to leak into the replacement.
    pub fn replace_effect(&mut self, effect: Effect, timeline_time: TimelineTime) -> Effect {
        let previous = core::mem::replace(&mut self.effect, effect);
        self.last_reported_iteration = self.current_iteration(timeline_time);
        previous
    }

    /// Reverses playback direction while preserving local continuity.
    ///
    /// Returns `false` when the instance is idle.
    pub fn reverse(&mut self, timeline_time: TimelineTime) -> bool {
        if self.playback.state() == AnimationPlayState::Idle {
            return false;
        }
        self.playback.reverse(timeline_time);
        true
    }

    /// Sets playback rate while preserving local continuity.
    ///
    /// Returns `false` when `playback_rate` is not finite or when the rate
    /// value was unchanged. An unchanged finite rate still reanchors playback
    /// at `timeline_time`, preserving local continuity for callers that use the
    /// call as an explicit rebase point.
    pub fn set_playback_rate(&mut self, playback_rate: f64, timeline_time: TimelineTime) -> bool {
        if !playback_rate.is_finite() {
            return false;
        }
        let changed = self.playback.playback_rate() != playback_rate;
        self.playback
            .set_playback_rate(playback_rate, timeline_time);
        changed
    }

    /// Finishes playback at this effect's finite total duration.
    ///
    /// Returns `false` for indefinite effects or instances that were already
    /// idle/finished.
    pub fn finish(&mut self) -> bool {
        let Some(duration) = self.effect.total_duration() else {
            return false;
        };
        let local_time = TimelineTime::from_duration(duration);
        let finished = self.playback.finish(local_time).is_some();
        if finished {
            self.last_reported_iteration = self.effect.completion_iteration();
        }
        finished
    }

    /// Finishes playback if sampling at `timeline_time` reaches a natural
    /// endpoint.
    pub fn finish_if_complete(&mut self, timeline_time: TimelineTime) -> bool {
        let (_, event) = self
            .playback
            .sample_with_completion(timeline_time, self.effect.total_duration());
        let finished = event == Some(crate::AnimationPlaybackEventKind::Finished);
        if finished {
            self.last_reported_iteration = self.effect.completion_iteration();
        }
        finished
    }

    /// Returns and records any crossed iteration boundary at `timeline_time`.
    ///
    /// This method is monotonic with respect to calls on this instance: it
    /// reports crossings since the previously reported iteration, then stores
    /// the current iteration as the new baseline.
    pub fn take_iteration_change(
        &mut self,
        timeline_time: TimelineTime,
    ) -> Option<AnimationIterationChange> {
        if !self.is_running() {
            return None;
        }
        let current = self.current_iteration(timeline_time)?;
        let previous = self.last_reported_iteration;
        self.last_reported_iteration = Some(current);
        let crossed = match previous {
            Some(previous) => previous != current,
            None => current > 0,
        };
        crossed.then_some(AnimationIterationChange::new(previous, current))
    }

    fn current_iteration(&self, timeline_time: TimelineTime) -> Option<u64> {
        self.local_time(timeline_time)
            .and_then(|time| self.effect.iteration_at(time))
    }
}
