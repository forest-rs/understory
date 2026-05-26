// Copyright 2025 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Resolution context for full precedence chain.
//!
//! This module provides [`ResolveCx`], which bundles everything needed to
//! resolve property values through the full precedence chain.

use understory_property::{
    DependencyObject, ParentLookup, Property, PropertyRegistry, PropertyStore,
};

use crate::matcher::{MatchState, StyleCascade};
use crate::style::StyleValueRef;
use crate::theme::Theme;

/// Parent data used by [`ResolveCx`] when walking inherited values.
///
/// When `match_state` is present, inherited properties include that
/// ancestor's style-layer value in the same precedence position as the starting
/// object. Use [`PropertyParentLookup`] when adapting a property-only parent
/// lookup that should preserve Animation → Local inheritance semantics.
#[derive(Copy, Clone, Debug)]
pub struct ResolveParent<'a, K: Copy + Eq + 'a> {
    store: &'a PropertyStore<K>,
    parent_key: Option<K>,
    match_state: Option<MatchState>,
}

impl<'a, K: Copy + Eq + 'a> ResolveParent<'a, K> {
    /// Creates a style-blind parent entry.
    #[must_use]
    #[inline]
    pub const fn new(store: &'a PropertyStore<K>, parent_key: Option<K>) -> Self {
        Self {
            store,
            parent_key,
            match_state: None,
        }
    }

    /// Creates a style-aware parent entry.
    #[must_use]
    #[inline]
    pub const fn with_match_state(
        store: &'a PropertyStore<K>,
        parent_key: Option<K>,
        match_state: MatchState,
    ) -> Self {
        Self {
            store,
            parent_key,
            match_state: Some(match_state),
        }
    }

    /// Returns the parent's property store.
    #[must_use]
    #[inline]
    pub const fn store(self) -> &'a PropertyStore<K> {
        self.store
    }

    /// Returns the next parent key.
    #[must_use]
    #[inline]
    pub const fn parent_key(self) -> Option<K> {
        self.parent_key
    }

    /// Returns the parent's style match state, if supplied.
    #[must_use]
    #[inline]
    pub const fn match_state(self) -> Option<MatchState> {
        self.match_state
    }
}

/// Lookup used by [`ResolveCx`] to walk inherited values.
///
/// This is the canonical resolver lookup contract for `understory_style`.
/// Hosts that cache style match state should return
/// [`ResolveParent::with_match_state`] for each ancestor so inherited values can
/// see ancestor style-layer values.
pub trait ResolveParentLookup<'a, K: Copy + Eq + 'a> {
    /// Returns the parent entry for `key`.
    fn lookup_resolve_parent(&self, key: K) -> Option<ResolveParent<'a, K>>;
}

impl<'a, K, F> ResolveParentLookup<'a, K> for F
where
    K: Copy + Eq + 'a,
    F: Fn(K) -> Option<ResolveParent<'a, K>>,
{
    fn lookup_resolve_parent(&self, key: K) -> Option<ResolveParent<'a, K>> {
        self(key)
    }
}

/// A resolver lookup with no parents.
#[derive(Copy, Clone, Debug, Default)]
pub struct NoResolveParentLookup;

impl<'a, K> ResolveParentLookup<'a, K> for NoResolveParentLookup
where
    K: Copy + Eq + 'a,
{
    #[inline]
    fn lookup_resolve_parent(&self, _key: K) -> Option<ResolveParent<'a, K>> {
        None
    }
}

/// Adapts a property-only [`ParentLookup`] for [`ResolveCx`].
///
/// This intentionally does not provide ancestor style match state, so inherited
/// resolution through this adapter sees only Animation → Local values on each
/// ancestor. Use a direct [`ResolveParentLookup`] implementation when the host
/// can provide match state.
#[derive(Copy, Clone, Debug)]
pub struct PropertyParentLookup<F> {
    lookup: F,
}

impl<F> PropertyParentLookup<F> {
    /// Wraps a property-only parent lookup.
    #[must_use]
    #[inline]
    pub const fn new(lookup: F) -> Self {
        Self { lookup }
    }

