// Copyright 2025 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Resolution context for property precedence.
//!
//! This module provides [`ResolveCx`], which bundles everything needed to
//! resolve property values through animation, local, style, theme, expression,
//! inheritance, and default stages. [`ResolveQuery`] adds explicit options for
//! configured lookups.

use alloc::boxed::Box;
use core::any::TypeId;

use understory_property::{
    DependencyObject, ErasedValue, LocalValueSource, ParentLookup, Property, PropertyId,
    PropertyRegistry, PropertyStore,
};
use understory_property_expression::{
    ExprError, ExpressionDefaults, ExpressionLayer, FunctionRegistry, InFlightProperties,
};

use crate::matcher::{MatchState, StyleCascade};
use crate::selector::{Selector, Specificity};
use crate::style::{ErasedStyleValueKind, StyleExpressionId, StyleValueKind};
use crate::stylesheet::StyleOrigin;
use crate::theme::{ResourceKey, Theme};

#[path = "resolve_expression.rs"]
mod expression;
#[path = "resolve_provenance.rs"]
mod provenance;

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

/// A resolved value paired with the source that supplied it.
///
/// This is an owned, cold-path inspection result. Normal rendering and update
/// loops should keep using [`ResolveCx::get_value`].
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved<T> {
    /// The resolved value.
    pub value: T,
    /// The resolution stage and source metadata that supplied `value`.
    pub source: ResolvedSource,
}

/// A value resolved by the resolver.
#[derive(Clone, Debug, PartialEq)]
enum ResolvedValue<'a, T> {
    /// A value borrowed from an existing property store, style, theme, or
    /// metadata default.
    Borrowed(&'a T),
    /// A value computed by evaluating an expression.
    Owned(T),
}

impl<T: Clone> ResolvedValue<'_, T> {
    /// Returns the resolved value as an owned value.
    #[must_use]
    fn into_owned(self) -> T {
        match self {
            Self::Borrowed(value) => value.clone(),
            Self::Owned(value) => value,
        }
    }
}

/// Expression state carried by [`ResolveCx`].
///
/// Most callers pass an [`ExpressionLayer`] to [`ResolveCx::new`]. Low-level
/// callers can use this type when defaults and function registrations are stored
/// separately.
#[derive(Copy, Clone, Debug)]
pub struct ExprResolveOptions<'a> {
    /// Property default expressions keyed by dependency property id.
    pub defaults: &'a ExpressionDefaults,
    /// Function registry used by expression call nodes.
    pub functions: &'a FunctionRegistry,
}

impl<'a> ExprResolveOptions<'a> {
    /// Creates expression resolution options from default expressions and
    /// function registry references.
    #[must_use]
    pub const fn new(defaults: &'a ExpressionDefaults, functions: &'a FunctionRegistry) -> Self {
        Self {
            defaults,
            functions,
        }
    }
}

impl<'a> From<&'a ExpressionLayer> for ExprResolveOptions<'a> {
    fn from(layer: &'a ExpressionLayer) -> Self {
        Self::new(layer.defaults(), layer.functions())
    }
}

