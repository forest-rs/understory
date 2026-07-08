// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;
use crate::{ExpressionLayer, StyleBuilder, ThemeBuilder, expr};
use crate::{StyleCascadeBuilder, StyleOrigin};
use crate::{ThemeScope, ThemeVariant, VariantThemeBuilder};
use alloc::collections::BTreeMap;
use alloc::vec;
use understory_property::{LocalValueSource, PropertyMetadataBuilder, PropertyStore};
use understory_property_expression::{Expr, ExprResourceKey, ExpressionDefaults, FunctionRegistry};

struct TestElement {
    key: u32,
    parent: Option<u32>,
    store: PropertyStore<u32>,
}

impl TestElement {
    fn new(key: u32, parent: Option<u32>) -> Self {
        Self {
            key,
            parent,
            store: PropertyStore::new(key),
        }
    }
}

impl DependencyObject<u32> for TestElement {
    fn property_store(&self) -> &PropertyStore<u32> {
        &self.store
    }

    fn property_store_mut(&mut self) -> &mut PropertyStore<u32> {
        &mut self.store
    }

    fn key(&self) -> u32 {
        self.key
    }

    fn parent_key(&self) -> Option<u32> {
        self.parent
    }
}

#[test]
fn resolve_local_value() {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let theme = ThemeBuilder::new().build();

    let mut element = TestElement::new(1, None);
    element.store.set_local(width, 100.0);

    let expressions = ExpressionLayer::new();
    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    let value = cx.get_value(&element, width, None);
    assert_eq!(value, 100.0);
}

#[test]
fn resolve_animation_over_local() {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let theme = ThemeBuilder::new().build();

    let mut element = TestElement::new(1, None);
    element.store.set_local(width, 100.0);
    element.store.set_animation(width, 200.0);

    let expressions = ExpressionLayer::new();
    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    let value = cx.get_value(&element, width, None);
    assert_eq!(value, 200.0);
}

#[test]
fn resolve_local_over_style() {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let theme = ThemeBuilder::new().build();
    let style = StyleBuilder::new().set(width, 50.0).build();
    let style = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Override, style)
        .build();

    let mut element = TestElement::new(1, None);
    element.store.set_local(width, 100.0);

    let expressions = ExpressionLayer::new();
    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    let value = cx.get_value(&element, width, Some((&style, style.root_state())));
    assert_eq!(value, 100.0);
}

#[test]
fn resolve_style_value() {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let theme = ThemeBuilder::new().build();
    let style = StyleBuilder::new().set(width, 50.0).build();
    let style = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Override, style)
        .build();

    let element = TestElement::new(1, None);

    let expressions = ExpressionLayer::new();
    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    let value = cx.get_value(&element, width, Some((&style, style.root_state())));
    assert_eq!(value, 50.0);
}

#[test]
fn resolve_default_value() {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(42.0_f64).build());

    let theme = ThemeBuilder::new().build();
    let element = TestElement::new(1, None);

    let expressions = ExpressionLayer::new();
    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    let value = cx.get_value(&element, width, None);
    assert_eq!(value, 42.0);
}

#[test]
fn expression_default_resolves_theme_token_and_reports_source() {
    const WIDTH_TOKEN: ResourceKey = ResourceKey::new(7);

    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let mut expressions = ExpressionLayer::new();
    expressions.set_default(width, expr::token(WIDTH_TOKEN));

    let theme = ThemeBuilder::new().set(WIDTH_TOKEN, 42.0_f64).build();
    let element = TestElement::new(1, None);
    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);

    assert_eq!(cx.get_value(&element, width, None), 42.0);
    assert_eq!(
        cx.explain_value(&element, width, None),
        Resolved {
            value: 42.0,
            source: ResolvedSource::DefaultExpression {
                property: width.id(),
            },
        }
    );
}

#[test]
fn expression_default_dependencies_use_current_style_and_theme() {
    const OFFSET_TOKEN: ResourceKey = ResourceKey::new(8);

    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
    let scale = registry.register("Scale", PropertyMetadataBuilder::new(1.0_f64).build());

    let mut expressions = ExpressionLayer::new();
    expressions.set_default(width, expr::prop(scale) * 2.0 + expr::token(OFFSET_TOKEN));

    let theme = ThemeBuilder::new().set(OFFSET_TOKEN, 2.0_f64).build();
    let style = StyleBuilder::new().set(scale, 5.0).build();
    let cascade = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Override, style)
        .build();
    let element = TestElement::new(1, None);
    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);

    assert_eq!(
        cx.query(&element, width)
            .style(&cascade, cascade.root_state())
            .try_value(),
        Ok(12.0)
    );
}

#[test]
fn query_resource_fallback_precedes_expression_default() {
    const FALLBACK_TOKEN: ResourceKey = ResourceKey::new(18);
    const DEFAULT_TOKEN: ResourceKey = ResourceKey::new(19);

    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let mut expressions = ExpressionLayer::new();
    expressions.set_default(width, expr::token(DEFAULT_TOKEN));

    let theme = ThemeBuilder::new()
        .set(FALLBACK_TOKEN, 80.0_f64)
        .set(DEFAULT_TOKEN, 40.0_f64)
        .build();
    let element = TestElement::new(1, None);
    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);

    let resolved = cx
        .query(&element, width)
        .resource_fallback(FALLBACK_TOKEN)
        .try_explain()
        .unwrap();

    assert_eq!(resolved.value, 80.0);
    assert_eq!(
        resolved.source,
        ResolvedSource::ThemeResource {
            key: FALLBACK_TOKEN,
        }
    );
}

