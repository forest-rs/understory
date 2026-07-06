// Copyright 2025 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shared style definitions.
//!
//! This module provides [`Style`], a shared collection of property setters
//! that can be referenced by multiple elements.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::TypeId;
use core::marker::PhantomData;

use understory_property::{ErasedValue, Property, PropertyId};
use understory_property_expression::{ErasedExpr, Expr, ExprDeps};

use crate::ResourceKey;

/// Per-style identifier for an expression style entry.
///
/// The identifier is stable for a built [`Style`] and is intended for
/// diagnostics and provenance. It is not stable across rebuilding the same
/// logical style and is not an expression arena node id.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct StyleExpressionId(u32);

impl StyleExpressionId {
    pub(crate) fn new(index: usize) -> Self {
        Self(u32::try_from(index).expect("too many style expressions"))
    }

    /// Returns this expression's compact index within its [`Style`].
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Borrowed typed reference to an expression stored in a [`Style`].
///
/// `ExprRef` is created only after checking the erased expression's result type
/// against the requested property type. It deliberately does not expose a
/// typed [`Expr`] because the style owns the erased arena.
#[derive(Copy, Clone, Debug)]
pub struct ExprRef<'a, T> {
    expr: &'a ErasedExpr,
    id: StyleExpressionId,
    _marker: PhantomData<fn() -> T>,
}

impl<'a, T: 'static> ExprRef<'a, T> {
    fn new(expr: &'a ErasedExpr, id: StyleExpressionId) -> Option<Self> {
        (expr.type_id() == TypeId::of::<T>()).then_some(Self {
            expr,
            id,
            _marker: PhantomData,
        })
    }

    /// Returns the expression entry id within the owning [`Style`].
    #[must_use]
    pub const fn expression_id(&self) -> StyleExpressionId {
        self.id
    }

    /// Returns the underlying erased expression.
    #[must_use]
    pub const fn as_erased(&self) -> &'a ErasedExpr {
        self.expr
    }
}

#[derive(Clone, Debug)]
enum StyleEntryValue {
    Literal(ErasedValue),
    Resource(ResourceKey),
    Expression {
        expr: ErasedExpr,
        id: StyleExpressionId,
    },
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum ErasedStyleValueKind<'a> {
    Value(&'a ErasedValue),
    Resource(ResourceKey),
    Expr(&'a ErasedExpr),
}

/// One style-layer entry for a property.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StyleValueRef<'a, T> {
    /// A concrete typed value stored directly in the style.
    Value(&'a T),
    /// A theme resource key to be resolved later.
    Resource(ResourceKey),
}

/// One typed style-layer entry for a property.
///
/// This is the expression-aware inspection API. The older [`StyleValueRef`]
/// remains expression-blind for borrowed legacy resolution.
#[derive(Copy, Clone, Debug)]
#[non_exhaustive]
pub enum StyleValueKind<'a, T> {
    /// A concrete typed value stored directly in the style.
    Value(&'a T),
    /// A theme resource key to be resolved later.
    Resource(ResourceKey),
    /// A typed expression stored in the style.
    Expr(ExprRef<'a, T>),
}

/// Borrowed reference to one expression style entry.
#[derive(Copy, Clone, Debug)]
pub struct StyleExpressionRef<'a> {
    property: PropertyId,
    expression: &'a ErasedExpr,
    id: StyleExpressionId,
}

impl<'a> StyleExpressionRef<'a> {
    /// Returns the property set by this expression entry.
    #[must_use]
    pub const fn property(self) -> PropertyId {
        self.property
    }

    /// Returns the expression entry id within the owning [`Style`].
    #[must_use]
    pub const fn expression_id(self) -> StyleExpressionId {
        self.id
    }

    /// Returns the erased expression.
    #[must_use]
    pub const fn expression(self) -> &'a ErasedExpr {
        self.expression
    }

    /// Returns the expression's static dependencies.
    #[must_use]
    pub fn deps(self) -> &'a ExprDeps {
        self.expression.deps()
    }
}