/// Owned provenance for a resolved style value.
///
/// The variants intentionally carry ids and selector structures rather than
/// author-facing names. Mapping ids to user-visible names belongs to embedders
/// that own a [`StyleVocabulary`].
///
/// [`StyleVocabulary`]: crate::StyleVocabulary
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedSource {
    /// The object's animation slot won the highest precedence stage.
    Animation,
    /// A local-layer value won.
    ///
    /// `source` distinguishes explicit user/local writes from template binding
    /// and template-default values. Animation is represented separately by
    /// [`Self::Animation`].
    LocalOverride {
        /// The local-layer source that won.
        source: LocalValueSource,
    },
    /// A selector rule won the cascade stage.
    ///
    /// If `resource` is `Some`, the rule won the cascade but its value was
    /// dereferenced through that theme resource key.
    /// If `expression` is `Some`, the rule won with an evaluated style
    /// expression entry.
    CascadeRule {
        /// The winning rule origin.
        origin: StyleOrigin,
        /// The selector that matched the element.
        selector: Selector,
        /// The selector specificity used for cascade ordering.
        specificity: Specificity,
        /// Source index used for cascade ordering.
        source_index: usize,
        /// Rule order within its source.
        order: u32,
        /// Theme resource key used by the cascade entry, if any.
        resource: Option<ResourceKey>,
        /// Style expression entry used by the cascade entry, if any.
        expression: Option<StyleExpressionId>,
    },
    /// A direct style source won the cascade stage.
    ///
    /// If `resource` is `Some`, the direct style won the cascade but its value
    /// was dereferenced through that theme resource key.
    /// If `expression` is `Some`, the direct style won with an evaluated style
    /// expression entry.
    CascadeDirect {
        /// The winning direct style origin.
        origin: StyleOrigin,
        /// Source index used for cascade ordering.
        source_index: usize,
        /// Theme resource key used by the cascade entry, if any.
        resource: Option<ResourceKey>,
        /// Style expression entry used by the cascade entry, if any.
        expression: Option<StyleExpressionId>,
    },
    /// A property-level theme resource fallback supplied the value.
    ///
    /// This is distinct from `CascadeRule { resource: Some(_) }` or
    /// `CascadeDirect { resource: Some(_) }`: no cascade entry won here; the
    /// caller-provided resource fallback did.
    ThemeResource {
        /// The resource key that supplied the value.
        key: ResourceKey,
    },
    /// An ancestor supplied the value through inherited property resolution.
    Inherited {
        /// Number of parent hops from the original object to the winning ancestor.
        ancestor_depth: u16,
        /// The source that won on that ancestor.
        inner: Box<Self>,
    },
    /// The property registry default supplied the value.
    Default,
    /// A property-level expression default supplied the value.
    DefaultExpression {
        /// The property whose expression default supplied the value.
        property: PropertyId,
    },
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

/// Resolution context bundling registry, theme resources, expression state, and
/// parent lookup.
///
/// This avoids passing many parameters to resolution functions. Resolution uses
/// this precedence chain:
///
/// **Animation → Local → Style → Resource fallback → Inherited → Expression default → Default**
///
/// The resource fallback stage is present only when a [`ResolveQuery`] supplies
/// one. Theme resources are also consulted when a winning style entry or
/// expression token references a [`ResourceKey`].
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
///     ExpressionLayer, NoResolveParentLookup, ResolveCx, StyleCascadeBuilder, StyleBuilder,
///     StyleOrigin, ThemeBuilder,
/// };
/// use understory_property::{
///     DependencyObject, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
/// };
///
/// let mut registry = PropertyRegistry::new();
/// let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
///
/// let theme = ThemeBuilder::new().build();
/// let expressions = ExpressionLayer::new();
/// let style = StyleBuilder::new().set(width, 80.0).build();
/// let cascade = StyleCascadeBuilder::new()
///     .push_style(StyleOrigin::Override, style)
///     .build();
///
/// let cx = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
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
    /// Expression defaults and functions used during resolution.
    expr: ExprResolveOptions<'a>,
    /// Lookup used to walk inherited values.
    store_lookup: F,
    /// Phantom to hold K in the type.
    _marker: core::marker::PhantomData<K>,
}

