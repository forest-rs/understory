// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::any::TypeId;

use peniko::Color;
use understory_property::{
    ErasedValue, Property, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
};
use understory_timing::TimerDuration;

use crate::{
    ColorInterpolation, NoTransitionResources, TargetOutcome, TargetUpdate, TransitionDeclaration,
    TransitionDeclarations, TransitionPhase, TransitionResourceKey, TransitionResources,
    TransitionRunner, TransitionTiming, TransitionWrite, TransitionWriteOp,
};

const OWNER: u32 = 1;

fn setup() -> (PropertyRegistry, Property<f64>) {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
    (registry, width)
}

fn transitions_of(width: Property<f64>, timing: TransitionTiming) -> TransitionDeclarations {
    let mut declarations = TransitionDeclarations::new();
    declarations.declare(width, timing);
    declarations
}

fn ev(value: f64) -> ErasedValue {
    ErasedValue::new(value)
}

fn ignore_update<K, G>(update: TargetUpdate<K, G>) {
    let _ = update.into_parts();
}

/// Extracts the single `Set` value from a one-write sample.
fn single_set<G: Copy>(writes: &[TransitionWrite<u32, G>]) -> f64 {
    assert_eq!(writes.len(), 1, "expected exactly one write");
    match writes[0].op() {
        TransitionWriteOp::Set(value) => *value.downcast_ref::<f64>().unwrap(),
        TransitionWriteOp::Clear => panic!("expected a Set write, got Clear"),
    }
}

fn update_set(update: &TargetUpdate<u32>) -> f64 {
    let write = update.immediate_write().expect("expected immediate write");
    match write.op() {
        TransitionWriteOp::Set(value) => *value.downcast_ref::<f64>().unwrap(),
        TransitionWriteOp::Clear => panic!("expected a Set write, got Clear"),
    }
}

fn update_clear<G: Copy>(update: &TargetUpdate<u32, G>) {
    let write = update.immediate_write().expect("expected immediate write");
    assert!(
        matches!(write.op(), TransitionWriteOp::Clear),
        "expected a Clear write"
    );
}

fn single_clear<G: Copy>(writes: &[TransitionWrite<u32, G>]) {
    assert_eq!(writes.len(), 1, "expected exactly one write");
    assert!(
        matches!(writes[0].op(), TransitionWriteOp::Clear),
        "expected a Clear write"
    );
}

#[test]
fn transition_samples_toward_target_and_clears_on_completion() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100));
    let mut runner = TransitionRunner::new();

    let outcome = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();
    assert_eq!(outcome.outcome(), TargetOutcome::Started);
    assert!(runner.has_transition(OWNER, width.id()));
    assert_eq!(runner.next_end_time(), Some(100));

    assert_eq!(update_set(&outcome), 0.0);
    assert!(runner.sample(0).is_empty());
    assert_eq!(single_set(&runner.sample(50)), 5.0);

    single_clear(&runner.sample(100));
    assert!(runner.is_idle());
    assert!(runner.sample(150).is_empty());
}

#[test]
fn retarget_mid_flight_starts_from_current_value() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100));
    let mut runner = TransitionRunner::new();

    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();

    // Halfway to 10.0 the target changes to 30.0 (not a reversal).
    let outcome = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(10.0),
            ev(30.0),
            50,
        )
        .unwrap();
    assert_eq!(outcome.outcome(), TargetOutcome::Retargeted);

    // The replacement continues from the animated value 5.0, not the base.
    assert_eq!(update_set(&outcome), 5.0);
    assert!(runner.sample(50).is_empty());
    assert_eq!(
        runner.target(OWNER, width.id()).unwrap().downcast_ref(),
        Some(&30.0)
    );
    // Non-reversing retargets keep the full declared duration.
    assert_eq!(runner.next_end_time(), Some(150));
    assert_eq!(single_set(&runner.sample(100)), 17.5);
    single_clear(&runner.sample(150));
}

#[test]
fn retarget_to_same_target_is_noop() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100));
    let mut runner = TransitionRunner::new();

    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();
    let outcome = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            50,
        )
        .unwrap();

    assert_eq!(outcome.outcome(), TargetOutcome::Unchanged);
    // The original timing keeps running; nothing restarted.
    assert_eq!(single_set(&runner.sample(50)), 5.0);
    assert_eq!(runner.next_end_time(), Some(100));
}

#[test]
fn idle_equal_values_are_unchanged() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100));
    let mut runner = TransitionRunner::new();

    let outcome = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(10.0),
            ev(10.0),
            0,
        )
        .unwrap();

    assert_eq!(outcome.outcome(), TargetOutcome::Unchanged);
    assert!(runner.is_idle());
}