    /// Returns the wrapped lookup.
    #[must_use]
    #[inline]
    pub const fn inner(&self) -> &F {
        &self.lookup
    }

    /// Consumes the adapter and returns the wrapped lookup.
    #[must_use]
    #[inline]
    pub fn into_inner(self) -> F {
        self.lookup
    }
}

impl<'a, K, F> ResolveParentLookup<'a, K> for PropertyParentLookup<F>
where
    K: Copy + Eq + 'a,
    F: ParentLookup<'a, K>,
{
    #[inline]
    fn lookup_resolve_parent(&self, key: K) -> Option<ResolveParent<'a, K>> {
        self.lookup
            .lookup(key)
            .map(|(store, parent_key)| ResolveParent::new(store, parent_key))
    }
}

/// Resolution context bundling registry, theme, and parent lookup.
///
/// This avoids passing many parameters to resolution functions and provides
/// the full WinUI-style precedence chain:
///
/// **Animation → Local → Style → Theme → Inherited → Default**
///
/// # Type Parameters
///
/// * `K` - The key type for objects (e.g., `u32`, `NodeId`)
/// * `F` - The resolver parent lookup type
///
/// # Example
///
/// ```rust
/// use understory_style::{
///     NoResolveParentLookup, ResolveCx, StyleCascadeBuilder, StyleBuilder, StyleOrigin, ThemeBuilder,
/// };
/// use understory_property::{
///     DependencyObject, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
/// };
///
/// let mut registry = PropertyRegistry::new();
/// let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
///
/// let theme = ThemeBuilder::new().build();
/// let style = StyleBuilder::new().set(width, 80.0).build();
/// let cascade = StyleCascadeBuilder::new()
///     .push_style(StyleOrigin::Override, style)
///     .build();
///
/// // Create context with no parent lookup (flat tree)
/// let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
///
/// struct Element {
///     key: u32,
///     parent: Option<u32>,
///     store: PropertyStore<u32>,
/// }
///
/// impl DependencyObject<u32> for Element {
///     fn property_store(&self) -> &PropertyStore<u32> { &self.store }
///     fn property_store_mut(&mut self) -> &mut PropertyStore<u32> { &mut self.store }
///     fn key(&self) -> u32 { self.key }
///     fn parent_key(&self) -> Option<u32> { self.parent }
/// }
///
/// let mut element = Element {
///     key: 1,
///     parent: None,
///     store: PropertyStore::new(1),
/// };
///
/// element.store.set_local(width, 100.0);
///
/// // Local still wins over style
/// let value = cx.get_value(&element, width, Some((&cascade, cascade.root_state())));
/// assert_eq!(value, 100.0);
/// ```
pub struct ResolveCx<'a, K, F>
where
    K: Copy + Eq + 'a,
    F: ResolveParentLookup<'a, K>,
{
    /// The property registry containing metadata and defaults.
    registry: &'a PropertyRegistry,
    /// The current theme for resource lookups.
    theme: &'a Theme,
    /// Lookup used to walk inherited values.
    store_lookup: F,
    /// Phantom to hold K in the type.
    _marker: core::marker::PhantomData<K>,
}

impl<'a, K, F> core::fmt::Debug for ResolveCx<'a, K, F>
where
    K: Copy + Eq + 'a,
    F: ResolveParentLookup<'a, K>,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ResolveCx")
            .field("registry", &self.registry)
            .field("theme", &self.theme)
            .field("store_lookup", &core::any::type_name::<F>())
            .finish()
    }
}