#[test]
fn style_expression_evaluates_against_current_style_and_theme() {
    const OFFSET_TOKEN: ResourceKey = ResourceKey::new(12);

    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
    let scale = registry.register("Scale", PropertyMetadataBuilder::new(1.0_f64).build());

    let expressions = ExpressionLayer::new();
    let style = StyleBuilder::new()
        .set(scale, 5.0)
        .set_expr(width, expr::prop(scale) * 2.0 + expr::token(OFFSET_TOKEN))
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Override, style)
        .build();
    let theme = ThemeBuilder::new().set(OFFSET_TOKEN, 2.0_f64).build();
    let element = TestElement::new(1, None);
    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);

    assert_eq!(
        cx.query(&element, width)
            .style(&cascade, cascade.root_state())
            .try_value(),
        Ok(12.0)
    );

    let explained = cx
        .query(&element, width)
        .style(&cascade, cascade.root_state())
        .try_explain()
        .unwrap();
    assert_eq!(explained.value, 12.0);
    let ResolvedSource::CascadeDirect {
        origin,
        source_index,
        resource,
        expression,
    } = explained.source
    else {
        panic!("expected direct style expression provenance");
    };
    assert_eq!(origin, StyleOrigin::Override);
    assert_eq!(source_index, 0);
    assert_eq!(resource, None);
    assert_eq!(expression.unwrap().index(), 0);
}

#[test]
fn local_value_beats_style_expression() {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let expressions = ExpressionLayer::new();
    let style = StyleBuilder::new().set_expr(width, expr::lit(12.0)).build();
    let cascade = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Override, style)
        .build();
    let theme = ThemeBuilder::new().build();
    let mut element = TestElement::new(1, None);
    element.store.set_local(width, 20.0);
    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);

    assert_eq!(
        cx.query(&element, width)
            .style(&cascade, cascade.root_state())
            .try_value(),
        Ok(20.0)
    );
}

#[test]
fn style_expression_beats_inherited_and_default_expression() {
    let mut registry = PropertyRegistry::new();
    let width = registry.register(
        "Width",
        PropertyMetadataBuilder::new(1.0_f64).inherits(true).build(),
    );

    let mut expressions = ExpressionLayer::new();
    expressions.set_default(width, expr::lit(99.0));
    let style = StyleBuilder::new().set_expr(width, expr::lit(24.0)).build();
    let cascade = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Override, style)
        .build();
    let theme = ThemeBuilder::new().build();
    let mut parent = TestElement::new(1, None);
    parent.store.set_local(width, 12.0);
    let child = TestElement::new(2, Some(1));
    let elements: BTreeMap<u32, &TestElement> = [(1, &parent), (2, &child)].into_iter().collect();
    let cx = ResolveCx::new(
        &registry,
        &theme,
        &expressions,
        PropertyParentLookup::new(|key| {
            elements
                .get(&key)
                .map(|e| (e.property_store(), e.parent_key()))
        }),
    );

    assert_eq!(
        cx.query(&child, width)
            .style(&cascade, cascade.root_state())
            .try_value(),
        Ok(24.0)
    );
}

#[test]
fn inherited_style_expression_resolves_on_ancestor_subject() {
    use crate::{SelectorInputs, SelectorStep, TypeTag};

    const PARENT: TypeTag = TypeTag(1);
    const CHILD: TypeTag = TypeTag(2);

    let mut registry = PropertyRegistry::new();
    let font_size = registry.register(
        "FontSize",
        PropertyMetadataBuilder::new(12.0_f64)
            .inherits(true)
            .build(),
    );
    let scale = registry.register("Scale", PropertyMetadataBuilder::new(1.0_f64).build());

    let expressions = ExpressionLayer::new();
    let style = StyleBuilder::new()
        .set(scale, 8.0)
        .set_expr(font_size, expr::prop(scale) * 2.0)
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(StyleOrigin::Override, SelectorStep::type_tag(PARENT), style)
        .build();
    let theme = ThemeBuilder::new().build();
    let parent = TestElement::new(1, None);
    let child = TestElement::new(2, Some(1));
    let parent_state = cascade.enter_subject(cascade.root_state(), &SelectorInputs::typed(PARENT));
    let child_state = cascade.enter_subject(parent_state, &SelectorInputs::typed(CHILD));
    let elements: BTreeMap<u32, (&TestElement, MatchState)> =
        [(1, (&parent, parent_state)), (2, (&child, parent_state))]
            .into_iter()
            .collect();
    let cx = ResolveCx::new(&registry, &theme, &expressions, |key| {
        elements.get(&key).map(|(element, state)| {
            ResolveParent::with_match_state(element.property_store(), element.parent_key(), *state)
        })
    });

    assert_eq!(
        cx.query(&child, font_size)
            .style(&cascade, child_state)
            .try_value(),
        Ok(16.0)
    );
}

#[test]
fn class_style_expression_option_value_inherits_to_descendant() {
    use crate::{ClassId, SelectorInputs, SelectorStep, TypeTag};

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum BadgeHint {
        Featured,
    }

    const FEATURED_CARD: ClassId = ClassId(1);
    const CARD: TypeTag = TypeTag(1);
    const BADGE: TypeTag = TypeTag(2);

    let mut registry = PropertyRegistry::new();
    let featured = registry.register("Featured", PropertyMetadataBuilder::new(false).build());
    let badge_hint = registry.register(
        "BadgeHint",
        PropertyMetadataBuilder::new(None::<BadgeHint>)
            .inherits(true)
            .build(),
    );

    let selector = SelectorStep::class(FEATURED_CARD);
    let style = StyleBuilder::new()
        .set(featured, true)
        .set_expr(
            badge_hint,
            expr::cond(
                expr::prop(featured),
                expr::lit(Some(BadgeHint::Featured)),
                expr::lit(None::<BadgeHint>),
            ),
        )
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(StyleOrigin::Sheet, selector.clone(), style)
        .build();
    let theme = ThemeBuilder::new().build();
    let expressions = ExpressionLayer::new();

    let card = TestElement::new(1, None);
    let badge = TestElement::new(2, Some(1));
    let classes = [FEATURED_CARD];
    let card_state = cascade.enter_subject(
        cascade.root_state(),
        &SelectorInputs::new(Some(CARD), &classes, &[]),
    );
    let badge_state = cascade.enter_subject(card_state, &SelectorInputs::typed(BADGE));
    let elements: BTreeMap<u32, (&TestElement, MatchState)> =
        [(1, (&card, card_state)), (2, (&badge, badge_state))]
            .into_iter()
            .collect();
    let cx = ResolveCx::new(&registry, &theme, &expressions, |key| {
        elements.get(&key).map(|(element, state)| {
            ResolveParent::with_match_state(element.property_store(), element.parent_key(), *state)
        })
    });

    let resolved = cx
        .query(&badge, badge_hint)
        .style(&cascade, badge_state)
        .try_explain()
        .unwrap();

    assert_eq!(resolved.value, Some(BadgeHint::Featured));
    let ResolvedSource::Inherited {
        ancestor_depth,
        inner,
    } = resolved.source
    else {
        panic!("expected inherited badge hint");
    };
    assert_eq!(ancestor_depth, 1);
    let ResolvedSource::CascadeRule {
        selector: source_selector,
        resource,
        expression,
        ..
    } = *inner
    else {
        panic!("expected inherited cascade expression source");
    };
    assert_eq!(source_selector, selector.into());
    assert_eq!(resource, None);
    assert_eq!(expression.unwrap().index(), 0);
}

