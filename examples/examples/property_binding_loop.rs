// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Canonical property binding host loop.
//!
//! This example shows the intended host-side shape for
//! `understory_property_binding`:
//!
//! 1. Register bindings between host-owned property endpoints.
//! 2. Mark source endpoints dirty when host state changes.
//! 3. Drain bindings.
//! 4. Apply both successful and partial drain reports to app invalidation.
//! 5. Tear down bindings by owner when a retained object goes away.
//!
//! Run:
//! - `cargo run -p understory_examples --example property_binding_loop`

use std::collections::BTreeMap;

use invalidation::{Channel, ChannelSet};
use understory_property::{ErasedValue, PropertyId, PropertyMetadataBuilder, PropertyRegistry};
use understory_property_binding::{
    BindingDrainError, BindingHost, BindingReport, BindingSet, BindingWrite, EndpointKey,
    PropertyEndpoint,
};

const BINDING: Channel = Channel::new(0);
const LAYOUT: Channel = Channel::new(1);

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ElementId(u32);

#[derive(Default)]
struct AppInvalidation {
    channels: ChannelSet,
}

impl AppInvalidation {
    fn apply_report(&mut self, report: BindingReport) {
        self.channels |= report.affected_channels();
        println!(
            "  report: evaluated={} changed={} affected={:?}",
            report.evaluated_bindings(),
            report.changed_bindings(),
            report.affected_channels()
        );
    }
}

#[derive(Default)]
struct Host {
    values: BTreeMap<EndpointKey<ElementId>, ErasedValue>,
    affects: BTreeMap<PropertyId, ChannelSet>,
}

impl Host {
    fn set_initial<T: Clone + 'static>(
        &mut self,
        endpoint: PropertyEndpoint<ElementId, T>,
        value: T,
    ) {
        self.values.insert(endpoint.key(), ErasedValue::new(value));
    }

    fn set_affects<T>(&mut self, endpoint: PropertyEndpoint<ElementId, T>, channels: ChannelSet) {
        self.affects.insert(endpoint.property().id(), channels);
    }

    fn value<T: 'static>(&self, endpoint: PropertyEndpoint<ElementId, T>) -> Option<&T> {
        self.values
            .get(&endpoint.key())
            .and_then(ErasedValue::downcast_ref)
    }

    fn erased_equal(left: &ErasedValue, right: &ErasedValue) -> bool {
        if left.type_id() != right.type_id() {
            return false;
        }

        left.downcast_ref::<u32>() == right.downcast_ref::<u32>()
    }
}

impl BindingHost<ElementId> for Host {
    fn get_erased(&self, endpoint: EndpointKey<ElementId>) -> Option<ErasedValue> {
        self.values.get(&endpoint).cloned()
    }

    fn set_erased(&mut self, endpoint: EndpointKey<ElementId>, value: ErasedValue) -> BindingWrite {
        let changed = self
            .values
            .get(&endpoint)
            .is_none_or(|old| !Self::erased_equal(old, &value));
        self.values.insert(endpoint, value);

        let channels = if changed {
            self.affects
                .get(&endpoint.property())
                .copied()
                .unwrap_or_else(ChannelSet::empty)
        } else {
            ChannelSet::empty()
        };

        BindingWrite::new(changed, channels)
    }
}

fn drain_frame(
    bindings: &mut BindingSet<ElementId>,
    host: &mut Host,
    invalidation: &mut AppInvalidation,
) -> bool {
    match bindings.drain(host) {
        Ok(report) => {
            println!("drain: ok");
            invalidation.apply_report(report);
            true
        }
        Err(error) => {
            print_drain_error(&error);
            invalidation.apply_report(error.report());
            false
        }
    }
}

fn print_drain_error(error: &BindingDrainError<ElementId>) {
    println!("drain: stopped at {:?}", error.error());
    println!("  completed writes still need app invalidation");
}

fn main() {
    let model = ElementId(1);
    let button = ElementId(2);
    let panel = ElementId(3);
    let delayed_model = ElementId(4);

    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0_u32).build());

    let model_width = PropertyEndpoint::new(model, width);
    let button_width = PropertyEndpoint::new(button, width);
    let delayed_width = PropertyEndpoint::new(delayed_model, width);
    let panel_width = PropertyEndpoint::new(panel, width);

    let mut host = Host::default();
    host.set_affects(button_width, LAYOUT.into_set());
    host.set_affects(panel_width, LAYOUT.into_set());
    host.set_initial(model_width, 320_u32);

    let mut invalidation = AppInvalidation::default();
    let mut bindings = BindingSet::new(BINDING);
    bindings.bind(model_width, button_width).unwrap();
    bindings.bind(delayed_width, panel_width).unwrap();

    println!("== first frame ==");
    println!("external changes: model width and delayed model width");
    bindings.mark_source_changed(model_width);
    bindings.mark_source_changed(delayed_width);

    let clean = drain_frame(&mut bindings, &mut host, &mut invalidation);
    println!(
        "  clean={clean} dirty={}",
        bindings.stats().dirty_bindings()
    );
    println!(
        "  app invalidated layout={}",
        invalidation.channels.contains(LAYOUT)
    );
    println!("  button width={:?}", host.value(button_width));
    println!("  panel width={:?}", host.value(panel_width));

    println!();
    println!("== repair and retry ==");
    host.set_initial(delayed_width, 480_u32);
    let clean = drain_frame(&mut bindings, &mut host, &mut invalidation);
    println!(
        "  clean={clean} dirty={}",
        bindings.stats().dirty_bindings()
    );
    println!("  panel width={:?}", host.value(panel_width));

    println!();
    println!("== normal update ==");
    println!("external change: model width");
    host.set_initial(model_width, 640_u32);
    bindings.mark_source_changed(model_width);
    let clean = drain_frame(&mut bindings, &mut host, &mut invalidation);
    println!(
        "  clean={clean} dirty={}",
        bindings.stats().dirty_bindings()
    );
    println!("  button width={:?}", host.value(button_width));

    println!();
    println!("== teardown ==");
    let removed = bindings.clear_owner(button) + bindings.clear_owner(panel);
    println!("removed bindings for retained owners: {removed}");
    println!("active bindings={}", bindings.stats().active_bindings());

    host.set_initial(model_width, 960_u32);
    let marked = bindings.mark_source_changed(model_width);
    println!("mark after teardown: {marked}");
    let clean = drain_frame(&mut bindings, &mut host, &mut invalidation);
    println!(
        "  clean={clean} button width={:?}",
        host.value(button_width)
    );
}