/// A shared, immutable collection of property setters.
///
/// Styles store property values once and can be shared across many elements.
/// This follows `WinUI`'s `OptimizedStyle` pattern for memory efficiency—rather
/// than storing style values per-element, elements hold a reference to a
/// shared style.
///
/// Styles are immutable after creation. Use [`StyleBuilder`] to construct them.
///
/// # Memory Layout
///
/// Internally, `Style` wraps an `Rc<StyleData>`, making cloning cheap (just
/// incrementing a reference count). The actual property values are stored once
/// in a sorted vector, similar to `PropertyStore`.
///
/// # Example
///
/// ```rust
/// use understory_style::{Style, StyleBuilder};
/// use understory_property::{PropertyMetadataBuilder, PropertyRegistry};
///
/// let mut registry = PropertyRegistry::new();
/// let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
///
/// let style = StyleBuilder::new()
///     .set(width, 100.0)
///     .build();
///
/// // Style can be cloned cheaply (`Rc`)
/// let style2 = style.clone();
///
/// assert_eq!(style.get(width), Some(&100.0));
/// assert_eq!(style2.get(width), Some(&100.0));
/// ```
#[derive(Clone, Debug)]
pub struct Style {
    inner: Rc<StyleData>,
}

/// Internal storage for style property values.
#[derive(Debug, Default)]
struct StyleData {
    /// Sorted by `PropertyId` for binary search lookup.
    entries: Vec<(PropertyId, StyleEntryValue)>,
}

impl Style {
    /// Returns `true` if this style has no property setters.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.entries.is_empty()
    }

    /// Returns the number of property setters in this style.
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.entries.len()
    }

    /// Gets the literal value for a property, if set in this style.
    ///
    /// Resource and expression entries are expression-blind here and return
    /// `None`. Use [`Self::value_kind`] to inspect all entry kinds.
    #[must_use]
    #[inline]
    pub fn get<T: Clone + 'static>(&self, property: Property<T>) -> Option<&T> {
        match self.value_ref(property)? {
            StyleValueRef::Value(value) => Some(value),
            StyleValueRef::Resource(_) => None,
        }
    }

    /// Returns `true` if this style has a value for the property.
    #[must_use]
    #[inline]
    pub fn contains<T: Clone + 'static>(&self, property: Property<T>) -> bool {
        self.inner
            .entries
            .binary_search_by_key(&property.id(), |(id, _)| *id)
            .is_ok()
    }

    pub(crate) fn contains_id(&self, property_id: PropertyId) -> bool {
        self.inner
            .entries
            .binary_search_by_key(&property_id, |(id, _)| *id)
            .is_ok()
    }

    /// Returns the theme resource key for a property, if this style references one.
    ///
    /// Expression entries are expression-blind here and return `None`. Use
    /// [`Self::value_kind`] to inspect all entry kinds.
    #[must_use]
    pub fn resource_key<T: Clone + 'static>(&self, property: Property<T>) -> Option<ResourceKey> {
        match self.value_ref(property)? {
            StyleValueRef::Value(_) => None,
            StyleValueRef::Resource(key) => Some(key),
        }
    }

    /// Returns an iterator over the property IDs set in this style.
    pub fn property_ids(&self) -> impl Iterator<Item = PropertyId> + '_ {
        self.inner.entries.iter().map(|(id, _)| *id)
    }

    /// Returns the borrowed legacy style entry for a property, if present.
    ///
    /// Expression entries are intentionally treated as absent by this API. Use
    /// [`Self::value_kind`] for expression-aware inspection.
    #[must_use]
    pub fn value_ref<T: Clone + 'static>(
        &self,
        property: Property<T>,
    ) -> Option<StyleValueRef<'_, T>> {
        let idx = self
            .inner
            .entries
            .binary_search_by_key(&property.id(), |(id, _)| *id)
            .ok()?;
        match &self.inner.entries[idx].1 {
            StyleEntryValue::Literal(value) => value.downcast_ref().map(StyleValueRef::Value),
            StyleEntryValue::Resource(key) => Some(StyleValueRef::Resource(*key)),
            StyleEntryValue::Expression { .. } => None,
        }
    }

    /// Returns the expression-aware style entry for a property, if present.
    #[must_use]
    pub fn value_kind<T: Clone + 'static>(
        &self,
        property: Property<T>,
    ) -> Option<StyleValueKind<'_, T>> {
        let idx = self
            .inner
            .entries
            .binary_search_by_key(&property.id(), |(id, _)| *id)
            .ok()?;
        match &self.inner.entries[idx].1 {
            StyleEntryValue::Literal(value) => value.downcast_ref().map(StyleValueKind::Value),
            StyleEntryValue::Resource(key) => Some(StyleValueKind::Resource(*key)),
            StyleEntryValue::Expression { expr, id } => {
                ExprRef::new(expr, *id).map(StyleValueKind::Expr)
            }
        }
    }

    /// Returns the dependencies for a property's expression style entry.
    ///
    /// Literal and resource entries return `None`.
    #[must_use]
    pub fn expression_deps<T: Clone + 'static>(&self, property: Property<T>) -> Option<&ExprDeps> {
        self.expression_deps_for_id(property.id())
    }

    /// Returns the dependencies for an expression style entry by property id.
    ///
    /// Literal and resource entries return `None`.
    #[must_use]
    pub fn expression_deps_for_id(&self, property: PropertyId) -> Option<&ExprDeps> {
        let idx = self
            .inner
            .entries
            .binary_search_by_key(&property, |(id, _)| *id)
            .ok()?;
        match &self.inner.entries[idx].1 {
            StyleEntryValue::Expression { expr, .. } => Some(expr.deps()),
            StyleEntryValue::Literal(_) | StyleEntryValue::Resource(_) => None,
        }
    }

    /// Returns this style's expression entries in property-id order.
    pub fn expression_entries(&self) -> impl Iterator<Item = StyleExpressionRef<'_>> {
        self.inner.entries.iter().filter_map(|(property, entry)| {
            if let StyleEntryValue::Expression { expr, id } = entry {
                Some(StyleExpressionRef {
                    property: *property,
                    expression: expr,
                    id: *id,
                })
            } else {
                None
            }
        })
    }

    pub(crate) fn value_kind_erased(
        &self,
        property: PropertyId,
    ) -> Option<ErasedStyleValueKind<'_>> {
        let idx = self
            .inner
            .entries
            .binary_search_by_key(&property, |(id, _)| *id)
            .ok()?;
        match &self.inner.entries[idx].1 {
            StyleEntryValue::Literal(value) => Some(ErasedStyleValueKind::Value(value)),
            StyleEntryValue::Resource(key) => Some(ErasedStyleValueKind::Resource(*key)),
            StyleEntryValue::Expression { expr, .. } => Some(ErasedStyleValueKind::Expr(expr)),
        }
    }
}