#[test]
fn reversal_shortens_duration_by_travelled_fraction() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100));
    let mut runner = TransitionRunner::new();

    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();

    // Hover-off at 50%: reverse back to 0.0.
    let outcome = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(10.0),
            ev(0.0),
            50,
        )
        .unwrap();
    assert_eq!(outcome.outcome(), TargetOutcome::Retargeted);

    // Shortening factor 0.5: the way back takes 50 ticks, not 100.
    assert_eq!(runner.next_end_time(), Some(100));
    assert_eq!(update_set(&outcome), 5.0);
    assert!(runner.sample(50).is_empty());
    assert_eq!(single_set(&runner.sample(75)), 2.5);
    single_clear(&runner.sample(100));
}

#[test]
fn double_reversal_composes_shortening_factor() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100));
    let mut runner = TransitionRunner::new();

    // 0 -> 10, interrupted at 50% and reversed (factor 0.5, duration 50).
    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();
    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(10.0),
            ev(0.0),
            50,
        )
        .unwrap();

    // Reversed again halfway back (current value 2.5): the factor becomes
    // |0.5 * 0.5 + (1 - 0.5)| = 0.75, so the third leg takes 75 ticks.
    let outcome = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            75,
        )
        .unwrap();
    assert_eq!(outcome.outcome(), TargetOutcome::Retargeted);
    assert_eq!(runner.next_end_time(), Some(150));

    assert_eq!(update_set(&outcome), 2.5);
    assert!(runner.sample(75).is_empty());
    let value = runner
        .current_value(OWNER, width.id(), 105)
        .unwrap()
        .downcast_ref::<f64>()
        .copied()
        .unwrap();
    // Progress 30/75 = 0.4 from 2.5 toward 10.0.
    assert!((value - 5.5).abs() < 1e-9, "value was {value}");
    single_clear(&runner.sample(150));
}

#[test]
fn delay_holds_the_from_value() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100).with_delay(50));
    let mut runner = TransitionRunner::new();

    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();

    // During the delay the old value keeps masking the flipped base.
    assert_eq!(single_set(&runner.sample(20)), 0.0);
    assert_eq!(single_set(&runner.sample(50)), 0.0);
    // Interpolation runs after the delay.
    assert_eq!(single_set(&runner.sample(100)), 5.0);
    assert_eq!(runner.next_end_time(), Some(150));
    single_clear(&runner.sample(150));
}

#[test]
fn reversing_during_delay_snaps_back() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100).with_delay(50));
    let mut runner = TransitionRunner::new();

    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();

    // The value has not moved yet, so returning to 0.0 is immediate.
    let outcome = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(10.0),
            ev(0.0),
            20,
        )
        .unwrap();
    assert_eq!(outcome.outcome(), TargetOutcome::Snapped);
    assert!(!runner.has_transition(OWNER, width.id()));
    update_clear(&outcome);
    assert!(runner.sample(20).is_empty());
}

#[test]
fn retarget_during_delay_starts_from_the_held_value() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100).with_delay(50));
    let mut runner = TransitionRunner::new();

    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();
    let outcome = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(10.0),
            ev(20.0),
            20,
        )
        .unwrap();

    assert_eq!(outcome.outcome(), TargetOutcome::Retargeted);
    // The delay restarts; the held value stays 0.0 until 20 + 50.
    assert_eq!(single_set(&runner.sample(60)), 0.0);
    assert_eq!(single_set(&runner.sample(120)), 10.0);
    single_clear(&runner.sample(170));
}

#[test]
fn zero_combined_duration_applies_immediately() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(0));
    let mut runner = TransitionRunner::new();

    let outcome = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();

    assert_eq!(outcome.outcome(), TargetOutcome::Snapped);
    assert!(runner.is_idle());
    assert!(runner.sample(0).is_empty());
}

#[test]
fn zero_duration_with_delay_holds_then_snaps() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(0).with_delay(50));
    let mut runner = TransitionRunner::new();

    let outcome = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();
    assert_eq!(outcome.outcome(), TargetOutcome::Started);

    assert_eq!(single_set(&runner.sample(25)), 0.0);
    single_clear(&runner.sample(50));
    assert!(runner.is_idle());
}

#[test]
fn undeclared_target_snaps() {
    let (_, width) = setup();
    let transitions = TransitionDeclarations::new();
    let mut runner = TransitionRunner::new();

    let outcome = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();

    assert_eq!(outcome.outcome(), TargetOutcome::Snapped);
    assert!(runner.is_idle());
}

