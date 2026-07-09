// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Style-authored transitions driving the dependency-property animation slot.
//!
//! This example shows the styled-transitions pattern from
//! `understory_transition`:
//!
//! 1. A "Transitions" dependency property carries `TransitionDeclarations`,
//!    authored per style rule; the winning rule's whole set applies.
//! 2. A pseudo-class flip re-resolves styled values; changed values are handed
//!    to `TransitionRunner::set_target` together with the after-change
//!    declarations.
//! 3. Late-bound declaration fields (here: the hover duration) resolve through
//!    a theme-backed `TransitionResources` adapter at transition start, so a
//!    theme swap retunes motion without rebuilding styles or declarations.
//! 4. A mid-flight flip back retargets from the current animated value and is
//!    shortened by the CSS reversing shortening factor.
//! 5. Completion clears the animation slot so the styled value shows through.
//!
//! Run:
//! - `cargo run -p understory_examples --example style_transition_loop`

use understory_property::{
    DependencyObject, ErasedValue, Property, PropertyMetadataBuilder, PropertyRegistry,
    PropertyStore,
};
use understory_style::{
    ExpressionLayer, MatchState, NoResolveParentLookup, PseudoClassId, ResolveCx, ResourceKey,
    SelectorInputs, SelectorStep, StyleBuilder, StyleCascade, StyleCascadeBuilder, StyleOrigin,
    Theme, ThemeBuilder, TypeTag,
};
use understory_timing::{TimerDuration, TimerInstant};
use understory_transition::{
    TransitionDeclaration, TransitionDeclarations, TransitionResourceKey, TransitionResources,
    TransitionRunner, TransitionTiming, millis,
};

const CARD: TypeTag = TypeTag(1);
const HOVERED: PseudoClassId = PseudoClassId(1);

/// Theme slot holding the hover transition duration (a `TimerDuration`).
const MOTION_DURATION: ResourceKey = ResourceKey::new(1);
/// The transition-crate token the hover declaration defers its duration to.
const HOVER_DURATION: TransitionResourceKey = TransitionResourceKey::new(1);

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
struct ElementId(u32);

struct Element {
    key: ElementId,
    store: PropertyStore<ElementId>,
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

/// Host adapter mapping transition resource tokens onto the style theme.
struct ThemeMotion<'a> {
    theme: &'a Theme,
}

impl TransitionResources for ThemeMotion<'_> {
    fn duration(&self, key: TransitionResourceKey) -> Option<TimerDuration> {
        match key {
            HOVER_DURATION => self.theme.get::<TimerDuration>(MOTION_DURATION).copied(),
            _ => None,
        }
    }
}

struct Host {
    registry: PropertyRegistry,
    width: Property<f64>,
    transitions: Property<TransitionDeclarations>,
    element: Element,
    state: MatchState,
    runner: TransitionRunner<ElementId>,
}

impl Host {
    fn resolve_width(&self, theme: &Theme, cascade: &StyleCascade) -> f64 {
        let expressions = ExpressionLayer::new();
        let cx = ResolveCx::new(&self.registry, theme, &expressions, NoResolveParentLookup);
        cx.get_value(&self.element, self.width, Some((cascade, self.state)))
    }

    fn resolve_transitions(&self, theme: &Theme, cascade: &StyleCascade) -> TransitionDeclarations {
        let expressions = ExpressionLayer::new();
        let cx = ResolveCx::new(&self.registry, theme, &expressions, NoResolveParentLookup);
        cx.get_value(&self.element, self.transitions, Some((cascade, self.state)))
    }