#[test]
fn winning_style_expression_error_masks_lower_style_sources() {
    const MISSING_TOKEN: ResourceKey = ResourceKey::new(13);

    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let expressions = ExpressionLayer::new();
    let lower = StyleBuilder::new().set(width, 5.0).build();
    let higher = StyleBuilder::new()
        .set_expr(width, expr::token(MISSING_TOKEN))
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Base, lower)
        .push_style(StyleOrigin::Override, higher)
        .build();
    let theme = ThemeBuilder::new().build();
    let element = TestElement::new(1, None);
    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);

    assert_eq!(
        cx.query(&element, width)
            .style(&cascade, cascade.root_state())
            .try_value(),
        Err(ExprError::MissingResource {
            key: expr_key(MISSING_TOKEN),
        })
    );
}

#[test]
fn normal_resolution_evaluates_style_expression() {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(3.0_f64).build());

    let style = StyleBuilder::new().set_expr(width, expr::lit(42.0)).build();
    let cascade = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Override, style)
        .build();
    let theme = ThemeBuilder::new().build();
    let expressions = ExpressionLayer::new();
    let element = TestElement::new(1, None);
    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);

    assert_eq!(cascade.get_value_ref(cascade.root_state(), width), None);
    assert_eq!(cascade.get_entry_ref(cascade.root_state(), width), None);
    assert_eq!(
        cx.get_value(&element, width, Some((&cascade, cascade.root_state()))),
        42.0
    );
}

#[test]
fn local_and_inherited_values_precede_expression_default() {
    const FONT_TOKEN: ResourceKey = ResourceKey::new(9);

    let mut registry = PropertyRegistry::new();
    let font_size = registry.register(
        "FontSize",
        PropertyMetadataBuilder::new(12.0_f64)
            .inherits(true)
            .build(),
    );

    let mut defaults = ExpressionDefaults::new();
    defaults.set(font_size, Expr::<f64>::token(expr_key(FONT_TOKEN)));
    let functions = FunctionRegistry::with_builtins();
    let theme = ThemeBuilder::new().set(FONT_TOKEN, 14.0_f64).build();

    let mut parent = TestElement::new(1, None);
    parent.store.set_local(font_size, 16.0);
    let mut child = TestElement::new(2, Some(1));
    let elements: BTreeMap<u32, &TestElement> = [(1, &parent), (2, &child)].into_iter().collect();
    let options = ExprResolveOptions::new(&defaults, &functions);
    let cx = ResolveCx::new(
        &registry,
        &theme,
        options,
        PropertyParentLookup::new(|key| {
            elements
                .get(&key)
                .map(|e| (e.property_store(), e.parent_key()))
        }),
    );

    assert_eq!(cx.get_value(&child, font_size, None), 16.0);

    child.store.set_local(font_size, 20.0);
    let cx = ResolveCx::new(&registry, &theme, options, NoResolveParentLookup);
    assert_eq!(cx.get_value(&child, font_size, None), 20.0);
}

#[test]
fn expression_default_reflects_theme_swaps() {
    const WIDTH_TOKEN: ResourceKey = ResourceKey::new(10);

    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let mut defaults = ExpressionDefaults::new();
    defaults.set(width, Expr::<f64>::token(expr_key(WIDTH_TOKEN)));
    let functions = FunctionRegistry::with_builtins();
    let element = TestElement::new(1, None);

    let light = ThemeBuilder::new().set(WIDTH_TOKEN, 10.0_f64).build();
    let dark = ThemeBuilder::new().set(WIDTH_TOKEN, 12.0_f64).build();
    let options = ExprResolveOptions::new(&defaults, &functions);
    let light_cx = ResolveCx::new(&registry, &light, options, NoResolveParentLookup);
    let dark_cx = ResolveCx::new(&registry, &dark, options, NoResolveParentLookup);

    assert_eq!(light_cx.get_value(&element, width, None), 10.0);
    assert_eq!(dark_cx.get_value(&element, width, None), 12.0);
}

#[test]
fn expression_default_reports_missing_and_wrong_resource_types() {
    const WIDTH_TOKEN: ResourceKey = ResourceKey::new(11);

    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let mut defaults = ExpressionDefaults::new();
    defaults.set(width, Expr::<f64>::token(expr_key(WIDTH_TOKEN)));
    let functions = FunctionRegistry::with_builtins();
    let element = TestElement::new(1, None);
    let options = ExprResolveOptions::new(&defaults, &functions);

    let missing_theme = ThemeBuilder::new().build();
    let missing_cx = ResolveCx::new(&registry, &missing_theme, options, NoResolveParentLookup);
    assert_eq!(
        missing_cx.query(&element, width).try_value(),
        Err(ExprError::MissingResource {
            key: expr_key(WIDTH_TOKEN),
        })
    );

    let wrong_theme = ThemeBuilder::new().set(WIDTH_TOKEN, true).build();
    let wrong_cx = ResolveCx::new(&registry, &wrong_theme, options, NoResolveParentLookup);
    assert_eq!(
        wrong_cx.query(&element, width).try_value(),
        Err(ExprError::TypeMismatch {
            expected: TypeId::of::<f64>(),
            actual: TypeId::of::<bool>(),
        })
    );
}