/// Configurable property resolution query.
///
/// `ResolveQuery` is the advanced API for callers that need to opt into a
/// non-default resolution policy, such as combining style state with a
/// caller-supplied property-level resource fallback. Normal app code should
/// prefer [`ResolveCx::get_value`] or [`ResolveCx::explain_value`].
///
/// ```rust
/// # use understory_property::{DependencyObject, PropertyMetadataBuilder, PropertyRegistry, PropertyStore};
/// # use understory_style::{ExpressionLayer, NoResolveParentLookup, ResolveCx, ResourceKey, ThemeBuilder};
/// # struct Element { key: u32, store: PropertyStore<u32> }
/// # impl DependencyObject<u32> for Element {
/// #     fn property_store(&self) -> &PropertyStore<u32> { &self.store }
/// #     fn property_store_mut(&mut self) -> &mut PropertyStore<u32> { &mut self.store }
/// #     fn key(&self) -> u32 { self.key }
/// #     fn parent_key(&self) -> Option<u32> { None }
/// # }
/// const WIDTH_TOKEN: ResourceKey = ResourceKey::new(1);
///
/// let mut registry = PropertyRegistry::new();
/// let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
///
/// let theme = ThemeBuilder::new().set(WIDTH_TOKEN, 80.0_f64).build();
/// let expressions = ExpressionLayer::new();
/// let resolve = ResolveCx::new(&registry, &theme, &expressions, NoResolveParentLookup);
/// let element = Element { key: 1, store: PropertyStore::new(1) };
///
/// let value = resolve
///     .query(&element, width)
///     .resource_fallback(WIDTH_TOKEN)
///     .try_value()
///     .unwrap();
///
/// assert_eq!(value, 80.0);
/// ```
#[derive(Copy, Clone, Debug)]
pub struct ResolveQuery<'cx, 'a, K, F, O, T>
where
    K: Copy + Eq + 'a,
    F: ResolveParentLookup<'a, K>,
{
    resolve: &'cx ResolveCx<'a, K, F>,
    object: &'cx O,
    property: Property<T>,
    style: Option<(&'cx StyleCascade, MatchState)>,
    resource_fallback: Option<ResourceKey>,
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
            .field("expr", &self.expr)
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
    /// * `expr` - Expression defaults and functions
    /// * `store_lookup` - Lookup returning resolver parent data for a given key
    pub fn new(
        registry: &'a PropertyRegistry,
        theme: &'a Theme,
        expr: impl Into<ExprResolveOptions<'a>>,
        store_lookup: F,
    ) -> Self {
        Self {
            registry,
            theme,
            expr: expr.into(),
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

    /// Starts a configurable resolution query.
    ///
    /// Use this when a lookup needs options beyond the normal
    /// [`ResolveCx::get_value`] or [`ResolveCx::explain_value`] path, such as
    /// combining style state with a caller-supplied resource fallback.
    #[must_use]
    pub fn query<'cx, T, O>(
        &'cx self,
        object: &'cx O,
        property: Property<T>,
    ) -> ResolveQuery<'cx, 'a, K, F, O, T>
    where
        T: Clone + 'static,
        O: DependencyObject<K>,
    {
        ResolveQuery {
            resolve: self,
            object,
            property,
            style: None,
            resource_fallback: None,
        }
    }
}

impl<'cx, 'a, K, F, O, T> ResolveQuery<'cx, 'a, K, F, O, T>
where
    K: Copy + Eq + 'a,
    F: ResolveParentLookup<'a, K>,
    O: DependencyObject<K>,
    T: Clone + 'static,
{
    /// Adds matched style state to this query.
    #[must_use]
    pub fn style(mut self, cascade: &'cx StyleCascade, state: MatchState) -> Self {
        self.style = Some((cascade, state));
        self
    }

    /// Adds a caller-supplied property-level theme resource fallback.
    ///
    /// This fallback is checked after animation, local, and style values, and
    /// before inherited values, expression defaults, and registry defaults. It
    /// is distinct from
    /// [`expr::token`][understory_property_expression::expr::token], which is
    /// an inspectable dependency inside a style or default expression.
    #[must_use]
    pub fn resource_fallback(mut self, resource: ResourceKey) -> Self {
        self.resource_fallback = Some(resource);
        self
    }

    /// Resolves this query and returns an owned value.
    ///
    /// This is the fallible counterpart to [`ResolveCx::get_value`] for callers
    /// that want to inspect expression and configuration errors instead of
    /// treating them as invalid style setup.
    pub fn try_value(self) -> Result<T, ExprError> {
        self.resolve
            .resolve_query_value(
                self.object,
                self.property,
                self.style,
                self.resource_fallback,
            )
            .map(ResolvedValue::into_owned)
    }

    /// Resolves this query and explains which precedence stage supplied the
    /// value.
    ///
    /// This is the fallible counterpart to [`ResolveCx::explain_value`].
    pub fn try_explain(self) -> Result<Resolved<T>, ExprError> {
        self.resolve.explain_query_value(
            self.object,
            self.property,
            self.style,
            self.resource_fallback,
        )
    }
}

#[derive(Copy, Clone)]
struct ResolveSubject<'a, K: Copy + Eq> {
    store: &'a PropertyStore<K>,
    parent_key: Option<K>,
}

impl<'a, K: Copy + Eq> ResolveSubject<'a, K> {
    fn from_object<O>(object: &'a O) -> Self
    where
        O: DependencyObject<K>,
    {
        Self {
            store: object.property_store(),
            parent_key: object.parent_key(),
        }
    }

    fn from_parent(parent: ResolveParent<'a, K>) -> Self {
        Self {
            store: parent.store(),
            parent_key: parent.parent_key(),
        }
    }
}

fn resolution_panic<T>(property: PropertyId, err: ExprError) -> T {
    panic!("failed to resolve property {:?}: {:?}", property, err)
}

impl<'a, K, F> ResolveCx<'a, K, F>
where
    K: Copy + Eq + 'a,
    F: ResolveParentLookup<'a, K>,
{
    fn style_resolved_value<'cx, 's, T>(
        &'cx self,
        subject: ResolveSubject<'s, K>,
        cascade: &'cx StyleCascade,
        state: MatchState,
        property: Property<T>,
        expr: ExprResolveOptions<'cx>,
        in_flight: &mut InFlightProperties,
    ) -> Result<Option<ResolvedValue<'cx, T>>, ExprError>
    where
        T: Clone + 'static,
    {
        self.style_resolved_value_source(subject, cascade, state, property, expr, in_flight)
            .map(|value| value.map(|(value, _)| value))
    }

    fn style_resolved_value_source<'cx, 's, T>(
        &'cx self,
        subject: ResolveSubject<'s, K>,
        cascade: &'cx StyleCascade,
        state: MatchState,
        property: Property<T>,
        expr: ExprResolveOptions<'cx>,
        in_flight: &mut InFlightProperties,
    ) -> Result<Option<(ResolvedValue<'cx, T>, ResolvedSource)>, ExprError>
    where
        T: Clone + 'static,
    {
        let Some(source) = cascade.winning_source_for_id(state, property.id()) else {
            return Ok(None);
        };
        let source_metadata = provenance::source_metadata(&source);
        let Some(kind) = source.value_kind(property) else {
            return Ok(None);
        };
        match kind {
            StyleValueKind::Value(value) => Ok(Some((
                ResolvedValue::Borrowed(value),
                source_metadata.into_resolved_source(None, None),
            ))),
            StyleValueKind::Resource(key) => Ok(self.theme.get::<T>(key).map(|value| {
                (
                    ResolvedValue::Borrowed(value),
                    source_metadata.into_resolved_source(Some(key), None),
                )
            })),
            StyleValueKind::Expr(expr_ref) => {
                let value = self.eval_expression(
                    subject,
                    property,
                    Some((cascade, state)),
                    expr,
                    expr_ref.as_erased(),
                    in_flight,
                )?;
                Ok(Some((
                    ResolvedValue::Owned(value),
                    source_metadata.into_resolved_source(None, Some(expr_ref.expression_id())),
                )))
            }
        }
    }

    fn style_erased_resolved_value<'cx, 's>(
        &'cx self,
        subject: ResolveSubject<'s, K>,
        cascade: &'cx StyleCascade,
        state: MatchState,
        property: PropertyId,
        expr: ExprResolveOptions<'cx>,
        in_flight: &mut InFlightProperties,
    ) -> Result<Option<ErasedValue>, ExprError> {
        let Some(source) = cascade.winning_source_for_id(state, property) else {
            return Ok(None);
        };
        match source.value_kind_erased(property) {
            Some(ErasedStyleValueKind::Value(value)) => Ok(Some(value.clone())),
            Some(ErasedStyleValueKind::Resource(key)) => Ok(self.theme.get_erased(key)),
            Some(ErasedStyleValueKind::Expr(style_expr)) => {
                let expected = self.registry.get(property).map_or_else(
                    || style_expr.type_id(),
                    |registration| registration.type_id(),
                );
                self.eval_expression_erased(
                    subject,
                    property,
                    Some((cascade, state)),
                    expr,
                    style_expr,
                    expected,
                    in_flight,
                )
                .map(Some)
            }
            None => Ok(None),
        }
    }

    fn inherited_erased_value<'cx>(
        &'cx self,
        mut current_key: Option<K>,
        property: PropertyId,
        cascade: Option<&'cx StyleCascade>,
        expr: ExprResolveOptions<'cx>,
        in_flight: &mut InFlightProperties,
    ) -> Result<Option<ErasedValue>, ExprError> {
        while let Some(key) = current_key {
            let Some(parent) = self.store_lookup.lookup_resolve_parent(key) else {
                break;
            };

            if let Some(value) = parent.store().animation_erased(property) {
                return Ok(Some(value.clone()));
            }
            if let Some(value) = parent.store().local_winner_erased(property) {
                return Ok(Some(value.clone()));
            }
            if let (Some(cascade), Some(match_state)) = (cascade, parent.match_state())
                && let Some(value) = self.style_erased_resolved_value(
                    ResolveSubject::from_parent(parent),
                    cascade,
                    match_state,
                    property,
                    expr,
                    in_flight,
                )?
            {
                return Ok(Some(value));
            }

            current_key = parent.parent_key();
        }

        Ok(None)
    }

    fn inherited_resolved_value<'cx, T>(
        &'cx self,
        mut current_key: Option<K>,
        property: Property<T>,
        cascade: Option<&'cx StyleCascade>,
        expr: ExprResolveOptions<'cx>,
        in_flight: &mut InFlightProperties,
    ) -> Result<Option<ResolvedValue<'cx, T>>, ExprError>
    where
        T: Clone + 'static,
    {
        while let Some(key) = current_key {
            let Some(parent) = self.store_lookup.lookup_resolve_parent(key) else {
                break;
            };

            if let Some(value) = parent.store().get_animation(property) {
                return Ok(Some(ResolvedValue::Borrowed(value)));
            }
            if let Some(value) = parent.store().get_local(property) {
                return Ok(Some(ResolvedValue::Borrowed(value)));
            }
            if let (Some(cascade), Some(match_state)) = (cascade, parent.match_state())
                && let Some(value) = self.style_resolved_value(
                    ResolveSubject::from_parent(parent),
                    cascade,
                    match_state,
                    property,
                    expr,
                    in_flight,
                )?
            {
                return Ok(Some(value));
            }

            current_key = parent.parent_key();
        }

        Ok(None)
    }

    fn inherited_resolved_value_source<'cx, T>(
        &'cx self,
        mut current_key: Option<K>,
        property: Property<T>,
        cascade: Option<&'cx StyleCascade>,
        expr: ExprResolveOptions<'cx>,
        in_flight: &mut InFlightProperties,
    ) -> Result<Option<(ResolvedValue<'cx, T>, ResolvedSource)>, ExprError>
    where
        T: Clone + 'static,
    {
        let mut ancestor_depth = 1_u16;
        while let Some(key) = current_key {
            let Some(parent) = self.store_lookup.lookup_resolve_parent(key) else {
                break;
            };

            if let Some(value) = parent.store().get_animation(property) {
                return Ok(Some((
                    ResolvedValue::Borrowed(value),
                    ResolvedSource::Inherited {
                        ancestor_depth,
                        inner: Box::new(ResolvedSource::Animation),
                    },
                )));
            }
            if let Some(value) = parent.store().get_local(property) {
                let source = parent
                    .store()
                    .winning_local_source(property)
                    .unwrap_or(LocalValueSource::Local);
                return Ok(Some((
                    ResolvedValue::Borrowed(value),
                    ResolvedSource::Inherited {
                        ancestor_depth,
                        inner: Box::new(ResolvedSource::LocalOverride { source }),
                    },
                )));
            }
            if let (Some(cascade), Some(match_state)) = (cascade, parent.match_state())
                && let Some((value, source)) = self.style_resolved_value_source(
                    ResolveSubject::from_parent(parent),
                    cascade,
                    match_state,
                    property,
                    expr,
                    in_flight,
                )?
            {
                return Ok(Some((
                    value,
                    ResolvedSource::Inherited {
                        ancestor_depth,
                        inner: Box::new(source),
                    },
                )));
            }

            current_key = parent.parent_key();
            ancestor_depth = ancestor_depth.saturating_add(1);
        }

        Ok(None)
    }

    /// Resolves a property value through the expression-aware precedence chain.
    ///
    /// Precedence (highest to lowest):
    /// 1. Animation value on the object
    /// 2. Local value on the object
    /// 3. Style value (if style provided)
    /// 4. Inherited value (if property inherits, walks parent chain)
    /// 5. Expression default
    /// 6. Default value from registry
    ///
    /// # Panics
    ///
    /// Panics when expression evaluation or resolver configuration fails, such
    /// as a missing resource referenced by an expression, a type mismatch, or
    /// an expression cycle. Use [`Self::query`] and [`ResolveQuery::try_value`]
    /// when the caller needs to inspect those errors.
    ///
    /// # Arguments
    ///
    /// * `object` - The object to get the value for
    /// * `property` - The property to resolve
    /// * `style` - Optional matched style cascade and state to check for property values
    ///
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
        self.resolve_query_value(object, property, style, None)
            .map(ResolvedValue::into_owned)
            .unwrap_or_else(|err| resolution_panic(property.id(), err))
    }

    /// Resolves a property value and explains which precedence stage supplied it.
    ///
    /// This is an owned, cold-path inspection API. It mirrors
    /// [`Self::get_value`].
    ///
    /// # Panics
    ///
    /// Panics for the same invalid expression or resolver configuration cases
    /// as [`Self::get_value`]. Use [`Self::query`] and
    /// [`ResolveQuery::try_explain`] when the caller needs to inspect those
    /// errors.
    pub fn explain_value<T, O>(
        &self,
        object: &O,
        property: Property<T>,
        style: Option<(&StyleCascade, MatchState)>,
    ) -> Resolved<T>
    where
        T: Clone + 'static,
        O: DependencyObject<K>,
    {
        self.explain_query_value(object, property, style, None)
            .unwrap_or_else(|err| resolution_panic(property.id(), err))
    }

    fn resolve_query_value<'cx, T, O>(
        &'cx self,
        object: &'cx O,
        property: Property<T>,
        style: Option<(&'cx StyleCascade, MatchState)>,
        resource_fallback: Option<ResourceKey>,
    ) -> Result<ResolvedValue<'cx, T>, ExprError>
    where
        T: Clone + 'static,
        O: DependencyObject<K>,
    {
        let expr = self.expr;
        if let Some(value) = object.property_store().get_animation(property) {
            return Ok(ResolvedValue::Borrowed(value));
        }

        if let Some(value) = object.property_store().get_local(property) {
            return Ok(ResolvedValue::Borrowed(value));
        }

        let mut in_flight = InFlightProperties::new();
        if let Some((style, state)) = style
            && let Some(value) = self.style_resolved_value(
                ResolveSubject::from_object(object),
                style,
                state,
                property,
                expr,
                &mut in_flight,
            )?
        {
            return Ok(value);
        }

        if let Some(key) = resource_fallback
            && let Some(value) = self.theme.get::<T>(key)
        {
            return Ok(ResolvedValue::Borrowed(value));
        }

        let metadata = self.typed_metadata(property)?;
        if metadata.inherits()
            && let Some(value) = self.inherited_resolved_value(
                object.parent_key(),
                property,
                style.map(|s| s.0),
                expr,
                &mut in_flight,
            )?
        {
            return Ok(value);
        }

        if let Some(default_expr) = expr.defaults.get(property.id()) {
            let value = self.eval_default_expression(
                ResolveSubject::from_object(object),
                property,
                style,
                expr,
                default_expr,
                &mut in_flight,
            )?;
            return Ok(ResolvedValue::Owned(value));
        }

        Ok(ResolvedValue::Borrowed(metadata.default_value()))
    }

    fn explain_query_value<'cx, T, O>(
        &'cx self,
        object: &'cx O,
        property: Property<T>,
        style: Option<(&'cx StyleCascade, MatchState)>,
        resource_fallback: Option<ResourceKey>,
    ) -> Result<Resolved<T>, ExprError>
    where
        T: Clone + 'static,
        O: DependencyObject<K>,
    {
        let expr = self.expr;
        if let Some(value) = object.property_store().get_animation(property) {
            return Ok(Resolved {
                value: value.clone(),
                source: ResolvedSource::Animation,
            });
        }

        if let Some(value) = object.property_store().get_local(property) {
            let source = object
                .property_store()
                .winning_local_source(property)
                .unwrap_or(LocalValueSource::Local);
            return Ok(Resolved {
                value: value.clone(),
                source: ResolvedSource::LocalOverride { source },
            });
        }

        let mut in_flight = InFlightProperties::new();
        if let Some((style, state)) = style
            && let Some((value, source)) = self.style_resolved_value_source(
                ResolveSubject::from_object(object),
                style,
                state,
                property,
                expr,
                &mut in_flight,
            )?
        {
            return Ok(Resolved {
                value: value.into_owned(),
                source,
            });
        }

        if let Some(key) = resource_fallback
            && let Some(value) = self.theme.get::<T>(key)
        {
            return Ok(Resolved {
                value: value.clone(),
                source: ResolvedSource::ThemeResource { key },
            });
        }

        let metadata = self.typed_metadata(property)?;
        if metadata.inherits()
            && let Some((value, source)) = self.inherited_resolved_value_source(
                object.parent_key(),
                property,
                style.map(|s| s.0),
                expr,
                &mut in_flight,
            )?
        {
            return Ok(Resolved {
                value: value.into_owned(),
                source,
            });
        }

        if let Some(default_expr) = expr.defaults.get(property.id()) {
            let value = self.eval_default_expression(
                ResolveSubject::from_object(object),
                property,
                style,
                expr,
                default_expr,
                &mut in_flight,
            )?;
            return Ok(Resolved {
                value,
                source: ResolvedSource::DefaultExpression {
                    property: property.id(),
                },
            });
        }

        Ok(Resolved {
            value: metadata.default_value().clone(),
            source: ResolvedSource::Default,
        })
    }

    fn typed_metadata<T: Clone + 'static>(
        &self,
        property: Property<T>,
    ) -> Result<&understory_property::PropertyMetadata<T>, ExprError> {
        self.registry.get_metadata(property).ok_or_else(|| {
            if let Some(registration) = self.registry.get(property.id()) {
                ExprError::TypeMismatch {
                    expected: TypeId::of::<T>(),
                    actual: registration.type_id(),
                }
            } else {
                ExprError::MissingProperty {
                    property: property.id(),
                }
            }
        })
    }
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod resolve_tests;
