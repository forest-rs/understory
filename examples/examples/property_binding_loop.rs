// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Canonical property binding host loop.
//!
//! This example shows the intended host-side shape for
//! `understory_property_binding` with `understory_property` source slots:
//!
//! 1. Host elements store values in [`PropertyStore`].
//! 2. Bindings write target values into [`LocalValueSource::TemplateBinding`].
//! 3. User [`LocalValueSource::Local`] values can mask binding updates without
//!    deleting them.
//! 4. Clearing `Local` reveals the latest binding value.
//! 5. Template teardown removes bindings and clears template-installed source
//!    slots, revealing [`LocalValueSource::TemplateDefault`].
//!
//! Run:
//! - `cargo run -p understory_examples --example property_binding_loop`

use std::collections::BTreeMap;

use invalidation::{Channel, ChannelSet};
use understory_property::{
    DependencyObject, DependencyObjectExt, ErasedValue, LocalValueSource, Property,
    PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
};
use understory_property_binding::{
    BindingDrainError, BindingHost, BindingReport, BindingSet, BindingWrite, EndpointKey,
    PropertyEndpoint,
};

const BINDING: Channel = Channel::new(0);
const LAYOUT: Channel = Channel::new(1);

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ElementId(u32);

struct Element {
    key: ElementId,
    store: PropertyStore<ElementId>,
}

impl Element {
    fn new(key: ElementId) -> Self {
        Self {
            key,
            store: PropertyStore::new(key),
        }
    }
}

impl DependencyObject<ElementId> for Element {
    fn property_store(&self) -> &PropertyStore<ElementId> {
        &self.store
    }

    fn property_store_mut(&mut self) -> &mut PropertyStore<ElementId> {
        &mut self.store
    }

    fn key(&self) -> ElementId {
        self.key
    }

    fn parent_key(&self) -> Option<ElementId> {
        None
    }
}

#[derive(Default)]
struct AppInvalidation {
    channels: ChannelSet,
}

impl AppInvalidation {
    fn apply_report(&mut self, report: BindingReport) {
        self.apply_channels(report.affected_channels());
        println!(
            "  report: evaluated={} changed={} affected={:?}",
            report.evaluated_bindings(),
            report.changed_bindings(),
            report.affected_channels()
        );
    }

    fn apply_channels(&mut self, channels: ChannelSet) {
        self.channels |= channels;
    }
}

struct Host {
    registry: PropertyRegistry,
    width: Property<u32>,
    elements: BTreeMap<ElementId, Element>,
}

impl Host {
    fn new(registry: PropertyRegistry, width: Property<u32>) -> Self {
        Self {
            registry,
            width,
            elements: BTreeMap::new(),
        }
    }

    fn insert_element(&mut self, id: ElementId) {
        self.elements.insert(id, Element::new(id));
    }

    fn set_model_width(&mut self, endpoint: PropertyEndpoint<ElementId, u32>, value: u32) {
        self.element_mut(endpoint.owner())
            .set_local(endpoint.property(), value);
    }

    fn set_template_default(&mut self, endpoint: PropertyEndpoint<ElementId, u32>, value: u32) {
        self.element_mut(endpoint.owner()).set_local_with_source(
            endpoint.property(),
            value,
            LocalValueSource::TemplateDefault,
        );
    }

    fn set_user_width(&mut self, endpoint: PropertyEndpoint<ElementId, u32>, value: u32) {
        self.element_mut(endpoint.owner())
            .set_local(endpoint.property(), value);
    }

    fn clear_user_width(&mut self, endpoint: PropertyEndpoint<ElementId, u32>) -> ChannelSet {
        let registry = &self.registry;
        self.elements
            .get_mut(&endpoint.owner())
            .expect("example element should exist")
            .clear_local_notifying(endpoint.property(), registry)
    }

    fn clear_template_bindings_for(&mut self, owner: ElementId) -> ChannelSet {
        let registry = &self.registry;
        self.elements
            .get_mut(&owner)
            .expect("example element should exist")
            .clear_local_by_source_notifying(LocalValueSource::TemplateBinding, registry)
    }

    fn width_value(&self, endpoint: PropertyEndpoint<ElementId, u32>) -> Option<u32> {
        self.elements
            .get(&endpoint.owner())
            .map(|element| element.get_effective_local(endpoint.property(), &self.registry))
    }

    fn width_source(&self, endpoint: PropertyEndpoint<ElementId, u32>) -> Option<LocalValueSource> {
        self.elements
            .get(&endpoint.owner())
            .and_then(|element| element.get_local_source(endpoint.property()))
    }

    fn width_at_source(
        &self,
        endpoint: PropertyEndpoint<ElementId, u32>,
        source: LocalValueSource,
    ) -> Option<&u32> {
        self.elements.get(&endpoint.owner()).and_then(|element| {
            element
                .property_store()
                .get_local_at_source(endpoint.property(), source)
        })
    }

    fn element_mut(&mut self, id: ElementId) -> &mut Element {
        self.elements
            .get_mut(&id)
            .expect("example element should exist")
    }
}

