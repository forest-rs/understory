// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use understory_animation_timeline::TimelineTime;
use understory_motion::AnimatableValue;
use understory_timing::TimerDuration;

use crate::{
    AnimationTiming, CompositeOperation, KeyframeEffect, RetainedAnimationEffect, TimingSample,
};

/// One effect plus its timing inside a target stack.
#[derive(Clone, Debug, PartialEq)]
pub struct StackEffect<T> {
    /// Sampled value effect.
    pub effect: KeyframeEffect<T>,
    /// Local timing for the effect.
    pub timing: AnimationTiming,
    /// Timeline time when this effect starts.
    pub starts_at: TimelineTime,
}

impl<T> StackEffect<T> {
    /// Creates a stack effect.
    #[must_use]
    pub const fn new(effect: KeyframeEffect<T>, timing: AnimationTiming) -> Self {
        Self {
            effect,
            timing,
            starts_at: TimelineTime::ZERO,
        }
    }

    /// Sets the timeline time when this effect starts.
    #[must_use]
    pub const fn starting_at(mut self, starts_at: TimelineTime) -> Self {
        self.starts_at = starts_at;
        self
    }

    /// Returns the finite timeline time when this effect is complete.
    ///
    /// Returns `None` for indefinitely repeating effects.
    #[must_use]
    pub fn end_time(&self) -> Option<TimelineTime> {
        self.timing
            .total_duration()
            .map(|duration| self.starts_at.saturating_add(duration))
    }
}

impl<T: AnimatableValue> StackEffect<T> {
    /// Samples this effect at absolute `time`.
    #[must_use]
    pub fn sample(&self, time: TimelineTime) -> Option<T> {
        if time < self.starts_at {
            return None;
        }
        let local = TimelineTime::from_duration(time.saturating_duration_since(self.starts_at));
        let timing_sample = self.timing.sample(local);
        let progress = timing_sample.eased_progress?;
        self.effect.sample_at(progress)
    }
}

impl<T> RetainedAnimationEffect for StackEffect<T> {
    fn timing_sample_at(&self, local_time: TimelineTime) -> Option<TimingSample> {
        if local_time < self.starts_at {
            return None;
        }
        let local =
            TimelineTime::from_duration(local_time.saturating_duration_since(self.starts_at));
        Some(self.timing.sample(local))
    }

    fn iteration_at(&self, local_time: TimelineTime) -> Option<u64> {
        self.timing_sample_at(local_time)?.iteration
    }

    fn total_duration(&self) -> Option<TimerDuration> {
        self.end_time().map(TimelineTime::duration)
    }

    fn completion_iteration(&self) -> Option<u64> {
        self.timing.completion_iteration()
    }
}

/// Samples and composites borrowed stack effects over `underlying`.
#[must_use]
pub fn sample_effects<'a, T, I>(underlying: &T, effects: I, time: TimelineTime) -> TargetSample<T>
where
    T: AnimatableValue + 'a,
    I: IntoIterator<Item = &'a StackEffect<T>>,
{
    let mut value = underlying.clone();
    let mut active_effects = 0;
    let mut unsupported_composites = 0;

    for stack_effect in effects {
        let Some(sampled) = stack_effect.sample(time) else {
            continue;
        };

        active_effects += 1;
        match stack_effect.effect.composite() {
            CompositeOperation::Replace => {
                value = sampled;
            }
            CompositeOperation::Add => {
                if let Some(composited) = value.add(&sampled) {
                    value = composited;
                } else {
                    unsupported_composites += 1;
                }
            }
            CompositeOperation::Accumulate => {
                if let Some(composited) = value.accumulate(&sampled, 1) {
                    value = composited;
                } else {
                    unsupported_composites += 1;
                }
            }
        }
    }

    TargetSample {
        value,
        active_effects,
        unsupported_composites,
    }
}

/// Ordered typed effects for one animation target.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TargetStack<T> {
    effects: Vec<StackEffect<T>>,
}

impl<T> TargetStack<T> {
    /// Creates an empty target stack.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            effects: Vec::new(),
        }
    }

    /// Pushes an effect above the current stack contents.
    pub fn push(&mut self, effect: StackEffect<T>) {
        self.effects.push(effect);
    }

    /// Returns the ordered stack effects.
    #[must_use]
    pub fn effects(&self) -> &[StackEffect<T>] {
        &self.effects
    }
}

impl<T: AnimatableValue> TargetStack<T> {
    /// Samples and composites the stack over `underlying` at `time`.
    #[must_use]
    pub fn sample(&self, underlying: &T, time: TimelineTime) -> TargetSample<T> {
        sample_effects(underlying, &self.effects, time)
    }
}

/// Final sampled value from a target stack.
#[derive(Clone, Debug, PartialEq)]
pub struct TargetSample<T> {
    /// Composited final value.
    pub value: T,
    /// Number of effects that produced a sample.
    pub active_effects: usize,
    /// Number of additive/accumulative effects skipped because the value type
    /// did not support the requested operation.
    pub unsupported_composites: usize,
}