/// Builder for constructing [`Style`] instances.
///
/// # Example
///
/// ```rust
/// use understory_style::StyleBuilder;
/// use understory_property::{PropertyMetadataBuilder, PropertyRegistry};
///
/// let mut registry = PropertyRegistry::new();
/// let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
/// let height = registry.register("Height", PropertyMetadataBuilder::new(0.0_f64).build());
///
/// let style = StyleBuilder::new()
///     .set(width, 100.0)
///     .set(height, 50.0)
///     .build();
///
/// assert_eq!(style.get(width), Some(&100.0));
/// assert_eq!(style.get(height), Some(&50.0));
/// ```
#[derive(Debug, Default)]
pub struct StyleBuilder {
    entries: Vec<(PropertyId, StyleEntryValue)>,
}

impl StyleBuilder {
    /// Creates a new empty style builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a property value in the style.
    ///
    /// If the property was already set, the value is replaced.
    #[must_use]
    pub fn set<T: Clone + 'static>(mut self, property: Property<T>, value: T) -> Self {
        let id = property.id();
        let entry = StyleEntryValue::Literal(ErasedValue::new(value));

        match self.entries.binary_search_by_key(&id, |(pid, _)| *pid) {
            Ok(idx) => {
                self.entries[idx].1 = entry;
            }
            Err(idx) => {
                self.entries.insert(idx, (id, entry));
            }
        }
        self
    }

    /// Sets a property to resolve from a theme resource key.
    #[must_use]
    pub fn set_resource<T: Clone + 'static>(
        mut self,
        property: Property<T>,
        resource_key: ResourceKey,
    ) -> Self {
        let id = property.id();
        let entry = StyleEntryValue::Resource(resource_key);

        match self.entries.binary_search_by_key(&id, |(pid, _)| *pid) {
            Ok(idx) => {
                self.entries[idx].1 = entry;
            }
            Err(idx) => {
                self.entries.insert(idx, (id, entry));
            }
        }
        self
    }

    /// Sets a property to resolve from an expression.
    ///
    /// If the property was already set, the value is replaced.
    #[must_use]
    pub fn set_expr<T: Clone + 'static>(mut self, property: Property<T>, expr: Expr<T>) -> Self {
        let id = property.id();
        let entry = StyleEntryValue::Expression {
            expr: expr.into_erased(),
            id: StyleExpressionId::new(0),
        };

        match self.entries.binary_search_by_key(&id, |(pid, _)| *pid) {
            Ok(idx) => {
                self.entries[idx].1 = entry;
            }
            Err(idx) => {
                self.entries.insert(idx, (id, entry));
            }
        }
        self
    }

    /// Builds the style.
    #[must_use]
    pub fn build(mut self) -> Style {
        assign_expression_ids(&mut self.entries);
        Style {
            inner: Rc::new(StyleData {
                entries: self.entries,
            }),
        }
    }
}

