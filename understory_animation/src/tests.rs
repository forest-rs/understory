// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;

use understory_animation_timeline::{AnimationTimeline, ManualTimeline, TimelineTime};

use crate::{
    AnimationTiming, CompositeOperation, FillMode, Keyframe, KeyframeEffect, PlaybackDirection,
    StackEffect, TargetStack, TimingPhase, sample_effects,
};

const MS: u64 = 1_000_000;

fn at_ms(value: u64) -> TimelineTime {
    TimelineTime::from_duration(value * MS)
}

#[test]
fn fill_modes_control_values_outside_active_interval() {
    let timing = AnimationTiming {
        delay: 20 * MS,
        duration: 100 * MS,
        fill: FillMode::Both,
        ..AnimationTiming::new(100 * MS)
    };

    let before = timing.sample(at_ms(10));
    assert_eq!(before.phase, TimingPhase::Before);
    assert_eq!(before.progress, Some(0.0));

    let after = timing.sample(at_ms(130));
    assert_eq!(after.phase, TimingPhase::After);
    assert_eq!(after.progress, Some(1.0));
}

#[test]
fn alternate_direction_flips_odd_iterations() {
    let timing = AnimationTiming {
        duration: 100 * MS,
        iterations: 3.0,
        direction: PlaybackDirection::Alternate,
        ..AnimationTiming::new(100 * MS)
    };

    assert_eq!(timing.sample(at_ms(25)).progress, Some(0.25));
    assert_eq!(timing.sample(at_ms(125)).progress, Some(0.75));
    assert_eq!(timing.sample(at_ms(225)).progress, Some(0.25));
}

#[test]
fn manual_timeline_seek_samples_effect_deterministically() {
    let mut timeline = ManualTimeline::at(at_ms(0));
    let effect = KeyframeEffect::from_values(vec![0.0_f64, 10.0]);
    let stack_effect = StackEffect::new(effect, AnimationTiming::new(100 * MS));
    let mut stack = TargetStack::new();
    stack.push(stack_effect);

    timeline.seek(at_ms(25));
    let first = stack.sample(&100.0, timeline.current_time(&()).unwrap());
    assert_eq!(first.value, 2.5);

    timeline.seek(at_ms(75));
    let second = stack.sample(&100.0, timeline.current_time(&()).unwrap());
    assert_eq!(second.value, 7.5);
}

#[test]
fn keyframes_are_sampled_in_offset_order() {
    let effect = KeyframeEffect::new(vec![Keyframe::new(1.0, 10.0_f64), Keyframe::new(0.0, 0.0)]);

    assert_eq!(effect.sample_at(0.25), Some(2.5));
    assert_eq!(effect.keyframes()[0].value, 0.0);
    assert_eq!(effect.keyframes()[1].value, 10.0);
}

#[test]
fn stack_effect_start_time_delays_sampling() {
    let effect = KeyframeEffect::from_values(vec![0.0_f64, 10.0]);
    let stack_effect =
        StackEffect::new(effect, AnimationTiming::new(100 * MS)).starting_at(at_ms(40));
    let mut stack = TargetStack::new();
    stack.push(stack_effect);

    let before = stack.sample(&100.0, at_ms(20));
    assert_eq!(before.value, 100.0);
    assert_eq!(before.active_effects, 0);

    let active = stack.sample(&100.0, at_ms(90));
    assert_eq!(active.value, 5.0);
    assert_eq!(active.active_effects, 1);
}

#[test]
fn later_replace_effect_wins() {
    let mut stack = TargetStack::new();
    stack.push(StackEffect::new(
        KeyframeEffect::from_values(vec![10.0_f64, 20.0]),
        AnimationTiming::new(100 * MS),
    ));
    stack.push(StackEffect::new(
        KeyframeEffect::from_values(vec![100.0_f64, 200.0]),
        AnimationTiming::new(100 * MS),
    ));

    let sample = stack.sample(&1.0, at_ms(50));

    assert_eq!(sample.value, 150.0);
    assert_eq!(sample.active_effects, 2);
    assert_eq!(sample.unsupported_composites, 0);
}

#[test]
fn borrowed_effect_reducer_matches_target_stack_sampling() {
    let first = StackEffect::new(
        KeyframeEffect::from_values(vec![10.0_f64, 20.0]),
        AnimationTiming::new(100 * MS),
    );
    let second = StackEffect::new(
        KeyframeEffect::from_values(vec![100.0_f64, 200.0]),
        AnimationTiming::new(100 * MS),
    );
    let mut stack = TargetStack::new();
    stack.push(first.clone());
    stack.push(second.clone());

    let borrowed = sample_effects(&1.0, [&first, &second], at_ms(50));
    let owned = stack.sample(&1.0, at_ms(50));

    assert_eq!(borrowed, owned);
}

#[test]
fn additive_and_accumulative_effects_fold_into_current_value() {
    let mut stack = TargetStack::new();
    stack.push(StackEffect::new(
        KeyframeEffect::from_values(vec![2.0_f64, 4.0]).with_composite(CompositeOperation::Add),
        AnimationTiming::new(100 * MS),
    ));
    stack.push(StackEffect::new(
        KeyframeEffect::from_values(vec![10.0_f64, 20.0])
            .with_composite(CompositeOperation::Accumulate),
        AnimationTiming::new(100 * MS),
    ));

    let sample = stack.sample(&1.0, at_ms(50));

    assert_eq!(sample.value, 1.0 + 3.0 + 15.0);
    assert_eq!(sample.active_effects, 2);
    assert_eq!(sample.unsupported_composites, 0);
}