    /// Flips the pseudo-class state and hands the changed width to the runner.
    fn flip(
        &mut self,
        theme: &Theme,
        cascade: &StyleCascade,
        inputs: &SelectorInputs<'_>,
        now: TimerInstant,
    ) {
        let before = self.resolve_width(theme, cascade);
        let restyle =
            cascade.restyle_subject(&self.registry, self.state, cascade.root_state(), inputs);
        self.state = restyle.state();

        // The transition runner owns this property's animation entry, and it
        // remembers the in-flight value itself, so the host may drop the mask
        // to read the after-change *styled* target through the normal
        // resolver. The retargeted transition returns the replacement write
        // immediately, so the host can apply it before presenting this update.
        self.element.store.clear_animation(self.width);
        let after = self.resolve_width(theme, cascade);

        // Transition parameters come from the after-change declarations and
        // resolve against the current theme now, at transition start.
        let declarations = self.resolve_transitions(theme, cascade);
        let update = self
            .runner
            .set_target(
                self.element.key,
                self.width.id(),
                &declarations,
                &ThemeMotion { theme },
                &ErasedValue::new(before),
                ErasedValue::new(after),
                now,
            )
            .unwrap();
        let outcome = update.outcome();
        if let Some(write) = update.into_immediate_write() {
            write.apply(&mut self.element.store);
        }

        println!(
            "  t={}ms width {before:.1} -> {after:.1} ({outcome:?})",
            ms(now)
        );
        for info in self.runner.active(now) {
            println!(
                "    {:?} duration={}ms shortening={:.2} ends at t={}ms",
                info.phase(),
                ms(info.duration()),
                info.reversing_shortening(),
                ms(info.end_time()),
            );
        }
    }

    /// Samples the runner and applies the write plan to the property store.
    fn frame(&mut self, theme: &Theme, cascade: &StyleCascade, now: TimerInstant) {
        for write in self.runner.sample(now) {
            write.apply(&mut self.element.store);
        }
        let width = self.resolve_width(theme, cascade);
        let masked = self.element.store.has_animation(self.width);
        println!(
            "  t={}ms width={width:.1} (animation slot {})",
            ms(now),
            if masked { "set" } else { "clear" },
        );
    }
}

fn ms(ticks: u64) -> u64 {
    ticks / 1_000_000
}

fn main() {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
    let transitions = registry.register(
        "Transitions",
        PropertyMetadataBuilder::new(TransitionDeclarations::new()).build(),
    );

    // Styles author both the values and the transitions, CSS-style. The base
    // rule uses a literal 200ms; the hover rule defers its duration to a
    // theme token resolved at transition start.
    let base_rule = StyleBuilder::new()
        .set(width, 40.0)
        .set(
            transitions,
            TransitionDeclarations::new().with(TransitionDeclaration::of(
                width,
                TransitionTiming::new(millis(200)),
            )),
        )
        .build();
    let hover_rule = StyleBuilder::new()
        .set(width, 96.0)
        .set(
            transitions,
            TransitionDeclarations::new().with(
                TransitionDeclaration::of(width, TransitionTiming::new(0))
                    .with_duration_resource(HOVER_DURATION),
            ),
        )
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(StyleOrigin::Sheet, SelectorStep::type_tag(CARD), base_rule)
        .push_rule(
            StyleOrigin::Sheet,
            SelectorStep::type_tag(CARD).with_pseudo(HOVERED),
            hover_rule,
        )
        .build();

    let base_inputs = SelectorInputs::typed(CARD);
    let hover_inputs = SelectorInputs::typed_with_pseudos(CARD, &[HOVERED]);

    let theme = ThemeBuilder::new()
        .set(MOTION_DURATION, millis(300))
        .build();

    let element = Element {
        key: ElementId(1),
        store: PropertyStore::new(ElementId(1)),
    };
    let state = cascade.enter_subject(cascade.root_state(), &base_inputs);
    let mut host = Host {
        registry,
        width,
        transitions,
        element,
        state,
        runner: TransitionRunner::new(),
    };

    println!("== hover on: duration comes from the theme token ==");
    host.flip(&theme, &cascade, &hover_inputs, 0);
    for now in [0, millis(100), millis(150)] {
        host.frame(&theme, &cascade, now);
    }

    println!();
    println!("== hover off at 50%: reversal shortens the way back ==");
    host.flip(&theme, &cascade, &base_inputs, millis(150));
    for now in [millis(150), millis(200), millis(250)] {
        host.frame(&theme, &cascade, now);
    }

    println!();
    println!("== theme swap retunes motion with no style rebuild ==");
    let snappy = ThemeBuilder::new()
        .set(MOTION_DURATION, millis(120))
        .build();
    host.flip(&snappy, &cascade, &hover_inputs, millis(1_000));
    for now in [millis(1_060), millis(1_120)] {
        host.frame(&snappy, &cascade, now);
    }
}
