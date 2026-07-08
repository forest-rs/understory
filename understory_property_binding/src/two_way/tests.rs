// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use invalidation::{Channel, ChannelSet};
use understory_property::{
    ErasedValue, Property, PropertyId, PropertyMetadataBuilder, PropertyRegistry,
};

use super::{ReverseRejectionReason, TwoWayBindingId, TwoWayBindings};
use crate::{BindingError, BindingHost, BindingSet, BindingWrite, EndpointKey, PropertyEndpoint};

const BINDING: Channel = Channel::new(0);
const LAYOUT: Channel = Channel::new(1);
const PAINT: Channel = Channel::new(2);

#[derive(Default)]
struct TestHost {
    values: BTreeMap<EndpointKey<u32>, ErasedValue>,
    channels: BTreeMap<PropertyId, ChannelSet>,
    writes: Vec<EndpointKey<u32>>,
}

impl TestHost {
    fn set_initial<T: Clone + 'static>(&mut self, endpoint: PropertyEndpoint<u32, T>, value: T) {
        self.values.insert(endpoint.key(), ErasedValue::new(value));
    }

    fn set_channels<T>(&mut self, endpoint: PropertyEndpoint<u32, T>, channels: ChannelSet) {
        self.channels.insert(endpoint.property().id(), channels);
    }

    fn value<T: 'static>(&self, endpoint: PropertyEndpoint<u32, T>) -> Option<&T> {
        self.values
            .get(&endpoint.key())
            .and_then(ErasedValue::downcast_ref)
    }

    fn erased_equal(left: &ErasedValue, right: &ErasedValue) -> bool {
        if left.type_id() != right.type_id() {
            return false;
        }
        if let (Some(left), Some(right)) = (left.downcast_ref::<u32>(), right.downcast_ref::<u32>())
        {
            return left == right;
        }
        if let (Some(left), Some(right)) = (
            left.downcast_ref::<String>(),
            right.downcast_ref::<String>(),
        ) {
            return left == right;
        }
        false
    }
}

impl BindingHost<u32> for TestHost {
    fn get_erased(&self, endpoint: EndpointKey<u32>) -> Option<ErasedValue> {
        self.values.get(&endpoint).cloned()
    }

    fn set_erased(
        &mut self,
        endpoint: EndpointKey<u32>,
        value: ErasedValue,
        (): (),
    ) -> BindingWrite {
        let changed = self
            .values
            .get(&endpoint)
            .is_none_or(|old| !Self::erased_equal(old, &value));
        self.values.insert(endpoint, value);
        self.writes.push(endpoint);

        let channels = if changed {
            self.channels
                .get(&endpoint.property())
                .copied()
                .unwrap_or_else(ChannelSet::empty)
        } else {
            ChannelSet::empty()
        };

        BindingWrite::new(changed, channels)
    }
}

fn registry() -> (PropertyRegistry, Property<u32>) {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0_u32).build());
    (registry, width)
}