fn assign_expression_ids(entries: &mut [(PropertyId, StyleEntryValue)]) {
    let mut next = 0;
    for (_, entry) in entries {
        if let StyleEntryValue::Expression { id, .. } = entry {
            *id = StyleExpressionId::new(next);
            next += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr;
    use understory_property::{PropertyMetadataBuilder, PropertyRegistry};
    use understory_property_expression::{Expr as PropertyExpr, ExprResourceKey};

    fn setup_registry() -> (PropertyRegistry, Property<f64>, Property<i32>) {
        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
        let count = registry.register("Count", PropertyMetadataBuilder::new(0_i32).build());
        (registry, width, count)
    }

    #[test]
    fn style_empty() {
        let style = StyleBuilder::new().build();
        assert!(style.is_empty());
        assert_eq!(style.len(), 0);
    }

    #[test]
    fn style_single_property() {
        let (_, width, _) = setup_registry();

        let style = StyleBuilder::new().set(width, 100.0).build();

        assert!(!style.is_empty());
        assert_eq!(style.len(), 1);
        assert_eq!(style.get(width), Some(&100.0));
    }

    #[test]
    fn style_multiple_properties() {
        let (_, width, count) = setup_registry();

        let style = StyleBuilder::new().set(width, 100.0).set(count, 42).build();

        assert_eq!(style.len(), 2);
        assert_eq!(style.get(width), Some(&100.0));
        assert_eq!(style.get(count), Some(&42));
    }

    #[test]
    fn style_replace_value() {
        let (_, width, _) = setup_registry();

        let style = StyleBuilder::new()
            .set(width, 100.0)
            .set(width, 200.0)
            .build();

        assert_eq!(style.len(), 1);
        assert_eq!(style.get(width), Some(&200.0));
    }

    #[test]
    fn style_contains() {
        let (_, width, count) = setup_registry();

        let style = StyleBuilder::new().set(width, 100.0).build();

        assert!(style.contains(width));
        assert!(!style.contains(count));
    }

    #[test]
    fn style_resource_property() {
        let (_, width, _) = setup_registry();
        let resource = ResourceKey::new(42);

        let style = StyleBuilder::new().set_resource(width, resource).build();

        assert!(style.contains(width));
        assert_eq!(style.get(width), None);
        assert_eq!(style.resource_key(width), Some(resource));
        assert_eq!(
            style.value_ref(width),
            Some(StyleValueRef::Resource(resource))
        );
    }

    #[test]
    fn style_expression_property_is_expression_blind_for_legacy_refs() {
        let (_, width, _) = setup_registry();

        let style = StyleBuilder::new()
            .set_expr(width, expr::lit(100.0))
            .build();

        assert!(style.contains(width));
        assert_eq!(style.get(width), None);
        assert_eq!(style.resource_key(width), None);
        assert!(style.value_ref(width).is_none());
    }

    #[test]
    fn style_value_kind_reports_expression() {
        let (_, width, _) = setup_registry();

        let style = StyleBuilder::new()
            .set_expr(width, expr::lit(100.0))
            .build();

        let Some(StyleValueKind::Expr(expr_ref)) = style.value_kind(width) else {
            panic!("expected expression style entry");
        };
        assert_eq!(expr_ref.expression_id().index(), 0);
        assert_eq!(expr_ref.as_erased().type_id(), TypeId::of::<f64>());
    }

    #[test]
    fn style_value_kind_rejects_wrong_typed_expression_ref() {
        let (_, width, _) = setup_registry();
        let wrong_width: Property<i32> = Property::from_id(width.id());

        let style = StyleBuilder::new()
            .set_expr(wrong_width, PropertyExpr::literal(12_i32))
            .build();

        assert!(style.value_kind(width).is_none());
    }

    #[test]
    fn style_expression_deps_report_only_expression_entries() {
        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
        let scale = registry.register("Scale", PropertyMetadataBuilder::new(1.0_f64).build());
        let count = registry.register("Count", PropertyMetadataBuilder::new(0_i32).build());
        const GAP: ResourceKey = ResourceKey::new(3);

        let style = StyleBuilder::new()
            .set(count, 4)
            .set_resource(scale, GAP)
            .set_expr(
                width,
                expr::cond(
                    expr::gt(expr::prop(scale), expr::token(GAP)),
                    expr::prop(scale) * 2.0,
                    expr::token(GAP) + 1.0,
                ),
            )
            .build();

        let deps = style.expression_deps(width).unwrap();
        assert_eq!(deps.properties.as_slice(), &[scale.id()]);
        assert_eq!(deps.resources.as_slice(), &[ExprResourceKey::from(GAP)]);
        assert_eq!(style.expression_deps_for_id(width.id()), Some(deps));
        assert!(style.expression_deps(scale).is_none());
        assert!(style.expression_deps(count).is_none());
    }

    #[test]
    fn style_expression_entries_expose_ids_targets_and_deps_in_property_order() {
        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
        let height = registry.register("Height", PropertyMetadataBuilder::new(0.0_f64).build());
        let count = registry.register("Count", PropertyMetadataBuilder::new(0_i32).build());
        const GAP: ResourceKey = ResourceKey::new(3);
        const EXTRA: ResourceKey = ResourceKey::new(4);

        let style = StyleBuilder::new()
            .set_expr(height, expr::token(EXTRA))
            .set(count, 4)
            .set_expr(width, expr::prop(height) + expr::token(GAP))
            .build();

        let entries: Vec<_> = style.expression_entries().collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].property(), width.id());
        assert_eq!(entries[0].expression_id().index(), 0);
        assert_eq!(entries[0].deps().properties.as_slice(), &[height.id()]);
        assert_eq!(
            entries[0].deps().resources.as_slice(),
            &[ExprResourceKey::from(GAP)]
        );
        assert_eq!(entries[1].property(), height.id());
        assert_eq!(entries[1].expression_id().index(), 1);
        assert_eq!(entries[1].deps().properties.as_slice(), &[]);
        assert_eq!(
            entries[1].deps().resources.as_slice(),
            &[ExprResourceKey::from(EXTRA)]
        );
        assert_eq!(entries[1].expression().type_id(), TypeId::of::<f64>());
    }

    #[test]
    fn style_clone_is_cheap() {
        let (_, width, _) = setup_registry();

        let style = StyleBuilder::new().set(width, 100.0).build();
        let style2 = style.clone();

        // Both reference the same data
        assert_eq!(style.get(width), Some(&100.0));
        assert_eq!(style2.get(width), Some(&100.0));

        // Rc makes this cheap
        assert!(Rc::ptr_eq(&style.inner, &style2.inner));
    }

    #[test]
    fn expression_style_clone_is_cheap() {
        let (_, width, _) = setup_registry();

        let style = StyleBuilder::new()
            .set_expr(width, expr::lit(100.0))
            .build();
        let style2 = style.clone();

        assert!(matches!(
            style.value_kind(width),
            Some(StyleValueKind::Expr(_))
        ));
        assert!(matches!(
            style2.value_kind(width),
            Some(StyleValueKind::Expr(_))
        ));
        assert!(Rc::ptr_eq(&style.inner, &style2.inner));
    }

    #[test]
    fn style_property_ids() {
        let (_, width, count) = setup_registry();

        let style = StyleBuilder::new().set(count, 42).set(width, 100.0).build();

        let ids: Vec<_> = style.property_ids().collect();
        assert_eq!(ids.len(), 2);
        // Should be sorted by PropertyId
        assert!(ids[0].index() < ids[1].index());
    }

    #[test]
    fn style_get_wrong_type_returns_none() {
        let (_, width, _) = setup_registry();

        let style = StyleBuilder::new().set(width, 100.0).build();

        // width is f64, trying to get as i32 fails
        let StyleEntryValue::Literal(value) = &style.inner.entries[0].1 else {
            panic!("expected literal style entry");
        };
        let wrong: Option<&i32> = value.downcast_ref();
        assert!(wrong.is_none());
    }
}