impl BindingHost<ElementId> for Host {
    fn get_erased(&self, endpoint: EndpointKey<ElementId>) -> Option<ErasedValue> {
        if endpoint.property() != self.width.id() {
            return None;
        }

        self.elements.get(&endpoint.owner()).map(|element| {
            ErasedValue::new(element.get_effective_local(self.width, &self.registry))
        })
    }

    fn set_erased(&mut self, endpoint: EndpointKey<ElementId>, value: ErasedValue) -> BindingWrite {
        if endpoint.property() != self.width.id() {
            return BindingWrite::unchanged();
        }

        let Some(value) = value.downcast_ref::<u32>().copied() else {
            return BindingWrite::unchanged();
        };

        let width = self.width;
        let registry = &self.registry;
        let Some(element) = self.elements.get_mut(&endpoint.owner()) else {
            return BindingWrite::unchanged();
        };

        let old_effective = element.get_effective_local(width, registry);
        let channels = element.set_local_with_source_notifying(
            width,
            value,
            LocalValueSource::TemplateBinding,
            registry,
        );
        let new_effective = element.get_effective_local(width, registry);

        BindingWrite::new(old_effective != new_effective, channels)
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

fn print_width(host: &Host, label: &str, endpoint: PropertyEndpoint<ElementId, u32>) {
    println!(
        "  {label}: effective={:?} source={:?} template_binding={:?} template_default={:?}",
        host.width_value(endpoint),
        host.width_source(endpoint),
        host.width_at_source(endpoint, LocalValueSource::TemplateBinding),
        host.width_at_source(endpoint, LocalValueSource::TemplateDefault)
    );
}

fn main() {
    let model = ElementId(1);
    let button = ElementId(2);
    let panel = ElementId(3);
    let delayed_model = ElementId(4);

    let mut registry = PropertyRegistry::new();
    let width = registry.register(
        "Width",
        PropertyMetadataBuilder::new(0_u32)
            .affects_channels(LAYOUT.into_set())
            .build(),
    );

    let model_width = PropertyEndpoint::new(model, width);
    let button_width = PropertyEndpoint::new(button, width);
    let delayed_width = PropertyEndpoint::new(delayed_model, width);
    let panel_width = PropertyEndpoint::new(panel, width);

    let mut host = Host::new(registry, width);
    host.insert_element(model);
    host.insert_element(button);
    host.insert_element(panel);
    host.set_model_width(model_width, 320);
    host.set_template_default(button_width, 100);
    host.set_template_default(panel_width, 200);

    let mut invalidation = AppInvalidation::default();
    let mut bindings = BindingSet::new(BINDING);
    bindings.bind(model_width, button_width).unwrap();
    bindings.bind(delayed_width, panel_width).unwrap();

    println!("== first frame ==");
    println!("external changes: model width and missing delayed model width");
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
    print_width(&host, "button", button_width);
    print_width(&host, "panel", panel_width);

    println!();
    println!("== repair and retry ==");
    host.insert_element(delayed_model);
    host.set_model_width(delayed_width, 480);
    let clean = drain_frame(&mut bindings, &mut host, &mut invalidation);
    println!(
        "  clean={clean} dirty={}",
        bindings.stats().dirty_bindings()
    );
    print_width(&host, "panel", panel_width);

    println!();
    println!("== normal update ==");
    println!("external change: model width");
    host.set_model_width(model_width, 640);
    bindings.mark_source_changed(model_width);
    let clean = drain_frame(&mut bindings, &mut host, &mut invalidation);
    println!(
        "  clean={clean} dirty={}",
        bindings.stats().dirty_bindings()
    );
    print_width(&host, "button", button_width);

    println!();
    println!("== user local masks binding ==");
    host.set_user_width(button_width, 700);
    print_width(&host, "button after user local", button_width);
    host.set_model_width(model_width, 800);
    bindings.mark_source_changed(model_width);
    let clean = drain_frame(&mut bindings, &mut host, &mut invalidation);
    println!(
        "  clean={clean} dirty={}",
        bindings.stats().dirty_bindings()
    );
    print_width(&host, "button after masked binding", button_width);

    println!();
    println!("== clear local reveals binding ==");
    let channels = host.clear_user_width(button_width);
    invalidation.apply_channels(channels);
    println!("  clear Local affected={channels:?}");
    print_width(&host, "button", button_width);

    println!();
    println!("== template teardown ==");
    let removed = bindings.clear_owner(button) + bindings.clear_owner(panel);
    let channels =
        host.clear_template_bindings_for(button) | host.clear_template_bindings_for(panel);
    invalidation.apply_channels(channels);
    println!("removed bindings for retained owners: {removed}");
    println!("cleared TemplateBinding affected={channels:?}");
    println!("active bindings={}", bindings.stats().active_bindings());
    print_width(&host, "button", button_width);
    print_width(&host, "panel", panel_width);

    host.set_model_width(model_width, 960);
    let marked = bindings.mark_source_changed(model_width);
    println!("mark after teardown: {marked}");
    let clean = drain_frame(&mut bindings, &mut host, &mut invalidation);
    println!("  clean={clean}");
    print_width(&host, "button", button_width);
}
