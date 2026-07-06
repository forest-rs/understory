// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! End-to-end style, expression, binding, and invalidation loop.
//!
//! This example shows the host-owned frame shape that ties the core crates
//! together:
//!
//! 1. Dependency properties are the typed state model.
//! 2. Bindings push app/template relationships into local source slots.
//! 3. Style cascades choose property opinions for matched subjects.
//! 4. Expressions derive values from properties and theme resources.
//! 5. The host expands property/resource/style changes into dirty channels.
//! 6. Animation samples write the animation slot, which wins during resolution.
//!
//! Run:
//! - `cargo run -p understory_examples --example style_reactive_loop`

use std::collections::{BTreeMap, BTreeSet};

use invalidation::{Channel, ChannelSet};
use understory_property::{
    DependencyObject, DependencyObjectExt, ErasedValue, LocalValueSource, Property, PropertyId,
    PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
};
use understory_property_binding::{
    BindingHost, BindingSet, BindingWrite, EndpointKey, PropertyEndpoint,
};
use understory_style::{
    ExprDeps, ExpressionLayer, MatchState, PseudoClassId, ResolveCx, ResolveParent, ResourceKey,
    SelectorInputs, SelectorStep, Style, StyleBuilder, StyleCascadeBuilder, StyleOrigin, Theme,
    ThemeBuilder, TypeTag, expr,
};

const BINDING: Channel = Channel::new(0);
const LAYOUT: Channel = Channel::new(1);
const PAINT: Channel = Channel::new(2);

const APP: TypeTag = TypeTag(1);
const CARD: TypeTag = TypeTag(2);
const FEATURED: PseudoClassId = PseudoClassId(1);

const UNIT: ResourceKey = ResourceKey::new(1);
const GAP: ResourceKey = ResourceKey::new(2);
const ACCENT: ResourceKey = ResourceKey::new(3);

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct ElementId(u32);

#[derive(Copy, Clone)]
struct Props {
    scale: Property<f64>,
    width: Property<f64>,
    padding: Property<f64>,
    accent: Property<u32>,
}

struct Element {
    key: ElementId,
    parent: Option<ElementId>,
    store: PropertyStore<ElementId>,
}

impl Element {
    fn new(key: ElementId, parent: Option<ElementId>) -> Self {
        Self {
            key,
            parent,
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
        self.parent
    }
}

#[derive(Default)]
struct ExpressionInvalidation {
    property_targets: BTreeMap<PropertyId, BTreeSet<PropertyId>>,
    resource_targets: BTreeMap<ResourceKey, BTreeSet<PropertyId>>,
}

impl ExpressionInvalidation {
    fn from(expressions: &ExpressionLayer, styles: &[&Style]) -> Self {
        let mut index = Self::default();
        for entry in expressions.defaults().expression_entries() {
            index.add(entry.property(), entry.deps());
        }
        for style in styles {
            for entry in style.expression_entries() {
                index.add(entry.property(), entry.deps());
            }
        }
        index
    }

    fn add(&mut self, target: PropertyId, deps: &ExprDeps) {
        for property in deps.properties.iter().copied() {
            self.property_targets
                .entry(property)
                .or_default()
                .insert(target);
        }
        for resource in deps.resources.iter().copied().map(ResourceKey::from) {
            self.resource_targets
                .entry(resource)
                .or_default()
                .insert(target);
        }
    }

    fn property_targets(&self, property: PropertyId) -> impl Iterator<Item = PropertyId> + '_ {
        self.property_targets
            .get(&property)
            .into_iter()
            .flat_map(|targets| targets.iter().copied())
    }

    fn resource_targets(&self, resource: ResourceKey) -> impl Iterator<Item = PropertyId> + '_ {
        self.resource_targets
            .get(&resource)
            .into_iter()
            .flat_map(|targets| targets.iter().copied())
    }
}

#[derive(Default)]
struct DirtyFrame {
    channels: ChannelSet,
    expression_targets: BTreeSet<PropertyId>,
}

impl DirtyFrame {
    fn mark_property_changed(
        &mut self,
        property: PropertyId,
        registry: &PropertyRegistry,
        expressions: &ExpressionInvalidation,
    ) -> ChannelSet {
        let mut channels = registry.affects_channels(property);
        for target in expressions.property_targets(property) {
            self.expression_targets.insert(target);
            channels |= registry.affects_channels(target);
        }
        self.channels |= channels;
        channels
    }

    fn mark_resource_changed(
        &mut self,
        resource: ResourceKey,
        registry: &PropertyRegistry,
        expressions: &ExpressionInvalidation,
    ) -> ChannelSet {
        let mut channels = ChannelSet::empty();
        for target in expressions.resource_targets(resource) {
            self.expression_targets.insert(target);
            channels |= registry.affects_channels(target);
        }
        self.channels |= channels;
        channels
    }

    fn apply_channels(&mut self, channels: ChannelSet) {
        self.channels |= channels;
    }