#[test]
fn expression_default_reports_cycles_with_stack() {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
    let height = registry.register("Height", PropertyMetadataBuilder::new(0.0_f64).build());

    let mut defaults = ExpressionDefaults::new();
    defaults.set(width, Expr::property(height));
    defaults.set(height, Expr::property(width));
    let functions = FunctionRegistry::with_builtins();

    let theme = ThemeBuilder::new().build();
    let element = TestElement::new(1, None);
    let cx = ResolveCx::new(
        &registry,
        &theme,
        ExprResolveOptions::new(&defaults, &functions),
        NoResolveParentLookup,
    );

    assert_eq!(
        cx.query(&element, width).try_value(),
        Err(ExprError::Cycle {
            property: width.id(),
            stack: vec![width.id(), height.id()],
        })
    );
}

#[test]
fn resolve_inherited_value() {
    let mut registry = PropertyRegistry::new();
    let font_size = registry.register(
        "FontSize",
        PropertyMetadataBuilder::new(12.0_f64)
            .inherits(true)
            .build(),
    );

    let theme = ThemeBuilder::new().build();
    let expressions = ExpressionLayer::new();

    let mut parent = TestElement::new(1, None);
    parent.store.set_local(font_size, 16.0);

    let child = TestElement::new(2, Some(1));

    let elements: BTreeMap<u32, &TestElement> = [(1, &parent), (2, &child)].into_iter().collect();

    let cx = ResolveCx::new(
        &registry,
        &theme,
        &expressions,
        PropertyParentLookup::new(|key| {
            elements
                .get(&key)
                .map(|e| (e.property_store(), e.parent_key()))
        }),
    );

    let value = cx.get_value(&child, font_size, None);
    assert_eq!(value, 16.0);
}

fn expr_key(key: ResourceKey) -> ExprResourceKey {
    ExprResourceKey::new(key.index())
}

#[test]
fn resolve_local_over_inherited() {
    let mut registry = PropertyRegistry::new();
    let font_size = registry.register(
        "FontSize",
        PropertyMetadataBuilder::new(12.0_f64)
            .inherits(true)
            .build(),
    );

    let theme = ThemeBuilder::new().build();
    let expressions = ExpressionLayer::new();

    let mut parent = TestElement::new(1, None);
    parent.store.set_local(font_size, 16.0);

    let mut child = TestElement::new(2, Some(1));
    child.store.set_local(font_size, 20.0);

    let elements: BTreeMap<u32, &TestElement> = [(1, &parent), (2, &child)].into_iter().collect();

    let cx = ResolveCx::new(
        &registry,
        &theme,
        &expressions,
        PropertyParentLookup::new(|key| {
            elements
                .get(&key)
                .map(|e| (e.property_store(), e.parent_key()))
        }),
    );

    let value = cx.get_value(&child, font_size, None);
    assert_eq!(value, 20.0);
}

#[test]
fn resolve_style_over_inherited() {
    let mut registry = PropertyRegistry::new();
    let font_size = registry.register(
        "FontSize",
        PropertyMetadataBuilder::new(12.0_f64)
            .inherits(true)
            .build(),
    );

    let theme = ThemeBuilder::new().build();
    let expressions = ExpressionLayer::new();
    let style = StyleBuilder::new().set(font_size, 18.0).build();
    let style = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Override, style)
        .build();

    let mut parent = TestElement::new(1, None);
    parent.store.set_local(font_size, 16.0);

    let child = TestElement::new(2, Some(1));

    let elements: BTreeMap<u32, &TestElement> = [(1, &parent), (2, &child)].into_iter().collect();

    let cx = ResolveCx::new(
        &registry,
        &theme,
        &expressions,
        PropertyParentLookup::new(|key| {
            elements
                .get(&key)
                .map(|e| (e.property_store(), e.parent_key()))
        }),
    );

    let value = cx.get_value(&child, font_size, Some((&style, style.root_state())));
    assert_eq!(value, 18.0);
}

#[test]
fn resolve_inherited_value_from_parent_style_state() {
    use crate::{SelectorInputs, SelectorStep, TypeTag};

    const BUTTON: TypeTag = TypeTag(1);
    const TEXT: TypeTag = TypeTag(2);

    struct StyledLookup<'a> {
        entries: &'a [(u32, &'a TestElement, MatchState)],
    }

    impl<'a> ResolveParentLookup<'a, u32> for StyledLookup<'a> {
        fn lookup_resolve_parent(&self, key: u32) -> Option<ResolveParent<'a, u32>> {
            self.entries
                .iter()
                .find(|(entry_key, _, _)| *entry_key == key)
                .map(|(_, element, match_state)| {
                    ResolveParent::with_match_state(
                        element.property_store(),
                        element.parent_key(),
                        *match_state,
                    )
                })
        }
    }

    let mut registry = PropertyRegistry::new();
    let foreground = registry.register(
        "Foreground",
        PropertyMetadataBuilder::new(0_u32).inherits(true).build(),
    );

    let theme = ThemeBuilder::new().build();
    let expressions = ExpressionLayer::new();
    let button_style = StyleBuilder::new().set(foreground, 0xff_ff_ff_u32).build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(
            StyleOrigin::Base,
            SelectorStep::type_tag(BUTTON),
            button_style,
        )
        .build();

    let button = TestElement::new(1, None);
    let text = TestElement::new(2, Some(1));
    let button_state = cascade.enter_subject(cascade.root_state(), &SelectorInputs::typed(BUTTON));
    let text_state = cascade.enter_subject(button_state, &SelectorInputs::typed(TEXT));

    let plain_elements: BTreeMap<u32, &TestElement> =
        [(1, &button), (2, &text)].into_iter().collect();
    let plain_cx = ResolveCx::new(
        &registry,
        &theme,
        &expressions,
        PropertyParentLookup::new(|key| {
            plain_elements
                .get(&key)
                .map(|e| (e.property_store(), e.parent_key()))
        }),
    );
    assert_eq!(
        plain_cx.get_value(&text, foreground, Some((&cascade, text_state))),
        0,
        "style-blind parent lookup preserves property-only inheritance"
    );

    let entries = [(1, &button, button_state), (2, &text, text_state)];
    let styled_cx = ResolveCx::new(
        &registry,
        &theme,
        &expressions,
        StyledLookup { entries: &entries },
    );
    assert_eq!(
        styled_cx.get_value(&text, foreground, Some((&cascade, text_state))),
        0xff_ff_ff,
        "style-aware parent lookup should inherit the styled button foreground"
    );
}

