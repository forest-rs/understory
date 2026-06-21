// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use understory_animation_timeline::TimelineTime;
use understory_motion::AnimatableValue;

use crate::{AnimationTiming, CompositeOperation, KeyframeEffect};

/// One effect plus its timing inside a target stack.
#[derive(Clone, Debug, PartialEq)]
pub struct StackEffect<T> {
    /// Sampled value effect.
    pub effect: KeyframeEffect<T>,
    /// Local timing for the effect.
    pub timing: AnimationTiming,
}

impl<T> StackEffect<T> {
    /// Creates a stack effect.
    #[must_use]
    pub const fn new(effect: KeyframeEffect<T>, timing: AnimationTiming) -> Self {
        Self { effect, timing }
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
        let mut value = underlying.clone();
        let mut active_effects = 0;
        let mut unsupported_composites = 0;

        for stack_effect in &self.effects {
            let timing_sample = stack_effect.timing.sample(time);
            let Some(progress) = timing_sample.eased_progress else {
                continue;
            };
            let Some(sampled) = stack_effect.effect.sample_at(progress) else {
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