    fn take(&mut self) -> Self {
        Self {
            channels: core::mem::take(&mut self.channels),
            expression_targets: core::mem::take(&mut self.expression_targets),
        }
    }
}

struct Host {
    registry: PropertyRegistry,
    props: Props,
    expressions: ExpressionInvalidation,
    elements: BTreeMap<ElementId, Element>,
    states: BTreeMap<ElementId, MatchState>,
    dirty: DirtyFrame,
}

impl Host {
    fn new(registry: PropertyRegistry, props: Props, expressions: ExpressionInvalidation) -> Self {
        Self {
            registry,
            props,
            expressions,
            elements: BTreeMap::new(),
            states: BTreeMap::new(),
            dirty: DirtyFrame::default(),
        }
    }

    fn insert(&mut self, key: ElementId, parent: Option<ElementId>, state: MatchState) {
        self.elements.insert(key, Element::new(key, parent));
        self.states.insert(key, state);
    }

    fn element(&self, key: ElementId) -> &Element {
        self.elements.get(&key).expect("example element exists")
    }

    fn state(&self, key: ElementId) -> MatchState {
        self.states
            .get(&key)
            .copied()
            .expect("example style state exists")
    }

    fn set_state(&mut self, key: ElementId, state: MatchState) {
        self.states.insert(key, state);
    }

    fn resolve_parent(&self, key: ElementId) -> Option<ResolveParent<'_, ElementId>> {
        let element = self.elements.get(&key)?;
        let state = self.states.get(&key).copied()?;
        Some(ResolveParent::with_match_state(
            element.property_store(),
            element.parent_key(),
            state,
        ))
    }

    fn set_local_source<T>(
        &mut self,
        endpoint: PropertyEndpoint<ElementId, T>,
        value: T,
        source: LocalValueSource,
    ) -> (bool, ChannelSet)
    where
        T: Clone + PartialEq + 'static,
    {
        let property = endpoint.property();
        let changed = {
            let registry = &self.registry;
            let element = self
                .elements
                .get_mut(&endpoint.owner())
                .expect("example element exists");
            let old_effective = element.get_effective_local(property, registry);
            element.set_local_with_source_notifying(property, value, source, registry);
            let new_effective = element.get_effective_local(property, registry);
            old_effective != new_effective
        };

        if changed {
            let channels =
                self.dirty
                    .mark_property_changed(property.id(), &self.registry, &self.expressions);
            (true, channels)
        } else {
            (false, ChannelSet::empty())
        }
    }

    fn set_animation<T>(&mut self, owner: ElementId, property: Property<T>, value: T) -> ChannelSet
    where
        T: Clone + PartialEq + 'static,
    {
        self.elements
            .get_mut(&owner)
            .expect("example element exists")
            .set_animation(property, value);
        self.dirty
            .mark_property_changed(property.id(), &self.registry, &self.expressions)
    }

    fn clear_animation<T>(&mut self, owner: ElementId, property: Property<T>) -> ChannelSet
    where
        T: Clone + PartialEq + 'static,
    {
        let removed = self
            .elements
            .get_mut(&owner)
            .expect("example element exists")
            .clear_animation(property);
        if removed {
            self.dirty
                .mark_property_changed(property.id(), &self.registry, &self.expressions)
        } else {
            ChannelSet::empty()
        }
    }
}

impl BindingHost<ElementId, LocalValueSource> for Host {
    fn get_erased(&self, endpoint: EndpointKey<ElementId>) -> Option<ErasedValue> {
        if endpoint.property() != self.props.scale.id() {
            return None;
        }
        self.elements.get(&endpoint.owner()).map(|element| {
            ErasedValue::new(element.get_effective_local(self.props.scale, &self.registry))
        })
    }

    fn set_erased(
        &mut self,
        endpoint: EndpointKey<ElementId>,
        value: ErasedValue,
        source: LocalValueSource,
    ) -> BindingWrite {
        if endpoint.property() != self.props.scale.id() {
            return BindingWrite::unchanged();
        }
        let Some(value) = value.downcast_ref::<f64>().copied() else {
            return BindingWrite::unchanged();
        };
        let target = PropertyEndpoint::new(endpoint.owner(), self.props.scale);
        let (changed, channels) = self.set_local_source(target, value, source);
        BindingWrite::new(changed, channels)
    }
}