#[test]
fn query_resource_fallback() {
    use crate::ResourceKey;

    const ACCENT_WIDTH: ResourceKey = ResourceKey::new(0);

    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let theme = ThemeBuilder::new().set(ACCENT_WIDTH, 75.0_f64).build();
    let expressions = ExpressionLayer::new();
    let element = TestElement::new(1, None);

    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    let value = cx
        .query(&element, width)
        .resource_fallback(ACCENT_WIDTH)
        .try_value()
        .unwrap();
    assert_eq!(value, 75.0);
}

#[test]
fn resolve_local_over_resource_fallback() {
    use crate::ResourceKey;

    const ACCENT_WIDTH: ResourceKey = ResourceKey::new(0);

    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let theme = ThemeBuilder::new().set(ACCENT_WIDTH, 75.0_f64).build();
    let expressions = ExpressionLayer::new();
    let mut element = TestElement::new(1, None);
    element.store.set_local(width, 100.0);

    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    let value = cx
        .query(&element, width)
        .resource_fallback(ACCENT_WIDTH)
        .try_value()
        .unwrap();
    assert_eq!(value, 100.0);
}

#[test]
fn resolve_style_over_resource_fallback() {
    use crate::ResourceKey;

    const ACCENT_WIDTH: ResourceKey = ResourceKey::new(0);

    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let theme = ThemeBuilder::new().set(ACCENT_WIDTH, 75.0_f64).build();
    let expressions = ExpressionLayer::new();
    let style = StyleBuilder::new().set(width, 50.0).build();
    let style = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Override, style)
        .build();
    let element = TestElement::new(1, None);

    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    let value = cx
        .query(&element, width)
        .style(&style, style.root_state())
        .resource_fallback(ACCENT_WIDTH)
        .try_value()
        .unwrap();
    assert_eq!(value, 50.0);
}

#[test]
fn resolve_style_resource_over_resource_fallback() {
    use crate::ResourceKey;

    const THEME_FALLBACK: ResourceKey = ResourceKey::new(0);
    const STYLE_TOKEN: ResourceKey = ResourceKey::new(1);

    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let theme = ThemeBuilder::new()
        .set(THEME_FALLBACK, 75.0_f64)
        .set(STYLE_TOKEN, 50.0_f64)
        .build();
    let expressions = ExpressionLayer::new();
    let style = StyleBuilder::new().set_resource(width, STYLE_TOKEN).build();
    let style = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Override, style)
        .build();
    let element = TestElement::new(1, None);

    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    let value = cx
        .query(&element, width)
        .style(&style, style.root_state())
        .resource_fallback(THEME_FALLBACK)
        .try_value()
        .unwrap();
    assert_eq!(value, 50.0);
}

#[test]
fn resolve_resource_fallback_over_inherited() {
    use crate::ResourceKey;

    const ACCENT_SIZE: ResourceKey = ResourceKey::new(0);

    let mut registry = PropertyRegistry::new();
    let font_size = registry.register(
        "FontSize",
        PropertyMetadataBuilder::new(12.0_f64)
            .inherits(true)
            .build(),
    );

    let theme = ThemeBuilder::new().set(ACCENT_SIZE, 18.0_f64).build();
    let expressions = ExpressionLayer::new();

    let mut parent = TestElement::new(1, None);
    parent.store.set_local(font_size, 16.0);

    let child = TestElement::new(2, Some(1));

    let elements: BTreeMap<u32, &TestElement> = [(1, &parent), (2, &child)].into_iter().collect();

    let cx = ResolveCx::new(
        &registry,
        &theme,
        &expressions,
        PropertyParentLookup::new(|key| {
            elements
                .get(&key)
                .map(|e| (e.property_store(), e.parent_key()))
        }),
    );

    let value = cx
        .query(&child, font_size)
        .resource_fallback(ACCENT_SIZE)
        .try_value()
        .unwrap();
    assert_eq!(value, 18.0);
}

#[test]
fn explain_reports_local_source_over_cascade() {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let theme = ThemeBuilder::new().build();
    let expressions = ExpressionLayer::new();
    let style = StyleBuilder::new().set(width, 50.0).build();
    let cascade = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Override, style)
        .build();

    let mut element = TestElement::new(1, None);
    element
        .store
        .set_local_with_source(width, 100.0, LocalValueSource::TemplateBinding);

    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    let resolved = cx.explain_value(&element, width, Some((&cascade, cascade.root_state())));

    assert_eq!(resolved.value, 100.0);
    assert_eq!(
        resolved.source,
        ResolvedSource::LocalOverride {
            source: LocalValueSource::TemplateBinding,
        }
    );
}