#[test]
fn declaration_removed_mid_flight_snaps_on_retarget() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100));
    let mut runner = TransitionRunner::new();

    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();

    let outcome = runner
        .set_target(
            OWNER,
            width.id(),
            &TransitionDeclarations::new(),
            &NoTransitionResources,
            &ev(10.0),
            ev(20.0),
            50,
        )
        .unwrap();

    assert_eq!(outcome.outcome(), TargetOutcome::Snapped);
    update_clear(&outcome);
    assert!(runner.sample(50).is_empty());
}

#[test]
fn retarget_to_current_animated_value_cancels() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100));
    let mut runner = TransitionRunner::new();

    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();

    // The new target equals the value currently on screen (5.0 at t=50).
    let outcome = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(10.0),
            ev(5.0),
            50,
        )
        .unwrap();

    assert_eq!(outcome.outcome(), TargetOutcome::Snapped);
    assert!(!runner.has_transition(OWNER, width.id()));
    update_clear(&outcome);
    assert!(runner.sample(50).is_empty());
}

#[test]
fn type_mismatch_is_rejected_and_leaves_runner_unchanged() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100));
    let mut runner = TransitionRunner::new();

    // Idle: mismatched target.
    let err = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ErasedValue::new(10_i32),
            0,
        )
        .unwrap_err();
    assert_eq!(err.expected, TypeId::of::<f64>());
    assert_eq!(err.actual, TypeId::of::<i32>());
    assert!(runner.is_idle());

    // Idle: mismatched before value.
    let err = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ErasedValue::new(0_i32),
            ev(10.0),
            0,
        )
        .unwrap_err();
    assert_eq!(err.actual, TypeId::of::<i32>());
    assert!(runner.is_idle());

    // Mid-flight: mismatched retarget leaves the transition running.
    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();
    let err = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(10.0),
            ErasedValue::new(20_i32),
            50,
        )
        .unwrap_err();
    assert_eq!(err.actual, TypeId::of::<i32>());
    assert_eq!(single_set(&runner.sample(50)), 5.0);
}

#[test]
fn cancel_returns_a_clear() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100));
    let mut runner = TransitionRunner::new();

    ignore_update(
        runner
            .set_target(
                OWNER,
                width.id(),
                &transitions,
                &NoTransitionResources,
                &ev(0.0),
                ev(10.0),
                0,
            )
            .unwrap(),
    );

    let clear = runner.cancel(OWNER, width.id()).expect("expected clear");
    assert!(runner.cancel(OWNER, width.id()).is_none());
    assert!(!runner.has_transition(OWNER, width.id()));
    single_clear(&[clear]);
    assert!(runner.sample(10).is_empty());
    assert!(runner.is_idle());
}

#[test]
fn cancel_owner_only_touches_that_owner() {
    let (mut registry, width) = setup();
    let height = registry.register("Height", PropertyMetadataBuilder::new(0.0_f64).build());
    let mut transitions = transitions_of(width, TransitionTiming::new(100));
    transitions.declare(height, TransitionTiming::new(100));
    let mut runner = TransitionRunner::new();

    ignore_update(
        runner
            .set_target(
                1_u32,
                width.id(),
                &transitions,
                &NoTransitionResources,
                &ev(0.0),
                ev(10.0),
                0,
            )
            .unwrap(),
    );
    ignore_update(
        runner
            .set_target(
                1_u32,
                height.id(),
                &transitions,
                &NoTransitionResources,
                &ev(0.0),
                ev(10.0),
                0,
            )
            .unwrap(),
    );
    ignore_update(
        runner
            .set_target(
                2_u32,
                width.id(),
                &transitions,
                &NoTransitionResources,
                &ev(0.0),
                ev(10.0),
                0,
            )
            .unwrap(),
    );

    let clears = runner.cancel_owner(1);
    assert_eq!(clears.len(), 2);
    assert!(clears.iter().all(|write| write.owner() == 1));
    assert!(
        clears
            .iter()
            .all(|write| matches!(write.op(), TransitionWriteOp::Clear))
    );
    assert_eq!(runner.active_count(), 1);
    assert!(runner.has_transition(2, width.id()));

    let writes = runner.sample(50);
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].owner(), 2);
}

#[test]
fn discard_owner_drops_state_without_writes() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100));
    let mut runner = TransitionRunner::new();

    ignore_update(
        runner
            .set_target(
                1_u32,
                width.id(),
                &transitions,
                &NoTransitionResources,
                &ev(0.0),
                ev(10.0),
                0,
            )
            .unwrap(),
    );
    ignore_update(
        runner
            .set_target(
                2_u32,
                width.id(),
                &transitions,
                &NoTransitionResources,
                &ev(0.0),
                ev(10.0),
                0,
            )
            .unwrap(),
    );

    assert_eq!(runner.discard_owner(1), 1);

    let writes = runner.sample(50);
    assert!(writes.iter().all(|write| write.owner() == 2));
    assert_eq!(writes.len(), 1);
}