fn main() {
    let model = ElementId(1);
    let root = ElementId(2);
    let card = ElementId(3);

    let mut registry = PropertyRegistry::new();
    let props = Props {
        scale: registry.register("Scale", PropertyMetadataBuilder::new(1.0_f64).build()),
        width: registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .build(),
        ),
        padding: registry.register(
            "Padding",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .build(),
        ),
        accent: registry.register(
            "Accent",
            PropertyMetadataBuilder::new(0_u32)
                .inherits(true)
                .affects_channels(PAINT.into_set())
                .build(),
        ),
    };

    let mut expressions = ExpressionLayer::new();
    expressions.set_default(props.padding, expr::prop(props.scale) * expr::token(GAP));
    expressions.set_default(props.accent, expr::token(ACCENT));

    let base_card = StyleBuilder::new()
        .set_expr(props.width, expr::prop(props.scale) * expr::token(UNIT))
        .build();
    let featured_card = StyleBuilder::new()
        .set_expr(
            props.width,
            expr::prop(props.scale) * expr::token(UNIT) * 1.5,
        )
        .build();
    let expression_index =
        ExpressionInvalidation::from(&expressions, &[&base_card, &featured_card]);

    let cascade = StyleCascadeBuilder::new()
        .push_rule(StyleOrigin::Sheet, SelectorStep::type_tag(CARD), base_card)
        .push_rule(
            StyleOrigin::Sheet,
            SelectorStep::type_tag(CARD).with_pseudo(FEATURED),
            featured_card,
        )
        .build();

    let mut theme = build_theme(8.0, 12.0, 0x3366cc);
    let root_state = cascade.enter_subject(cascade.root_state(), &SelectorInputs::typed(APP));
    let card_state = cascade.enter_subject(root_state, &SelectorInputs::typed(CARD));

    let mut host = Host::new(registry, props, expression_index);
    host.insert(model, None, cascade.root_state());
    host.insert(root, None, root_state);
    host.insert(card, Some(root), card_state);

    let model_scale = PropertyEndpoint::new(model, props.scale);
    let card_scale = PropertyEndpoint::new(card, props.scale);
    host.set_local_source(model_scale, 1.25, LocalValueSource::Local);

    let mut bindings = BindingSet::new(BINDING);
    bindings
        .bind(model_scale, card_scale, LocalValueSource::TemplateBinding)
        .unwrap();

    println!("== first frame: app state flows through binding into style expressions ==");
    bindings.mark_source_changed(model_scale);
    let report = bindings.drain(&mut host).unwrap();
    host.dirty.apply_channels(report.affected_channels());
    print_dirty(&mut host);
    print_resolved(&host, &theme, &expressions, &cascade, card);

    println!();
    println!("== theme frame: resources invalidate expression targets ==");
    theme = build_theme(10.0, 16.0, 0x8844ff);
    host.dirty
        .mark_resource_changed(UNIT, &host.registry, &host.expressions);
    host.dirty
        .mark_resource_changed(GAP, &host.registry, &host.expressions);
    host.dirty
        .mark_resource_changed(ACCENT, &host.registry, &host.expressions);
    print_dirty(&mut host);
    print_resolved(&host, &theme, &expressions, &cascade, card);

    println!();
    println!("== restyle frame: selector state changes the winning style expression ==");
    let featured_inputs = SelectorInputs::typed_with_pseudos(CARD, &[FEATURED]);
    let restyle = cascade.restyle_subject(
        &host.registry,
        host.state(card),
        root_state,
        &featured_inputs,
    );
    host.set_state(card, restyle.state());
    host.dirty.apply_channels(restyle.changed_channels());
    print_dirty(&mut host);
    print_resolved(&host, &theme, &expressions, &cascade, card);

    println!();
    println!("== animation frame: sampled motion writes the animation slot ==");
    let channels = host.set_animation(card, props.width, 320.0);
    println!("  animation sample affected={channels:?}");
    print_dirty(&mut host);
    print_resolved(&host, &theme, &expressions, &cascade, card);

    println!();
    println!("== animation clears: style expression is visible again ==");
    let channels = host.clear_animation(card, props.width);
    println!("  animation clear affected={channels:?}");
    print_dirty(&mut host);
    print_resolved(&host, &theme, &expressions, &cascade, card);
}

fn build_theme(unit: f64, gap: f64, accent: u32) -> Theme {
    ThemeBuilder::new()
        .set(UNIT, unit)
        .set(GAP, gap)
        .set(ACCENT, accent)
        .build()
}

fn print_resolved(
    host: &Host,
    theme: &Theme,
    expressions: &ExpressionLayer,
    cascade: &understory_style::StyleCascade,
    card: ElementId,
) {
    let lookup = |key| host.resolve_parent(key);
    let cx = ResolveCx::new(&host.registry, theme, expressions, lookup);
    let style = Some((cascade, host.state(card)));
    let element = host.element(card);

    let width = cx.explain_value(element, host.props.width, style);
    let padding = cx.explain_value(element, host.props.padding, style);
    let accent = cx.explain_value(element, host.props.accent, style);

    println!("  width={} from {:?}", width.value, width.source);
    println!("  padding={} from {:?}", padding.value, padding.source);
    println!("  accent=#{:06x} from {:?}", accent.value, accent.source);
}

fn print_dirty(host: &mut Host) {
    let dirty = host.dirty.take();
    let targets = dirty
        .expression_targets
        .iter()
        .map(|id| host.registry.name(*id).unwrap_or("<unknown>"))
        .collect::<Vec<_>>();
    println!(
        "  dirty channels={:?} expression_targets={targets:?}",
        dirty.channels
    );
}