#[test]
fn explain_reports_winning_rule_specificity() {
    use crate::{ClassId, SelectorInputs, SelectorStep, TypeTag};

    const BUTTON: TypeTag = TypeTag(1);
    const PRIMARY: ClassId = ClassId(1);

    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let theme = ThemeBuilder::new().build();
    let expressions = ExpressionLayer::new();
    let broad_selector = SelectorStep::type_tag(BUTTON);
    let specific_selector = SelectorStep::type_tag(BUTTON).with_class(PRIMARY);
    let cascade = StyleCascadeBuilder::new()
        .push_rules(
            StyleOrigin::Sheet,
            [
                (
                    broad_selector.clone(),
                    StyleBuilder::new().set(width, 10.0).build(),
                ),
                (
                    specific_selector.clone(),
                    StyleBuilder::new().set(width, 20.0).build(),
                ),
            ],
        )
        .build();
    let classes = [PRIMARY];
    let state = cascade.enter_subject(
        cascade.root_state(),
        &SelectorInputs::new(Some(BUTTON), &classes, &[]),
    );
    let element = TestElement::new(1, None);

    let source = cascade.winning_source_for_id(state, width.id()).unwrap();
    assert_eq!(
        source.rule().unwrap().selector(),
        &specific_selector.clone().into()
    );

    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    let resolved = cx.explain_value(&element, width, Some((&cascade, state)));

    assert_eq!(resolved.value, 20.0);
    assert_eq!(
        resolved.source,
        ResolvedSource::CascadeRule {
            origin: StyleOrigin::Sheet,
            selector: specific_selector.clone().into(),
            specificity: specific_selector.specificity(),
            source_index: 0,
            order: 1,
            resource: None,
            expression: None,
        }
    );
}

#[test]
fn explain_reports_cascade_resource_indirection() {
    use crate::{ResourceKey, SelectorInputs, SelectorStep, TypeTag};

    const CARD: TypeTag = TypeTag(1);
    const CARD_BG: ResourceKey = ResourceKey::new(1);
    const FALLBACK_BG: ResourceKey = ResourceKey::new(2);

    let mut registry = PropertyRegistry::new();
    let background = registry.register("Background", PropertyMetadataBuilder::new(0_u32).build());

    let theme = ThemeBuilder::new()
        .set(CARD_BG, 0x00_11_22_u32)
        .set(FALLBACK_BG, 0xff_ee_dd_u32)
        .build();
    let expressions = ExpressionLayer::new();
    let selector = SelectorStep::type_tag(CARD);
    let style = StyleBuilder::new()
        .set_resource(background, CARD_BG)
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_rule(StyleOrigin::Sheet, selector.clone(), style)
        .build();
    let state = cascade.enter_subject(cascade.root_state(), &SelectorInputs::typed(CARD));
    let element = TestElement::new(1, None);

    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    let resolved = cx
        .query(&element, background)
        .style(&cascade, state)
        .resource_fallback(FALLBACK_BG)
        .try_explain()
        .unwrap();

    assert_eq!(resolved.value, 0x00_11_22);
    assert_eq!(
        resolved.source,
        ResolvedSource::CascadeRule {
            origin: StyleOrigin::Sheet,
            selector: selector.into(),
            specificity: Specificity(0, 0, 0, 1),
            source_index: 0,
            order: 0,
            resource: Some(CARD_BG),
            expression: None,
        }
    );
}

#[test]
fn explain_reports_property_level_theme_resource() {
    use crate::ResourceKey;

    const ACCENT_WIDTH: ResourceKey = ResourceKey::new(0);

    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let theme = ThemeBuilder::new().set(ACCENT_WIDTH, 75.0_f64).build();
    let expressions = ExpressionLayer::new();
    let element = TestElement::new(1, None);

    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    let resolved = cx
        .query(&element, width)
        .resource_fallback(ACCENT_WIDTH)
        .try_explain()
        .unwrap();

    assert_eq!(resolved.value, 75.0);
    assert_eq!(
        resolved.source,
        ResolvedSource::ThemeResource { key: ACCENT_WIDTH }
    );
}

#[test]
fn explain_reports_inherited_ancestor_depth_and_inner_source() {
    let mut registry = PropertyRegistry::new();
    let font_size = registry.register(
        "FontSize",
        PropertyMetadataBuilder::new(12.0_f64)
            .inherits(true)
            .build(),
    );

    let theme = ThemeBuilder::new().build();
    let expressions = ExpressionLayer::new();
    let mut grandparent = TestElement::new(1, None);
    grandparent
        .store
        .set_local_with_source(font_size, 18.0, LocalValueSource::TemplateDefault);
    let parent = TestElement::new(2, Some(1));
    let child = TestElement::new(3, Some(2));

    let elements: BTreeMap<u32, &TestElement> = [(1, &grandparent), (2, &parent), (3, &child)]
        .into_iter()
        .collect();

    let cx = ResolveCx::new(
        &registry,
        &theme,
        &expressions,
        PropertyParentLookup::new(|key| {
            elements
                .get(&key)
                .map(|e| (e.property_store(), e.parent_key()))
        }),
    );
    let resolved = cx.explain_value(&child, font_size, None);

    assert_eq!(resolved.value, 18.0);
    assert_eq!(
        resolved.source,
        ResolvedSource::Inherited {
            ancestor_depth: 2,
            inner: Box::new(ResolvedSource::LocalOverride {
                source: LocalValueSource::TemplateDefault,
            }),
        }
    );
}

#[test]
fn explain_reports_default_fallback() {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(42.0_f64).build());

    let theme = ThemeBuilder::new().build();
    let expressions = ExpressionLayer::new();
    let element = TestElement::new(1, None);

    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    let resolved = cx.explain_value(&element, width, None);

    assert_eq!(resolved.value, 42.0);
    assert_eq!(resolved.source, ResolvedSource::Default);
}

#[test]
fn cx_accessors() {
    let registry = PropertyRegistry::new();
    let theme = ThemeBuilder::new().build();
    let expressions = ExpressionLayer::new();

    let cx = ResolveCx::<u32, _>::new(&registry, &theme, &expressions, NoResolveParentLookup);

    // Can access registry and theme
    assert_eq!(cx.registry().len(), 0);
    assert!(cx.theme().is_empty());
}