#[test]
fn forward_propagation_matches_one_way_behavior() {
    let (_registry, width) = registry();
    let model = PropertyEndpoint::new(1, width);
    let control = PropertyEndpoint::new(2, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    two_way.bind(&mut bindings, model, control, (), ()).unwrap();

    let mut host = TestHost::default();
    host.set_initial(model, 42_u32);
    host.set_channels(control, LAYOUT.into_set());

    bindings.mark_source_changed(model);
    let report = two_way.drain(&mut bindings, &mut host).unwrap();

    assert_eq!(report.reverse_writes(), 0);
    assert_eq!(report.forward().evaluated_bindings(), 1);
    assert_eq!(report.forward().changed_bindings(), 1);
    assert!(report.affected_channels().contains(LAYOUT));
    assert_eq!(host.value(control), Some(&42));
}

#[test]
fn target_write_reaches_source() {
    let (_registry, width) = registry();
    let model = PropertyEndpoint::new(1, width);
    let control = PropertyEndpoint::new(2, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    let id = two_way.bind(&mut bindings, model, control, (), ()).unwrap();

    let mut host = TestHost::default();
    host.set_initial(model, 10_u32);
    host.set_channels(model, PAINT.into_set());

    // The control writes its own endpoint, then reports it.
    host.set_initial(control, 70_u32);
    assert_eq!(
        two_way.report_target_write(&mut bindings, control.key()),
        Some(id),
    );
    assert!(two_way.has_pending_reverse());

    let report = two_way.drain(&mut bindings, &mut host).unwrap();

    assert_eq!(report.reverse_writes(), 1);
    assert_eq!(report.reverse_changed(), 1);
    assert!(report.reverse_channels().contains(PAINT));
    assert!(report.affected_channels().contains(PAINT));
    assert_eq!(host.value(model), Some(&70));
    assert!(!two_way.has_pending_reverse());
}

#[test]
fn round_trip_settles_in_one_drain_without_echo() {
    let (_registry, width) = registry();
    let model = PropertyEndpoint::new(1, width);
    let control = PropertyEndpoint::new(2, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    two_way.bind(&mut bindings, model, control, (), ()).unwrap();

    let mut host = TestHost::default();
    host.set_initial(model, 10_u32);
    host.set_initial(control, 70_u32);

    two_way.report_target_write(&mut bindings, control.key());
    let report = two_way.drain(&mut bindings, &mut host).unwrap();

    // One reverse write to the source; the forward binding is not
    // re-dirtied, so the control endpoint is never rewritten.
    assert_eq!(report.reverse_writes(), 1);
    assert_eq!(report.forward().evaluated_bindings(), 0);
    assert_eq!(host.writes, alloc::vec![model.key()]);
    assert!(!bindings.has_dirty_bindings());
    assert!(!two_way.has_pending_reverse());

    // No residual oscillation across later drains.
    let settle = two_way.drain(&mut bindings, &mut host).unwrap();
    assert_eq!(settle.reverse_writes(), 0);
    assert_eq!(settle.forward().evaluated_bindings(), 0);
}

#[test]
fn reverse_write_updates_other_source_readers() {
    let (_registry, width) = registry();
    let model = PropertyEndpoint::new(1, width);
    let slider = PropertyEndpoint::new(2, width);
    let mirror = PropertyEndpoint::new(3, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    two_way.bind(&mut bindings, model, slider, (), ()).unwrap();
    bindings.bind(model, mirror, ()).unwrap();

    let mut host = TestHost::default();
    host.set_initial(model, 10_u32);
    host.set_initial(slider, 70_u32);

    two_way.report_target_write(&mut bindings, slider.key());
    let report = two_way.drain(&mut bindings, &mut host).unwrap();

    // Reverse write to the model, then forward propagation to the mirror
    // reader only. The originating slider endpoint is not rewritten.
    assert_eq!(report.reverse_writes(), 1);
    assert_eq!(report.forward().evaluated_bindings(), 1);
    assert_eq!(host.writes, alloc::vec![model.key(), mirror.key()]);
    assert_eq!(host.value(mirror), Some(&70));
}

#[test]
fn sibling_two_way_targets_stay_in_sync_without_reverse_cascade() {
    let (_registry, width) = registry();
    let model = PropertyEndpoint::new(1, width);
    let slider = PropertyEndpoint::new(2, width);
    let stepper = PropertyEndpoint::new(3, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    two_way.bind(&mut bindings, model, slider, (), ()).unwrap();
    two_way.bind(&mut bindings, model, stepper, (), ()).unwrap();

    let mut host = TestHost::default();
    host.set_initial(model, 10_u32);
    host.set_initial(slider, 70_u32);
    host.set_initial(stepper, 10_u32);

    two_way.report_target_write(&mut bindings, slider.key());
    let report = two_way.drain(&mut bindings, &mut host).unwrap();

    // Reverse to the model, forward to the sibling stepper. The stepper's
    // update is binding-driven, so it schedules no further reverse.
    assert_eq!(report.reverse_writes(), 1);
    assert_eq!(report.forward().evaluated_bindings(), 1);
    assert_eq!(host.value(model), Some(&70));
    assert_eq!(host.value(stepper), Some(&70));
    assert_eq!(host.writes, alloc::vec![model.key(), stepper.key()]);
    assert!(!two_way.has_pending_reverse());
}

#[test]
fn control_write_updates_target_readers() {
    let (_registry, width) = registry();
    let model = PropertyEndpoint::new(1, width);
    let slider = PropertyEndpoint::new(2, width);
    let tooltip = PropertyEndpoint::new(3, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    two_way.bind(&mut bindings, model, slider, (), ()).unwrap();
    bindings.bind(slider, tooltip, ()).unwrap();

    let mut host = TestHost::default();
    host.set_initial(model, 10_u32);
    host.set_initial(slider, 70_u32);

    two_way.report_target_write(&mut bindings, slider.key());
    two_way.drain(&mut bindings, &mut host).unwrap();

    assert_eq!(host.value(tooltip), Some(&70));
    assert_eq!(host.value(model), Some(&70));
}

#[test]
fn report_for_plain_endpoint_marks_readers_and_returns_none() {
    let (_registry, width) = registry();
    let source = PropertyEndpoint::new(1, width);
    let target = PropertyEndpoint::new(2, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    bindings.bind(source, target, ()).unwrap();

    let mut host = TestHost::default();
    host.set_initial(source, 5_u32);

    assert_eq!(
        two_way.report_target_write(&mut bindings, source.key()),
        None,
    );
    assert!(!two_way.has_pending_reverse());
    assert!(bindings.has_dirty_bindings());

    two_way.drain(&mut bindings, &mut host).unwrap();
    assert_eq!(host.value(target), Some(&5));
}

#[test]
fn back_edge_is_not_flagged_as_cycle() {
    let (_registry, width) = registry();
    let model = PropertyEndpoint::new(1, width);
    let control = PropertyEndpoint::new(2, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();

    assert!(two_way.bind(&mut bindings, model, control, (), ()).is_ok());
    assert_eq!(two_way.len(), 1);
    assert_eq!(bindings.len(), 1);
}

#[test]
fn genuine_cycle_through_third_binding_is_rejected() {
    let (_registry, width) = registry();
    let first = PropertyEndpoint::new(1, width);
    let second = PropertyEndpoint::new(2, width);
    let third = PropertyEndpoint::new(3, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    two_way.bind(&mut bindings, first, second, (), ()).unwrap();
    bindings.bind(second, third, ()).unwrap();

    // third -> first closes first -> second -> third -> first through the
    // two-way pair's forward edge.
    let error = bindings.bind(third, first, ()).unwrap_err();
    assert!(matches!(error, BindingError::Cycle { .. }));

    // The same applies when the closing edge is itself two-way.
    let error = two_way
        .bind(&mut bindings, third, first, (), ())
        .unwrap_err();
    assert!(matches!(error, BindingError::Cycle { .. }));
    assert_eq!(two_way.len(), 1);
}

#[test]
fn conflict_in_same_drain_resolves_to_target_initiated_value() {
    let (_registry, width) = registry();
    let model = PropertyEndpoint::new(1, width);
    let control = PropertyEndpoint::new(2, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    two_way.bind(&mut bindings, model, control, (), ()).unwrap();

    let mut host = TestHost::default();

    // The model changed to 33 and the user wrote 70 before the drain.
    host.set_initial(model, 33_u32);
    bindings.mark_source_changed(model);
    host.set_initial(control, 70_u32);
    two_way.report_target_write(&mut bindings, control.key());

    let report = two_way.drain(&mut bindings, &mut host).unwrap();

    // Reverse applies first, so the dirty forward binding re-reads the
    // post-write-back source and re-asserts the control's value.
    assert_eq!(report.reverse_writes(), 1);
    assert_eq!(report.forward().evaluated_bindings(), 1);
    assert_eq!(report.forward().changed_bindings(), 0);
    assert_eq!(host.value(model), Some(&70));
    assert_eq!(host.value(control), Some(&70));
    assert!(!bindings.has_dirty_bindings());
}

#[test]
fn repeated_reports_before_drain_write_back_latest_value_once() {
    let (_registry, width) = registry();
    let model = PropertyEndpoint::new(1, width);
    let control = PropertyEndpoint::new(2, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    two_way.bind(&mut bindings, model, control, (), ()).unwrap();

    let mut host = TestHost::default();
    host.set_initial(model, 10_u32);

    host.set_initial(control, 40_u32);
    two_way.report_target_write(&mut bindings, control.key());
    host.set_initial(control, 55_u32);
    two_way.report_target_write(&mut bindings, control.key());

    let report = two_way.drain(&mut bindings, &mut host).unwrap();

    assert_eq!(report.reverse_writes(), 1);
    assert_eq!(host.value(model), Some(&55));
    assert_eq!(host.writes, alloc::vec![model.key()]);
}

#[test]
fn mapped_two_way_converts_in_both_directions() {
    let mut registry = PropertyRegistry::new();
    let count = registry.register("Count", PropertyMetadataBuilder::new(0_u32).build());
    let text = registry.register("Text", PropertyMetadataBuilder::new(String::new()).build());
    let model = PropertyEndpoint::new(1, count);
    let editor = PropertyEndpoint::new(2, text);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    two_way
        .try_bind_map(
            &mut bindings,
            model,
            editor,
            (),
            (),
            |count: &u32| count.to_string(),
            |text: &String| text.parse::<u32>().ok(),
        )
        .unwrap();

    let mut host = TestHost::default();
    host.set_initial(model, 7_u32);

    bindings.mark_source_changed(model);
    two_way.drain(&mut bindings, &mut host).unwrap();
    assert_eq!(host.value(editor).map(String::as_str), Some("7"));

    host.set_initial(editor, String::from("29"));
    two_way.report_target_write(&mut bindings, editor.key());
    two_way.drain(&mut bindings, &mut host).unwrap();
    assert_eq!(host.value(model), Some(&29));
}

#[test]
fn unbind_cleans_up_both_directions() {
    let (_registry, width) = registry();
    let model = PropertyEndpoint::new(1, width);
    let control = PropertyEndpoint::new(2, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    let id = two_way.bind(&mut bindings, model, control, (), ()).unwrap();

    let mut host = TestHost::default();
    host.set_initial(model, 10_u32);
    host.set_initial(control, 70_u32);
    two_way.report_target_write(&mut bindings, control.key());

    assert!(two_way.unbind(&mut bindings, id));
    assert!(!two_way.unbind(&mut bindings, id));
    assert_eq!(two_way.len(), 0);
    assert!(bindings.is_empty());
    assert!(!two_way.has_pending_reverse());
    assert_eq!(two_way.forward_binding(id), None);
    assert_eq!(two_way.binding_for_target(control.key()), None);

    // Forward direction is gone: source changes no longer propagate.
    assert!(!bindings.mark_source_changed(model));
    // Reverse direction is gone: reports no longer schedule write-back.
    assert_eq!(
        two_way.report_target_write(&mut bindings, control.key()),
        None,
    );

    let report = two_way.drain(&mut bindings, &mut host).unwrap();
    assert_eq!(report.reverse_writes(), 0);
    assert_eq!(report.forward().evaluated_bindings(), 0);
    assert_eq!(host.value(model), Some(&10));
}

#[test]
fn clear_owner_removes_two_way_pairs() {
    let (_registry, width) = registry();
    let model = PropertyEndpoint::new(1, width);
    let control = PropertyEndpoint::new(2, width);
    let other_model = PropertyEndpoint::new(3, width);
    let other_control = PropertyEndpoint::new(4, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    two_way.bind(&mut bindings, model, control, (), ()).unwrap();
    two_way
        .bind(&mut bindings, other_model, other_control, (), ())
        .unwrap();

    assert_eq!(two_way.clear_owner(&mut bindings, 2), 1);
    assert_eq!(two_way.len(), 1);
    assert_eq!(bindings.len(), 1);
    assert_eq!(two_way.binding_for_target(control.key()), None);
    assert!(two_way.binding_for_target(other_control.key()).is_some());
}

#[test]
fn missing_target_value_skips_reverse_and_stays_unscheduled() {
    let (_registry, width) = registry();
    let model = PropertyEndpoint::new(1, width);
    let control = PropertyEndpoint::new(2, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    two_way.bind(&mut bindings, model, control, (), ()).unwrap();

    let mut host = TestHost::default();
    host.set_initial(model, 10_u32);

    // Reported before the control endpoint has any stored value.
    two_way.report_target_write(&mut bindings, control.key());
    let report = two_way.drain(&mut bindings, &mut host).unwrap();

    assert_eq!(report.reverse_writes(), 0);
    assert_eq!(report.reverse_skipped_missing_target(), 1);
    assert!(!two_way.has_pending_reverse());
    assert_eq!(host.value(model), Some(&10));

    // Once the value exists the host re-reports and write-back succeeds.
    host.set_initial(control, 70_u32);
    two_way.report_target_write(&mut bindings, control.key());
    let retry = two_way.drain(&mut bindings, &mut host).unwrap();

    assert_eq!(retry.reverse_writes(), 1);
    assert_eq!(retry.reverse_skipped_missing_target(), 0);
    assert_eq!(host.value(model), Some(&70));
}

#[test]
fn reverse_type_mismatch_is_reported_without_stalling_batch() {
    let (_registry, width) = registry();
    let model = PropertyEndpoint::new(1, width);
    let control = PropertyEndpoint::new(2, width);
    let sibling_model = PropertyEndpoint::new(3, width);
    let sibling_control = PropertyEndpoint::new(4, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    let id = two_way.bind(&mut bindings, model, control, (), ()).unwrap();
    let forward = two_way.forward_binding(id).unwrap();
    two_way
        .bind(&mut bindings, sibling_model, sibling_control, (), ())
        .unwrap();

    let mut host = TestHost::default();
    host.set_initial(model, 10_u32);
    host.set_initial(sibling_model, 20_u32);
    host.values
        .insert(control.key(), ErasedValue::new(String::from("wrong")));
    host.set_initial(sibling_control, 80_u32);

    two_way.report_target_write(&mut bindings, control.key());
    two_way.report_target_write(&mut bindings, sibling_control.key());
    let report = two_way.drain(&mut bindings, &mut host).unwrap();

    assert_eq!(report.reverse_writes(), 1);
    assert_eq!(report.reverse_rejected(), 1);
    let rejection = &report.reverse_rejections()[0];
    assert_eq!(rejection.binding(), id);
    assert_eq!(rejection.forward_binding(), forward);
    assert_eq!(rejection.source(), model.key());
    assert_eq!(rejection.target(), control.key());
    assert!(matches!(
        rejection.reason(),
        ReverseRejectionReason::TargetTypeMismatch { .. }
    ));
    assert_eq!(host.value(model), Some(&10));
    assert_eq!(host.value(sibling_model), Some(&80));
    assert!(!two_way.has_pending_reverse());

    // Repairing the host value requires a fresh report, mirroring the
    // missing-target retry contract.
    host.values.insert(control.key(), ErasedValue::new(70_u32));
    two_way.report_target_write(&mut bindings, control.key());
    let retry = two_way.drain(&mut bindings, &mut host).unwrap();
    assert_eq!(retry.reverse_writes(), 1);
    assert_eq!(host.value(model), Some(&70));
}

#[test]
fn rejected_reverse_conversion_is_reported_without_write_back() {
    let mut registry = PropertyRegistry::new();
    let count = registry.register("Count", PropertyMetadataBuilder::new(0_u32).build());
    let text = registry.register("Text", PropertyMetadataBuilder::new(String::new()).build());
    let model = PropertyEndpoint::new(1, count);
    let editor = PropertyEndpoint::new(2, text);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    let id = two_way
        .try_bind_map(
            &mut bindings,
            model,
            editor,
            (),
            (),
            |count: &u32| count.to_string(),
            |text: &String| text.parse::<u32>().ok(),
        )
        .unwrap();

    let mut host = TestHost::default();
    host.set_initial(model, 7_u32);
    host.set_initial(editor, String::from("not a number"));

    two_way.report_target_write(&mut bindings, editor.key());
    let report = two_way.drain(&mut bindings, &mut host).unwrap();

    assert_eq!(report.reverse_writes(), 0);
    assert_eq!(report.reverse_rejected(), 1);
    let rejection = &report.reverse_rejections()[0];
    assert_eq!(rejection.binding(), id);
    assert_eq!(rejection.source(), model.key());
    assert_eq!(rejection.target(), editor.key());
    assert_eq!(
        rejection.reason(),
        ReverseRejectionReason::ConversionRejected
    );
    assert_eq!(host.value(model), Some(&7));
    assert!(!two_way.has_pending_reverse());
}

#[test]
fn reverse_writes_apply_in_report_order() {
    let (_registry, width) = registry();
    let first_model = PropertyEndpoint::new(1, width);
    let first_control = PropertyEndpoint::new(2, width);
    let second_model = PropertyEndpoint::new(3, width);
    let second_control = PropertyEndpoint::new(4, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    let first = two_way
        .bind(&mut bindings, first_model, first_control, (), ())
        .unwrap();
    let second = two_way
        .bind(&mut bindings, second_model, second_control, (), ())
        .unwrap();
    assert!(first < second);

    let mut host = TestHost::default();
    host.set_initial(first_model, 1_u32);
    host.set_initial(second_model, 2_u32);
    host.set_initial(first_control, 11_u32);
    host.set_initial(second_control, 22_u32);

    // Reported out of id order; write-back follows host report order.
    two_way.report_target_write(&mut bindings, second_control.key());
    two_way.report_target_write(&mut bindings, first_control.key());
    let report = two_way.drain(&mut bindings, &mut host).unwrap();

    assert_eq!(report.reverse_writes(), 2);
    assert_eq!(
        host.writes,
        alloc::vec![second_model.key(), first_model.key()]
    );
}

#[test]
fn repeated_report_moves_reverse_write_to_back() {
    let (_registry, width) = registry();
    let first_model = PropertyEndpoint::new(1, width);
    let first_control = PropertyEndpoint::new(2, width);
    let second_model = PropertyEndpoint::new(3, width);
    let second_control = PropertyEndpoint::new(4, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    two_way
        .bind(&mut bindings, first_model, first_control, (), ())
        .unwrap();
    two_way
        .bind(&mut bindings, second_model, second_control, (), ())
        .unwrap();

    let mut host = TestHost::default();
    host.set_initial(first_model, 1_u32);
    host.set_initial(second_model, 2_u32);
    host.set_initial(first_control, 11_u32);
    host.set_initial(second_control, 22_u32);

    two_way.report_target_write(&mut bindings, first_control.key());
    two_way.report_target_write(&mut bindings, second_control.key());
    two_way.report_target_write(&mut bindings, first_control.key());
    let report = two_way.drain(&mut bindings, &mut host).unwrap();

    assert_eq!(report.reverse_writes(), 2);
    assert_eq!(
        host.writes,
        alloc::vec![second_model.key(), first_model.key()]
    );
}

#[test]
fn unchanged_reverse_write_does_not_dirty_source_readers() {
    let (_registry, width) = registry();
    let model = PropertyEndpoint::new(1, width);
    let control = PropertyEndpoint::new(2, width);
    let mirror = PropertyEndpoint::new(3, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    two_way.bind(&mut bindings, model, control, (), ()).unwrap();
    bindings.bind(model, mirror, ()).unwrap();

    let mut host = TestHost::default();
    host.set_initial(model, 70_u32);
    host.set_initial(control, 70_u32);

    // The control reports a write whose value equals the source. The host
    // reports it as unchanged, so downstream readers are not re-run.
    two_way.report_target_write(&mut bindings, control.key());
    let report = two_way.drain(&mut bindings, &mut host).unwrap();

    assert_eq!(report.reverse_writes(), 1);
    assert_eq!(report.reverse_changed(), 0);
    assert_eq!(report.forward().evaluated_bindings(), 0);
    assert_eq!(host.writes, alloc::vec![model.key()]);
}

#[test]
fn two_way_binding_id_accessors() {
    let (_registry, width) = registry();
    let model = PropertyEndpoint::new(1, width);
    let control = PropertyEndpoint::new(2, width);

    let mut bindings = BindingSet::new(BINDING);
    let mut two_way = TwoWayBindings::new();
    let id = two_way.bind(&mut bindings, model, control, (), ()).unwrap();

    assert_eq!(id.get(), 0);
    assert_eq!(two_way.binding_for_target(control.key()), Some(id));
    assert_eq!(two_way.binding_for_target(model.key()), None);
    let forward = two_way.forward_binding(id).unwrap();
    assert_eq!(two_way.binding_for_forward(forward), Some(id));
    assert_eq!(two_way.forward_binding(TwoWayBindingId(9)), None);
}