impl<'a, K, F> ResolveCx<'a, K, F>
where
    K: Copy + Eq + 'a,
    F: ResolveParentLookup<'a, K>,
{
    /// Creates a new resolution context.
    ///
    /// # Arguments
    ///
    /// * `registry` - The property registry
    /// * `theme` - The current theme
    /// * `store_lookup` - Lookup returning resolver parent data for a given key
    pub fn new(registry: &'a PropertyRegistry, theme: &'a Theme, store_lookup: F) -> Self {
        Self {
            registry,
            theme,
            store_lookup,
            _marker: core::marker::PhantomData,
        }
    }

    /// Returns a reference to the property registry.
    #[must_use]
    #[inline]
    pub fn registry(&self) -> &PropertyRegistry {
        self.registry
    }

    /// Returns a reference to the current theme.
    #[must_use]
    #[inline]
    pub fn theme(&self) -> &Theme {
        self.theme
    }
}

impl<'a, K, F> ResolveCx<'a, K, F>
where
    K: Copy + Eq + 'a,
    F: ResolveParentLookup<'a, K>,
{
    fn style_value_ref<'cx, T>(
        &'cx self,
        cascade: &'cx StyleCascade,
        state: MatchState,
        property: Property<T>,
    ) -> Option<&'cx T>
    where
        T: Clone + 'static,
    {
        cascade
            .get_entry_ref(state, property)
            .and_then(|entry| match entry {
                StyleValueRef::Value(value) => Some(value),
                StyleValueRef::Resource(key) => self.theme.get::<T>(key),
            })
    }

    fn inherited_value_ref<'cx, T>(
        &'cx self,
        mut current_key: Option<K>,
        property: Property<T>,
        cascade: Option<&'cx StyleCascade>,
    ) -> Option<&'cx T>
    where
        T: Clone + 'static,
    {
        while let Some(key) = current_key {
            let Some(parent) = self.store_lookup.lookup_resolve_parent(key) else {
                break;
            };

            if let Some(value) = parent.store().get_animation(property) {
                return Some(value);
            }
            if let Some(value) = parent.store().get_local(property) {
                return Some(value);
            }
            if let (Some(cascade), Some(match_state)) = (cascade, parent.match_state())
                && let Some(value) = self.style_value_ref(cascade, match_state, property)
            {
                return Some(value);
            }

            current_key = parent.parent_key();
        }

        None
    }

    /// Resolves a property value through the full precedence chain, borrowed.
    ///
    /// This is the borrowed variant of [`ResolveCx::get_value`]. It returns a reference to the
    /// effective value, avoiding cloning for large types.
    ///
    /// # Panics
    ///
    /// Panics if the property is not registered in the registry.
    pub fn get_value_ref<'cx, T, O>(
        &'cx self,
        object: &'cx O,
        property: Property<T>,
        style: Option<(&'cx StyleCascade, MatchState)>,
    ) -> &'cx T
    where
        T: Clone + 'static,
        O: DependencyObject<K>,
    {
        // 1. Animation value
        if let Some(value) = object.property_store().get_animation(property) {
            return value;
        }

        // 2. Local value
        if let Some(value) = object.property_store().get_local(property) {
            return value;
        }

        // 3. Style-layer value
        if let Some((style, state)) = style
            && let Some(value) = self.style_value_ref(style, state, property)
        {
            return value;
        }

        // 4. Inherited value (if property inherits). Style-aware lookups
        // include Animation -> Local -> Style for each ancestor; plain
        // property lookups include Animation -> Local only.
        if let Some(metadata) = self.registry.get_metadata::<T>(property) {
            if metadata.inherits()
                && let Some(value) =
                    self.inherited_value_ref(object.parent_key(), property, style.map(|s| s.0))
            {
                return value;
            }
            // 5. Default value
            return metadata.default_value();
        }

        panic!("Property {:?} not found in registry", property.id());
    }

    /// Resolves a property value through the full precedence chain.
    ///
    /// Precedence (highest to lowest):
    /// 1. Animation value on the object
    /// 2. Local value on the object
    /// 3. Style value (if style provided)
    /// 4. Inherited value (if property inherits, walks parent chain)
    /// 5. Default value from registry
    ///
    /// # Arguments
    ///
    /// * `object` - The object to get the value for
    /// * `property` - The property to resolve
    /// * `style` - Optional matched style cascade and state to check for property values
    ///
    /// # Panics
    ///
    /// Panics if the property is not registered in the registry.
    pub fn get_value<T, O>(
        &self,
        object: &O,
        property: Property<T>,
        style: Option<(&StyleCascade, MatchState)>,
    ) -> T
    where
        T: Clone + 'static,
        O: DependencyObject<K>,
    {
        self.get_value_ref(object, property, style).clone()
    }

    /// Resolves a property value with a resource key fallback, borrowed.
    ///
    /// This is the borrowed variant of [`ResolveCx::get_value_with_theme`].
    ///
    /// # Panics
    ///
    /// Panics if the property is not registered in the registry.
    pub fn get_value_with_theme_ref<'cx, T, O>(
        &'cx self,
        object: &'cx O,
        property: Property<T>,
        style: Option<(&'cx StyleCascade, MatchState)>,
        resource_key: Option<crate::theme::ResourceKey>,
    ) -> &'cx T
    where
        T: Clone + 'static,
        O: DependencyObject<K>,
    {
        // 1. Animation value
        if let Some(value) = object.property_store().get_animation(property) {
            return value;
        }

        // 2. Local value
        if let Some(value) = object.property_store().get_local(property) {
            return value;
        }

        // 3. Style-layer value
        if let Some((style, state)) = style
            && let Some(value) = self.style_value_ref(style, state, property)
        {
            return value;
        }

        // 4. Theme resource
        if let Some(key) = resource_key
            && let Some(value) = self.theme.get::<T>(key)
        {
            return value;
        }

        // 5. Inherited value (if property inherits). Style-aware lookups
        // include Animation -> Local -> Style for each ancestor; plain
        // property lookups include Animation -> Local only.
        if let Some(metadata) = self.registry.get_metadata::<T>(property) {
            if metadata.inherits()
                && let Some(value) =
                    self.inherited_value_ref(object.parent_key(), property, style.map(|s| s.0))
            {
                return value;
            }
            // 6. Default value
            return metadata.default_value();
        }

        panic!("Property {:?} not found in registry", property.id());
    }

    /// Resolves a property value with a resource key fallback.
    ///
    /// This is useful when a property can be set directly or can reference
    /// a theme resource. The precedence is:
    ///
    /// 1. Animation value on the object
    /// 2. Local value on the object
    /// 3. Style value (if style provided)
    /// 4. Theme resource (if `resource_key` provided and present in theme)
    /// 5. Inherited value (if property inherits)
    /// 6. Default value from registry
    ///
    /// # Arguments
    ///
    /// * `object` - The object to get the value for
    /// * `property` - The property to resolve
    /// * `style` - Optional matched style cascade and state to check
    /// * `resource_key` - Optional theme resource key to check
    ///
    /// # Panics
    ///
    /// Panics if the property is not registered in the registry.
    pub fn get_value_with_theme<T, O>(
        &self,
        object: &O,
        property: Property<T>,
        style: Option<(&StyleCascade, MatchState)>,
        resource_key: Option<crate::theme::ResourceKey>,
    ) -> T
    where
        T: Clone + 'static,
        O: DependencyObject<K>,
    {
        self.get_value_with_theme_ref(object, property, style, resource_key)
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StyleBuilder, ThemeBuilder};
    use crate::{StyleCascadeBuilder, StyleOrigin};
    use alloc::collections::BTreeMap;
    use alloc::string::String;
    use understory_property::PropertyMetadataBuilder;
    use understory_property::PropertyStore;

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

        let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
        let value = cx.get_value(&element, width, None);
        assert_eq!(value, 100.0);
    }

    #[test]
    fn resolve_local_value_ref_borrows_from_store() {
        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

        let theme = ThemeBuilder::new().build();

        let mut element = TestElement::new(1, None);
        element.store.set_local(width, 100.0);

        let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
        let value_ref = cx.get_value_ref(&element, width, None);
        assert!(core::ptr::eq(
            value_ref,
            element.property_store().get_local(width).unwrap()
        ));
        assert_eq!(*value_ref, 100.0);
    }

    #[test]
    fn resolve_animation_over_local() {
        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

        let theme = ThemeBuilder::new().build();

        let mut element = TestElement::new(1, None);
        element.store.set_local(width, 100.0);
        element.store.set_animation(width, 200.0);

        let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
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

        let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
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

        let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
        let value = cx.get_value(&element, width, Some((&style, style.root_state())));
        assert_eq!(value, 50.0);
    }

    #[test]
    fn resolve_default_value() {
        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(42.0_f64).build());

        let theme = ThemeBuilder::new().build();
        let element = TestElement::new(1, None);

        let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
        let value = cx.get_value(&element, width, None);
        assert_eq!(value, 42.0);
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

        let mut parent = TestElement::new(1, None);
        parent.store.set_local(font_size, 16.0);

        let child = TestElement::new(2, Some(1));

        let elements: BTreeMap<u32, &TestElement> =
            [(1, &parent), (2, &child)].into_iter().collect();

        let cx = ResolveCx::new(
            &registry,
            &theme,
            PropertyParentLookup::new(|key| {
                elements
                    .get(&key)
                    .map(|e| (e.property_store(), e.parent_key()))
            }),
        );

        let value = cx.get_value(&child, font_size, None);
        assert_eq!(value, 16.0);
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

        let mut parent = TestElement::new(1, None);
        parent.store.set_local(font_size, 16.0);

        let mut child = TestElement::new(2, Some(1));
        child.store.set_local(font_size, 20.0);

        let elements: BTreeMap<u32, &TestElement> =
            [(1, &parent), (2, &child)].into_iter().collect();

        let cx = ResolveCx::new(
            &registry,
            &theme,
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
        let style = StyleBuilder::new().set(font_size, 18.0).build();
        let style = StyleCascadeBuilder::new()
            .push_style(StyleOrigin::Override, style)
            .build();

        let mut parent = TestElement::new(1, None);
        parent.store.set_local(font_size, 16.0);

        let child = TestElement::new(2, Some(1));

        let elements: BTreeMap<u32, &TestElement> =
            [(1, &parent), (2, &child)].into_iter().collect();

        let cx = ResolveCx::new(
            &registry,
            &theme,
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
        let button_state =
            cascade.enter_subject(cascade.root_state(), &SelectorInputs::typed(BUTTON));
        let text_state = cascade.enter_subject(button_state, &SelectorInputs::typed(TEXT));

        let plain_elements: BTreeMap<u32, &TestElement> =
            [(1, &button), (2, &text)].into_iter().collect();
        let plain_cx = ResolveCx::new(
            &registry,
            &theme,
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
        let styled_cx = ResolveCx::new(&registry, &theme, StyledLookup { entries: &entries });
        assert_eq!(
            styled_cx.get_value(&text, foreground, Some((&cascade, text_state))),
            0xff_ff_ff,
            "style-aware parent lookup should inherit the styled button foreground"
        );
    }

    #[test]
    fn resolve_with_theme_resource() {
        use crate::ResourceKey;

        const ACCENT_WIDTH: ResourceKey = ResourceKey::new(0);

        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

        let theme = ThemeBuilder::new().set(ACCENT_WIDTH, 75.0_f64).build();
        let element = TestElement::new(1, None);

        let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
        let value = cx.get_value_with_theme(&element, width, None, Some(ACCENT_WIDTH));
        assert_eq!(value, 75.0);
    }

    #[test]
    fn resolve_with_theme_resource_ref_borrows_from_theme() {
        use crate::ResourceKey;

        const ACCENT_WIDTH: ResourceKey = ResourceKey::new(0);

        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

        let theme = ThemeBuilder::new().set(ACCENT_WIDTH, 75.0_f64).build();
        let element = TestElement::new(1, None);

        let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
        let value_ref = cx.get_value_with_theme_ref(&element, width, None, Some(ACCENT_WIDTH));
        assert!(core::ptr::eq(
            value_ref,
            theme.get::<f64>(ACCENT_WIDTH).unwrap()
        ));
        assert_eq!(*value_ref, 75.0);
    }

    #[test]
    fn resolve_local_over_theme() {
        use crate::ResourceKey;

        const ACCENT_WIDTH: ResourceKey = ResourceKey::new(0);

        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

        let theme = ThemeBuilder::new().set(ACCENT_WIDTH, 75.0_f64).build();
        let mut element = TestElement::new(1, None);
        element.store.set_local(width, 100.0);

        let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
        let value = cx.get_value_with_theme(&element, width, None, Some(ACCENT_WIDTH));
        assert_eq!(value, 100.0);
    }

    #[test]
    fn resolve_style_over_theme() {
        use crate::ResourceKey;

        const ACCENT_WIDTH: ResourceKey = ResourceKey::new(0);

        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

        let theme = ThemeBuilder::new().set(ACCENT_WIDTH, 75.0_f64).build();
        let style = StyleBuilder::new().set(width, 50.0).build();
        let style = StyleCascadeBuilder::new()
            .push_style(StyleOrigin::Override, style)
            .build();
        let element = TestElement::new(1, None);

        let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
        let value = cx.get_value_with_theme(
            &element,
            width,
            Some((&style, style.root_state())),
            Some(ACCENT_WIDTH),
        );
        assert_eq!(value, 50.0);
    }

    #[test]
    fn resolve_style_resource_over_theme_fallback() {
        use crate::ResourceKey;

        const THEME_FALLBACK: ResourceKey = ResourceKey::new(0);
        const STYLE_TOKEN: ResourceKey = ResourceKey::new(1);

        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());

        let theme = ThemeBuilder::new()
            .set(THEME_FALLBACK, 75.0_f64)
            .set(STYLE_TOKEN, 50.0_f64)
            .build();
        let style = StyleBuilder::new().set_resource(width, STYLE_TOKEN).build();
        let style = StyleCascadeBuilder::new()
            .push_style(StyleOrigin::Override, style)
            .build();
        let element = TestElement::new(1, None);

        let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
        let value = cx.get_value_with_theme(
            &element,
            width,
            Some((&style, style.root_state())),
            Some(THEME_FALLBACK),
        );
        assert_eq!(value, 50.0);
    }

    #[test]
    fn resolve_theme_over_inherited() {
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

        let mut parent = TestElement::new(1, None);
        parent.store.set_local(font_size, 16.0);

        let child = TestElement::new(2, Some(1));

        let elements: BTreeMap<u32, &TestElement> =
            [(1, &parent), (2, &child)].into_iter().collect();

        let cx = ResolveCx::new(
            &registry,
            &theme,
            PropertyParentLookup::new(|key| {
                elements
                    .get(&key)
                    .map(|e| (e.property_store(), e.parent_key()))
            }),
        );

        let value = cx.get_value_with_theme(&child, font_size, None, Some(ACCENT_SIZE));
        assert_eq!(value, 18.0);
    }

    #[test]
    fn cx_accessors() {
        let registry = PropertyRegistry::new();
        let theme = ThemeBuilder::new().build();

        let cx = ResolveCx::<u32, _>::new(&registry, &theme, NoResolveParentLookup);

        // Can access registry and theme
        assert_eq!(cx.registry().len(), 0);
        assert!(cx.theme().is_empty());
    }

    #[test]
    fn resolve_string_local_ref() {
        let mut registry = PropertyRegistry::new();
        let text = registry.register("Text", PropertyMetadataBuilder::new(String::new()).build());

        let theme = ThemeBuilder::new().build();

        let mut element = TestElement::new(1, None);
        element.store.set_local(text, String::from("hello world"));

        let cx = ResolveCx::new(&registry, &theme, NoResolveParentLookup);
        let value_ref = cx.get_value_ref(&element, text, None);
        assert!(core::ptr::eq(
            value_ref,
            element.property_store().get_local(text).unwrap()
        ));
        assert_eq!(value_ref.as_str(), "hello world");
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
        let cx = ResolveCx::new(&registry, &theme, PropertyParentLookup::new(store_lookup));
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
}