/// Asserts `ResolveCx::get_value` matches `DependencyObjectExt::get_inherited`
/// when style=None and theme is empty for an inheriting property.
/// This prevents precedence drift between the two APIs.
#[test]
fn resolve_matches_get_inherited() {
    use understory_property::DependencyObjectExt;

    let mut registry = PropertyRegistry::new();
    let font_size = registry.register(
        "FontSize",
        PropertyMetadataBuilder::new(12.0_f64)
            .inherits(true)
            .build(),
    );

    let theme = ThemeBuilder::new().build();
    let expressions = ExpressionLayer::new();

    // Build a 3-level hierarchy: grandparent -> parent -> child
    let mut grandparent = TestElement::new(1, None);
    grandparent.store.set_local(font_size, 24.0);

    let mut parent = TestElement::new(2, Some(1));
    parent.store.set_animation(font_size, 18.0); // Animation at parent level

    let child = TestElement::new(3, Some(2));

    let elements: BTreeMap<u32, &TestElement> = [(1, &grandparent), (2, &parent), (3, &child)]
        .into_iter()
        .collect();

    let store_lookup = |key| {
        elements
            .get(&key)
            .map(|e| (e.property_store(), e.parent_key()))
    };

    // ResolveCx::get_value with no style
    let cx = ResolveCx::new(
        &registry,
        &theme,
        &expressions,
        PropertyParentLookup::new(store_lookup),
    );
    let cx_value = cx.get_value(&child, font_size, None);

    // DependencyObjectExt::get_inherited
    let ext_value = child.get_inherited(font_size, &registry, &|key| {
        elements
            .get(&key)
            .map(|e| (e.property_store(), e.parent_key()))
    });

    // Both should return the same value (parent's animation: 18.0)
    assert_eq!(cx_value, ext_value);
    assert_eq!(cx_value, 18.0);
}

const ACCENT: ResourceKey = ResourceKey::new(40);
const GAP: ResourceKey = ResourceKey::new(41);

/// App theme: `ACCENT` and `GAP` in the base table, `ACCENT` overridden by
/// the dark variant.
fn light_dark_variant_theme() -> VariantTheme {
    VariantThemeBuilder::new()
        .base(
            ThemeBuilder::new()
                .set(ACCENT, 0x0078D4_u32)
                .set(GAP, 4.0_f64)
                .build(),
        )
        .variant(
            ThemeVariant::DARK,
            ThemeBuilder::new().set(ACCENT, 0x4CC2FF_u32).build(),
        )
        .build()
}

#[test]
fn resolve_style_resource_through_variant_theme() {
    let mut registry = PropertyRegistry::new();
    let background = registry.register("Background", PropertyMetadataBuilder::new(0_u32).build());

    let style = StyleBuilder::new().set_resource(background, ACCENT).build();
    let cascade = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Base, style)
        .build();

    let theme = light_dark_variant_theme();
    let expressions = ExpressionLayer::new();
    let element = TestElement::new(1, None);
    let style_state = Some((&cascade, cascade.root_state()));

    // No active variant resolves from the base table.
    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    assert_eq!(cx.get_value(&element, background, style_state), 0x0078D4);

    // The dark variant overrides the base entry.
    let dark = ThemeResolveOptions::from(&theme).with_variant(ThemeVariant::DARK);
    let cx = ResolveCx::new(&registry, dark, &expressions, NoResolveParentLookup);
    assert_eq!(cx.get_value(&element, background, style_state), 0x4CC2FF);

    // An unknown variant falls back cleanly to the base table.
    let sepia = ThemeResolveOptions::from(&theme).with_variant(ThemeVariant::new(16));
    let cx = ResolveCx::new(&registry, sepia, &expressions, NoResolveParentLookup);
    assert_eq!(cx.get_value(&element, background, style_state), 0x0078D4);
}

#[test]
fn resolve_style_resource_through_scope_shadows_outer_scope() {
    let mut registry = PropertyRegistry::new();
    let background = registry.register("Background", PropertyMetadataBuilder::new(0_u32).build());

    let style = StyleBuilder::new().set_resource(background, ACCENT).build();
    let cascade = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Base, style)
        .build();

    let app = ThemeScope::new(ThemeBuilder::new().set(ACCENT, 0x0078D4_u32).build());
    let popup = app.child(ThemeBuilder::new().set(ACCENT, 0xE81123_u32).build());
    let expressions = ExpressionLayer::new();
    let element = TestElement::new(1, None);
    let style_state = Some((&cascade, cascade.root_state()));

    let cx = ResolveCx::new(&registry, &popup, &expressions, NoResolveParentLookup);
    let resolved = cx.explain_value(&element, background, style_state);
    assert_eq!(resolved.value, 0xE81123);
    assert_eq!(
        resolved.source,
        ResolvedSource::CascadeDirect {
            origin: StyleOrigin::Base,
            source_index: 0,
            resource: Some(ACCENT),
            expression: None,
        }
    );

    // The outer scope is unaffected by the popup's overrides.
    let cx = ResolveCx::new(&registry, &app, &expressions, NoResolveParentLookup);
    assert_eq!(cx.get_value(&element, background, style_state), 0x0078D4);
}

#[test]
fn resolve_scope_chain_falls_back_to_root() {
    let mut registry = PropertyRegistry::new();
    let gap = registry.register("Gap", PropertyMetadataBuilder::new(0.0_f64).build());

    let style = StyleBuilder::new().set_resource(gap, GAP).build();
    let cascade = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Base, style)
        .build();

    // Neither inner scope overrides GAP, so the root app theme supplies it.
    let app = ThemeScope::new(ThemeBuilder::new().set(GAP, 4.0_f64).build());
    let inner = app
        .child(ThemeBuilder::new().set(ACCENT, 1_u32).build())
        .child(Theme::default());
    let expressions = ExpressionLayer::new();
    let element = TestElement::new(1, None);

    let cx = ResolveCx::new(&registry, &inner, &expressions, NoResolveParentLookup);
    let value = cx.get_value(&element, gap, Some((&cascade, cascade.root_state())));
    assert_eq!(value, 4.0);
}

