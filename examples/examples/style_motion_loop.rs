// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Style target resolution feeding a motion transition.
//!
//! This example shows the intended boundary between presentation policy and
//! motion:
//!
//! 1. Style expressions resolve the target value for a property.
//! 2. `understory_motion` samples intermediate values over time.
//! 3. The host writes each sample into the dependency property's animation slot.
//! 4. `ResolveCx` sees the animation slot first while the transition is active.
//! 5. Clearing the animation slot reveals the style target again.
//!
//! Run:
//! - `cargo run -p understory_examples --example style_motion_loop`

use invalidation::Channel;
use understory_motion::{TimingFunction, Transition, millis};
use understory_property::{
    DependencyObject, Property, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
};
use understory_style::{
    ExpressionLayer, NoResolveParentLookup, PseudoClassId, ResolveCx, Resolved, ResourceKey,
    SelectorInputs, SelectorStep, StyleBuilder, StyleCascade, StyleCascadeBuilder, StyleOrigin,
    Theme, ThemeBuilder, TypeTag, expr,
};

const LAYOUT: Channel = Channel::new(1);

const CARD: TypeTag = TypeTag(1);
const EXPANDED: PseudoClassId = PseudoClassId(1);
const UNIT: ResourceKey = ResourceKey::new(1);

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
struct ElementId(u32);

#[derive(Copy, Clone)]
struct Props {
    scale: Property<f64>,
    width: Property<f64>,
}

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

struct Host {
    registry: PropertyRegistry,
    props: Props,
    element: Element,
    state: understory_style::MatchState,
}

impl Host {
    fn resolve_width(
        &self,
        theme: &Theme,
        expressions: &ExpressionLayer,
        cascade: &StyleCascade,
    ) -> Resolved<f64> {
        let cx = ResolveCx::new(&self.registry, theme, expressions, NoResolveParentLookup);
        cx.explain_value(&self.element, self.props.width, Some((cascade, self.state)))
    }

    fn set_animation_width(&mut self, value: f64) -> invalidation::ChannelSet {
        self.element.store.set_animation(self.props.width, value);
        self.registry.affects_channels(self.props.width.id())
    }

    fn clear_animation_width(&mut self) -> invalidation::ChannelSet {
        if self.element.store.clear_animation(self.props.width) {
            self.registry.affects_channels(self.props.width.id())
        } else {
            invalidation::ChannelSet::empty()
        }
    }
}

fn main() {
    let mut registry = PropertyRegistry::new();
    let props = Props {
        scale: registry.register("Scale", PropertyMetadataBuilder::new(1.0_f64).build()),
        width: registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .build(),
        ),
    };

    let theme = ThemeBuilder::new().set(UNIT, 80.0_f64).build();
    let expressions = ExpressionLayer::new();

    let base_card = StyleBuilder::new()
        .set_expr(props.width, expr::prop(props.scale) * expr::token(UNIT))
        .build();
    let expanded_card = StyleBuilder::new()
        .set_expr(
            props.width,
            expr::prop(props.scale) * expr::token(UNIT) * 1.5,
        )
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(StyleOrigin::Sheet, SelectorStep::type_tag(CARD), base_card)
        .push_rule(
            StyleOrigin::Sheet,
            SelectorStep::type_tag(CARD).with_pseudo(EXPANDED),
            expanded_card,
        )
        .build();

    let base_inputs = SelectorInputs::typed(CARD);
    let base_state = cascade.enter_subject(cascade.root_state(), &base_inputs);
    let expanded_inputs = SelectorInputs::typed_with_pseudos(CARD, &[EXPANDED]);

    let mut element = Element::new(ElementId(1));
    element.store.set_local(props.scale, 1.25);

    let mut host = Host {
        registry,
        props,
        element,
        state: base_state,
    };

    println!("== initial style target ==");
    let initial = host.resolve_width(&theme, &expressions, &cascade);
    print_resolved("base target", &initial);

    println!();
    println!("== restyle chooses a new target ==");
    let restyle = cascade.restyle_subject(
        &host.registry,
        host.state,
        cascade.root_state(),
        &expanded_inputs,
    );
    host.state = restyle.state();
    println!(
        "  restyle changed_properties={:?} channels={:?}",
        restyle.changed_properties().property_ids(),
        restyle.changed_channels()
    );
    let target = host.resolve_width(&theme, &expressions, &cascade);
    print_resolved("expanded target", &target);

    println!();
    println!("== motion samples write the animation slot ==");
    let transition = Transition::new(
        initial.value,
        target.value,
        0,
        millis(240),
        TimingFunction::cubic_bezier(0.215, 0.61, 0.355, 1.0),
    );

    for now in [0, millis(80), millis(160), transition.end_time()] {
        let sample = transition.sample(now);
        let channels = host.set_animation_width(sample);
        let resolved = host.resolve_width(&theme, &expressions, &cascade);
        println!(
            "  t={}ms sample={sample:.2} resolved={:.2} source={:?} channels={channels:?}",
            now / 1_000_000,
            resolved.value,
            resolved.source
        );
    }

    println!();
    println!("== animation clears after completion ==");
    let channels = host.clear_animation_width();
    let settled = host.resolve_width(&theme, &expressions, &cascade);
    println!("  clear channels={channels:?}");
    print_resolved("settled target", &settled);
}

fn print_resolved(label: &str, resolved: &Resolved<f64>) {
    println!(
        "  {label}: value={:.2} source={:?}",
        resolved.value, resolved.source
    );
}