#[test]
fn writes_apply_to_the_property_store_animation_slot() {
    let (registry, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100));
    let mut runner = TransitionRunner::new();
    let mut store = PropertyStore::<u32>::new(OWNER);

    // The style flip already updated the base value.
    store.set_local(width, 10.0);
    let update = runner
        .set_target(
            store.owner(),
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();
    update.into_immediate_write().unwrap().apply(&mut store);
    assert!(store.has_animation(width));
    assert_eq!(store.get_effective_local(width, &registry), 0.0);
    assert!(runner.sample(0).is_empty());

    for write in runner.sample(50) {
        write.apply(&mut store);
    }
    assert!(store.has_animation(width));
    assert_eq!(store.get_effective_local(width, &registry), 5.0);

    for write in runner.sample(100) {
        write.apply(&mut store);
    }
    assert!(!store.has_animation(width));
    assert_eq!(store.get_effective_local(width, &registry), 10.0);
}

#[test]
fn next_end_time_reports_earliest_completion() {
    let (mut registry, width) = setup();
    let height = registry.register("Height", PropertyMetadataBuilder::new(0.0_f64).build());
    let mut transitions = transitions_of(width, TransitionTiming::new(100));
    transitions.declare(height, TransitionTiming::new(30).with_delay(10));
    let mut runner = TransitionRunner::new();

    assert_eq!(runner.next_end_time(), None);

    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();
    let _ = runner
        .set_target(
            OWNER,
            height.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();

    assert_eq!(runner.next_end_time(), Some(40));
}

// -----------------------------------------------------------------------------
// Late-bound resources
// -----------------------------------------------------------------------------

const DURATION_TOKEN: TransitionResourceKey = TransitionResourceKey::new(1);

/// A mutable stand-in for a host theme.
struct MotionTheme {
    duration: Option<TimerDuration>,
}

impl TransitionResources for MotionTheme {
    fn duration(&self, key: TransitionResourceKey) -> Option<TimerDuration> {
        (key == DURATION_TOKEN).then_some(self.duration?)
    }
}

#[test]
fn late_bound_duration_resolves_at_transition_start() {
    let (_, width) = setup();
    let declarations = TransitionDeclarations::new().with(
        TransitionDeclaration::of(width, TransitionTiming::new(100))
            .with_duration_resource(DURATION_TOKEN),
    );
    let mut runner = TransitionRunner::new();

    let theme = MotionTheme {
        duration: Some(200),
    };
    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &declarations,
            &theme,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();
    // The literal 100 was overridden by the resource at start.
    assert_eq!(runner.next_end_time(), Some(200));
    assert_eq!(single_set(&runner.sample(100)), 5.0);

    // A theme swap retunes the next transition without touching the
    // declaration set, but never an in-flight one.
    let swapped = MotionTheme {
        duration: Some(400),
    };
    assert_eq!(runner.next_end_time(), Some(200));
    single_clear(&runner.sample(200));

    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &declarations,
            &swapped,
            &ev(10.0),
            ev(0.0),
            1_000,
        )
        .unwrap();
    assert_eq!(runner.next_end_time(), Some(1_400));
}

#[test]
fn late_bound_resources_accept_trait_objects() {
    let (_, width) = setup();
    let declarations = TransitionDeclarations::new().with(
        TransitionDeclaration::of(width, TransitionTiming::new(100))
            .with_duration_resource(DURATION_TOKEN),
    );
    let mut runner = TransitionRunner::new();
    let theme = MotionTheme {
        duration: Some(150),
    };
    let resources: &dyn TransitionResources = &theme;

    assert_eq!(
        declarations
            .find(&width.id())
            .unwrap()
            .resolve_timing(resources)
            .duration(),
        150
    );
    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &declarations,
            resources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();

    assert_eq!(runner.next_end_time(), Some(150));
}

#[test]
fn missing_resource_falls_back_to_zero_duration_and_snaps() {
    let (_, width) = setup();
    let declarations = TransitionDeclarations::new().with(
        TransitionDeclaration::of(width, TransitionTiming::new(100))
            .with_duration_resource(DURATION_TOKEN),
    );
    let mut runner = TransitionRunner::new();

    let empty_theme = MotionTheme { duration: None };
    let outcome = runner
        .set_target(
            OWNER,
            width.id(),
            &declarations,
            &empty_theme,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();

    assert_eq!(outcome.outcome(), TargetOutcome::Snapped);
    assert!(runner.is_idle());
}

// -----------------------------------------------------------------------------
// Generic target keys
// -----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum VisualKey {
    OffsetX,
}