#[test]
fn resolve_variant_and_scope_composed() {
    let mut registry = PropertyRegistry::new();
    let background = registry.register("Background", PropertyMetadataBuilder::new(0_u32).build());
    let gap = registry.register("Gap", PropertyMetadataBuilder::new(0.0_f64).build());

    let style = StyleBuilder::new()
        .set_resource(background, ACCENT)
        .set_resource(gap, GAP)
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Base, style)
        .build();

    // The island shadows ACCENT with a base-only entry; GAP falls back
    // outward to the app theme under the same active variant.
    let app = ThemeScope::new(light_dark_variant_theme());
    let island = app.child(ThemeBuilder::new().set(ACCENT, 0xE81123_u32).build());
    let expressions = ExpressionLayer::new();
    let element = TestElement::new(1, None);
    let style_state = Some((&cascade, cascade.root_state()));

    let dark_island = ThemeResolveOptions::from(&island).with_variant(ThemeVariant::DARK);
    let cx = ResolveCx::new(&registry, dark_island, &expressions, NoResolveParentLookup);
    // Inner scope wins for keys it contains, even over the outer dark table.
    assert_eq!(cx.get_value(&element, background, style_state), 0xE81123);
    // Keys the inner scope lacks fall back outward with variant fallback.
    assert_eq!(cx.get_value(&element, gap, style_state), 4.0);

    let dark_app = ThemeResolveOptions::from(&app).with_variant(ThemeVariant::DARK);
    let cx = ResolveCx::new(&registry, dark_app, &expressions, NoResolveParentLookup);
    assert_eq!(cx.get_value(&element, background, style_state), 0x4CC2FF);
}

#[test]
fn style_expression_token_observes_scoped_variant_values() {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let style = StyleBuilder::new()
        .set_expr(width, expr::token(GAP) * 2.0)
        .build();
    let cascade = StyleCascadeBuilder::new()
        .push_style(StyleOrigin::Override, style)
        .build();

    let app = ThemeScope::new(light_dark_variant_theme());
    let popup = app.child(
        VariantThemeBuilder::new()
            .variant(
                ThemeVariant::DARK,
                ThemeBuilder::new().set(GAP, 8.0_f64).build(),
            )
            .build(),
    );
    let expressions = ExpressionLayer::new();
    let element = TestElement::new(1, None);
    let style_state = Some((&cascade, cascade.root_state()));

    // The popup's dark table supplies GAP under the dark variant.
    let dark_popup = ThemeResolveOptions::from(&popup).with_variant(ThemeVariant::DARK);
    let cx = ResolveCx::new(&registry, dark_popup, &expressions, NoResolveParentLookup);
    assert_eq!(cx.get_value(&element, width, style_state), 16.0);

    // Without the dark variant, the popup theme does not contain GAP, so the
    // token falls back outward to the app theme's base table.
    let cx = ResolveCx::new(&registry, &popup, &expressions, NoResolveParentLookup);
    assert_eq!(cx.get_value(&element, width, style_state), 8.0);
}

#[test]
fn default_expression_token_observes_variant_values() {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let mut expressions = ExpressionLayer::new();
    expressions.set_default(width, expr::token(ACCENT));

    let theme = VariantThemeBuilder::new()
        .base(ThemeBuilder::new().set(ACCENT, 10.0_f64).build())
        .variant(
            ThemeVariant::HIGH_CONTRAST,
            ThemeBuilder::new().set(ACCENT, 20.0_f64).build(),
        )
        .build();
    let element = TestElement::new(1, None);

    let high_contrast = ThemeResolveOptions::from(&theme).with_variant(ThemeVariant::HIGH_CONTRAST);
    let cx = ResolveCx::new(
        &registry,
        high_contrast,
        &expressions,
        NoResolveParentLookup,
    );
    assert_eq!(cx.get_value(&element, width, None), 20.0);

    let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
    assert_eq!(cx.get_value(&element, width, None), 10.0);
}

#[test]
fn query_resource_fallback_observes_scoped_variant_values() {
    let mut registry = PropertyRegistry::new();
    let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

    let app = ThemeScope::new(
        VariantThemeBuilder::new()
            .base(ThemeBuilder::new().set(GAP, 4.0_f64).build())
            .variant(
                ThemeVariant::DARK,
                ThemeBuilder::new().set(GAP, 6.0_f64).build(),
            )
            .build(),
    );
    let popup = app.child(ThemeBuilder::new().set(GAP, 12.0_f64).build());
    let expressions = ExpressionLayer::new();
    let element = TestElement::new(1, None);

    let dark_app = ThemeResolveOptions::from(&app).with_variant(ThemeVariant::DARK);
    let cx = ResolveCx::new(&registry, dark_app, &expressions, NoResolveParentLookup);
    let resolved = cx
        .query(&element, width)
        .resource_fallback(GAP)
        .try_explain()
        .unwrap();
    assert_eq!(resolved.value, 6.0);
    assert_eq!(resolved.source, ResolvedSource::ThemeResource { key: GAP });

    let cx = ResolveCx::new(&registry, &popup, &expressions, NoResolveParentLookup);
    let value = cx
        .query(&element, width)
        .resource_fallback(GAP)
        .try_value()
        .unwrap();
    assert_eq!(value, 12.0);
}

#[test]
fn theme_accessors_expose_base_theme_and_active_variant() {
    let registry = PropertyRegistry::new();
    let expressions = ExpressionLayer::new();

    let app = ThemeScope::new(light_dark_variant_theme());
    let popup = app.child(ThemeBuilder::new().set(ACCENT, 0xE81123_u32).build());

    let dark_popup = ThemeResolveOptions::from(&popup).with_variant(ThemeVariant::DARK);
    let cx: ResolveCx<'_, u32, _> =
        ResolveCx::new(&registry, dark_popup, &expressions, NoResolveParentLookup);

    assert_eq!(
        cx.theme_options().active_variant(),
        Some(ThemeVariant::DARK)
    );
    // `theme()` exposes the root scope's variant-blind base table.
    assert_eq!(cx.theme().get::<u32>(ACCENT), Some(&0x0078D4));
    // `theme_options()` sees the popup's shadowing entry.
    assert_eq!(cx.theme_options().get::<u32>(ACCENT), Some(&0xE81123));
    assert!(cx.theme_options().contains(GAP));
}
