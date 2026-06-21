// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;

use understory_animation_timeline::{AnimationTimeline, ManualTimeline, TimelineTime};

use crate::{
    AnimationPlayState, AnimationPlayback, AnimationPlaybackEventKind, AnimationTiming,
    CompositeOperation, FillMode, Keyframe, KeyframeEffect, PlaybackDirection, StackEffect,
    TargetStack, TimingPhase, sample_effects,
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

#[test]
fn playback_maps_timeline_time_to_local_time() {
    let playback = AnimationPlayback::running_at(at_ms(10));

    assert_eq!(playback.state(), AnimationPlayState::Running);
    assert_eq!(playback.current_time(at_ms(10)), Some(at_ms(0)));
    assert_eq!(playback.current_time(at_ms(35)), Some(at_ms(25)));
}

#[test]
fn playback_pause_holds_and_play_resumes_from_hold_time() {
    let mut playback = AnimationPlayback::running_at(at_ms(10));

    playback.pause(at_ms(35));
    assert_eq!(playback.state(), AnimationPlayState::Paused);
    assert_eq!(playback.current_time(at_ms(80)), Some(at_ms(25)));

    assert_eq!(playback.play(at_ms(80)), None);
    assert_eq!(playback.current_time(at_ms(95)), Some(at_ms(40)));
}

#[test]
fn playback_seek_reanchors_without_forcing_running_state() {
    let mut playback = AnimationPlayback::new();

    playback.seek(at_ms(40), at_ms(10));

    assert_eq!(playback.state(), AnimationPlayState::Paused);
    assert_eq!(playback.current_time(at_ms(100)), Some(at_ms(40)));

    assert_eq!(playback.play(at_ms(100)), None);
    assert_eq!(playback.current_time(at_ms(125)), Some(at_ms(65)));
}

#[test]
fn playback_rate_and_reverse_preserve_local_continuity() {
    let mut playback = AnimationPlayback::running_at(at_ms(0));

    playback.set_playback_rate(2.0, at_ms(20));
    assert_eq!(playback.current_time(at_ms(30)), Some(at_ms(40)));

    playback.reverse(at_ms(30));
    assert_eq!(playback.playback_rate(), -2.0);
    assert_eq!(playback.current_time(at_ms(35)), Some(at_ms(30)));
}

#[test]
fn playback_fractional_rate_does_not_sample_ahead() {
    let mut playback = AnimationPlayback::running_at(at_ms(0));

    playback.set_playback_rate(0.5, at_ms(0));

    // Local time may lag by sub-nanosecond truncation, but it must not round
    // ahead of the timeline. Early samples can finish or fire cues too soon.
    assert_eq!(
        playback.current_time(TimelineTime::from_duration(3)),
        Some(TimelineTime::from_duration(1))
    );
}

#[test]
fn playback_completion_finishes_at_boundaries() {
    let mut playback = AnimationPlayback::running_at(at_ms(0));

    let (local_time, event) = playback
        .sample_with_completion(at_ms(150), AnimationTiming::new(100 * MS).total_duration());

    assert_eq!(local_time, Some(at_ms(100)));
    assert_eq!(event, Some(AnimationPlaybackEventKind::Finished));
    assert_eq!(playback.state(), AnimationPlayState::Finished);
    assert_eq!(playback.current_time(at_ms(300)), Some(at_ms(100)));
}

#[test]
fn playback_from_inactive_normalizes_non_positive_rate() {
    let mut playback = AnimationPlayback::new();

    playback.set_playback_rate(-2.0, at_ms(0));
    assert_eq!(playback.playback_rate(), -2.0);
    assert_eq!(
        playback.play(at_ms(10)),
        Some(AnimationPlaybackEventKind::Started)
    );
    assert_eq!(playback.playback_rate(), 2.0);
    assert_eq!(playback.current_time(at_ms(20)), Some(at_ms(20)));

    assert_eq!(
        playback.cancel(),
        Some(AnimationPlaybackEventKind::Canceled)
    );
    playback.set_playback_rate(0.0, at_ms(20));
    assert_eq!(
        playback.play(at_ms(30)),
        Some(AnimationPlaybackEventKind::Started)
    );
    assert_eq!(playback.playback_rate(), 1.0);
}

#[test]
fn playback_reverse_completion_finishes_at_zero_from_sought_endpoint() {
    let mut playback = AnimationPlayback::new();

    playback.seek(at_ms(100), at_ms(0));
    playback.reverse(at_ms(0));
    let (local_time, event) = playback
        .sample_with_completion(at_ms(150), AnimationTiming::new(100 * MS).total_duration());

    assert_eq!(playback.playback_rate(), -1.0);
    assert_eq!(local_time, Some(TimelineTime::ZERO));
    assert_eq!(event, Some(AnimationPlaybackEventKind::Finished));
    assert_eq!(playback.state(), AnimationPlayState::Finished);
}

#[test]
fn playback_cancel_returns_to_idle() {
    let mut playback = AnimationPlayback::running_at(at_ms(0));

    assert_eq!(
        playback.cancel(),
        Some(AnimationPlaybackEventKind::Canceled)
    );

    assert_eq!(playback.state(), AnimationPlayState::Idle);
    assert_eq!(playback.current_time(at_ms(40)), None);
    assert_eq!(playback.cancel(), None);
}

#[test]
fn playback_cancel_from_finished_returns_to_idle() {
    let mut playback = AnimationPlayback::running_at(at_ms(0));

    assert_eq!(
        playback.finish(at_ms(100)),
        Some(AnimationPlaybackEventKind::Finished)
    );
    assert_eq!(
        playback.cancel(),
        Some(AnimationPlaybackEventKind::Canceled)
    );

    assert_eq!(playback.state(), AnimationPlayState::Idle);
    assert_eq!(playback.current_time(at_ms(100)), None);
}