#[test]
fn generic_target_keys_run_transitions() {
    let declarations = TransitionDeclarations::new().with(
        TransitionDeclaration::for_target::<f64>(VisualKey::OffsetX, TransitionTiming::new(100)),
    );
    let mut runner: TransitionRunner<u32, VisualKey> = TransitionRunner::new();

    let _ = runner
        .set_target(
            OWNER,
            VisualKey::OffsetX,
            &declarations,
            &NoTransitionResources,
            &ev(0.0),
            ev(8.0),
            0,
        )
        .unwrap();

    let writes = runner.sample(50);
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].target(), VisualKey::OffsetX);
    match writes[0].op() {
        TransitionWriteOp::Set(value) => assert_eq!(value.downcast_ref::<f64>(), Some(&4.0)),
        TransitionWriteOp::Clear => panic!("expected a Set write"),
    }
    single_clear(&runner.sample(100));
}

// -----------------------------------------------------------------------------
// Color transitions
// -----------------------------------------------------------------------------

#[test]
fn color_transitions_use_the_declared_interpolation() {
    let mut registry = PropertyRegistry::new();
    let fill = registry.register(
        "Fill",
        PropertyMetadataBuilder::new(Color::TRANSPARENT).build(),
    );
    let declarations = TransitionDeclarations::new().with(
        TransitionDeclaration::of_color(fill, TransitionTiming::new(100))
            .with_color_interpolation(ColorInterpolation::OKLAB),
    );
    let mut runner = TransitionRunner::new();

    let _ = runner
        .set_target(
            OWNER,
            fill.id(),
            &declarations,
            &NoTransitionResources,
            &ErasedValue::new(Color::BLACK),
            ErasedValue::new(Color::WHITE),
            0,
        )
        .unwrap();

    let mid = runner.current_value(OWNER, fill.id(), 50).unwrap();
    let expected = ColorInterpolation::OKLAB.interpolate(Color::BLACK, Color::WHITE, 0.5);
    assert_eq!(mid.downcast_ref::<Color>(), Some(&expected));
    single_clear(&runner.sample(100));
}

// -----------------------------------------------------------------------------
// Inspection snapshots
// -----------------------------------------------------------------------------

#[test]
fn active_snapshots_report_phase_progress_and_shortening() {
    let (_, width) = setup();
    let transitions = transitions_of(width, TransitionTiming::new(100).with_delay(50));
    let mut runner = TransitionRunner::new();

    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(0.0),
            ev(10.0),
            0,
        )
        .unwrap();

    // During the delay.
    let info: Vec<_> = runner.active(20).collect();
    assert_eq!(info.len(), 1);
    assert_eq!(info[0].phase(), TransitionPhase::Delaying);
    assert_eq!(info[0].progress(), 0.0);
    assert_eq!(info[0].current().downcast_ref::<f64>(), Some(&0.0));
    assert_eq!(info[0].reversing_shortening(), 1.0);
    assert_eq!(info[0].start(), 50);
    assert_eq!(info[0].duration(), 100);
    assert_eq!(info[0].end_time(), 150);

    // Mid-run.
    let info: Vec<_> = runner.active(100).collect();
    assert_eq!(info[0].phase(), TransitionPhase::Running);
    assert_eq!(info[0].progress(), 0.5);
    assert_eq!(info[0].from().downcast_ref::<f64>(), Some(&0.0));
    assert_eq!(info[0].to().downcast_ref::<f64>(), Some(&10.0));
    assert_eq!(info[0].current().downcast_ref::<f64>(), Some(&5.0));
    assert_eq!(info[0].owner(), OWNER);
    assert_eq!(info[0].target(), width.id());

    // Reverse at 50% of the run: the replacement reports Reversing with a
    // shortened duration.
    let _ = runner
        .set_target(
            OWNER,
            width.id(),
            &transitions,
            &NoTransitionResources,
            &ev(10.0),
            ev(0.0),
            100,
        )
        .unwrap();
    let info: Vec<_> = runner.active(160).collect();
    assert_eq!(info[0].phase(), TransitionPhase::Reversing);
    assert_eq!(info[0].reversing_shortening(), 0.5);
    assert_eq!(info[0].duration(), 50);
    assert_eq!(info[0].start(), 150);
    assert_eq!(info[0].end_time(), 200);
}
