// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Cross-property coercion support.
//!
//! This module provides [`CoerceContext`], the read-only view passed to
//! coerce callbacks so they can consult the owning object's *other* effective
//! property values, and [`CoerceRead`], the trait that supplies those reads.
//!
//! The canonical use case is `Slider.Value` coercion: `Value` must be clamped
//! against the **current** values of `Minimum` and `Maximum` on the same
//! object, not against constants baked in at registration time. See
//! [`PropertyMetadataBuilder::coerce_with`](crate::PropertyMetadataBuilder::coerce_with).

use crate::id::{Property, PropertyId};
use crate::registry::PropertyRegistry;
use crate::store::PropertyStore;
use crate::value::ErasedValue;

/// Read access to an object's effective property values for coercion.
///
/// [`PropertyStore`] implements this by resolving its effective-local value
/// (`Animation` → winning local-layer source). Higher-level crates that layer
/// style or theme resolution on top can implement this trait on their own
/// resolver types and build a [`CoerceContext`] from them, so coerce callbacks
/// observe the same values the rest of the system does.
pub trait CoerceRead {
    /// Returns the erased effective value for `id`, if the object has one.
    ///
    /// Returning `None` means "no explicit value is set"; [`CoerceContext`]
    /// then falls back to the registry default.
    fn effective_erased(&self, id: PropertyId) -> Option<&ErasedValue>;
}

impl<K: Copy + Eq> CoerceRead for PropertyStore<K> {
    fn effective_erased(&self, id: PropertyId) -> Option<&ErasedValue> {
        self.effective_local_erased(id)
    }
}

/// The context passed to coerce callbacks.
///
/// Provides read access to the owning object's other effective property
/// values (via a [`CoerceRead`] implementation, typically the object's
/// [`PropertyStore`]) and the [`PropertyRegistry`], so coercion can depend on
/// the current state of related properties.
///
/// # Example
///
/// ```rust
/// use understory_property::{
///     CoerceContext, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
/// };
///
/// let mut registry = PropertyRegistry::new();
/// let minimum = registry.register("Minimum", PropertyMetadataBuilder::new(0.0_f64).build());
/// let maximum = registry.register("Maximum", PropertyMetadataBuilder::new(100.0_f64).build());
/// let value = registry.register(
///     "Value",
///     PropertyMetadataBuilder::new(0.0_f64)
///         .coerce_with(move |cx: &CoerceContext<'_>, v: f64| {
///             v.max(cx.get(minimum)).min(cx.get(maximum))
///         })
///         .build(),
/// );
///
/// let mut store = PropertyStore::<u32>::new(1);
/// store.set_local(minimum, 10.0);
///
/// // The checked write paths coerce against the *current* Minimum/Maximum.
/// store.try_set_local(value, -5.0, &registry).unwrap();
/// assert_eq!(store.get_local(value), Some(&10.0));
/// ```
pub struct CoerceContext<'a> {
    reader: &'a dyn CoerceRead,
    registry: &'a PropertyRegistry,
}

impl<'a> CoerceContext<'a> {
    /// Creates a coerce context from a value reader and a registry.
    #[must_use]
    pub fn new(reader: &'a dyn CoerceRead, registry: &'a PropertyRegistry) -> Self {
        Self { reader, registry }
    }

    /// Returns the property registry.
    #[must_use]
    #[inline]
    pub fn registry(&self) -> &'a PropertyRegistry {
        self.registry
    }

    /// Returns the effective value of `property`, cloned.
    ///
    /// Resolves through the [`CoerceRead`] reader first (for a
    /// [`PropertyStore`] that is `Animation` → winning local-layer source),
    /// then falls back to the registry default.
    ///
    /// # Panics
    ///
    /// Panics if the property is not registered in the registry (matching
    /// [`PropertyStore::get_effective_local`]).
    #[must_use]
    pub fn get<T: Clone + 'static>(&self, property: Property<T>) -> T {
        self.get_ref(property).clone()
    }

    /// Returns the effective value of `property`, borrowed.
    ///
    /// # Panics
    ///
    /// Panics if the property is not registered in the registry.
    #[must_use]
    pub fn get_ref<T: Clone + 'static>(&self, property: Property<T>) -> &'a T {
        if let Some(v) = self
            .reader
            .effective_erased(property.id())
            .and_then(ErasedValue::downcast_ref)
        {
            return v;
        }
        if let Some(metadata) = self.registry.get_metadata(property) {
            return metadata.default_value();
        }
        panic!("Property {:?} not found in registry", property.id());
    }
}

impl core::fmt::Debug for CoerceContext<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CoerceContext")
            .field("registry", &self.registry)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::PropertyMetadataBuilder;

    #[test]
    fn context_reads_effective_values() {
        let mut registry = PropertyRegistry::new();
        let minimum = registry.register("Minimum", PropertyMetadataBuilder::new(0.0_f64).build());
        let maximum = registry.register("Maximum", PropertyMetadataBuilder::new(100.0_f64).build());

        let mut store = PropertyStore::<u32>::new(1);
        store.set_local(minimum, 25.0);

        let cx = CoerceContext::new(&store, &registry);
        // Local value visible through the context.
        assert_eq!(cx.get(minimum), 25.0);
        // Falls back to registry default when nothing is stored.
        assert_eq!(cx.get(maximum), 100.0);
        assert_eq!(cx.get_ref(maximum), &100.0);
    }

    #[test]
    fn context_prefers_animation_over_local() {
        let mut registry = PropertyRegistry::new();
        let minimum = registry.register("Minimum", PropertyMetadataBuilder::new(0.0_f64).build());

        let mut store = PropertyStore::<u32>::new(1);
        store.set_local(minimum, 25.0);
        store.set_animation(minimum, 50.0);

        let cx = CoerceContext::new(&store, &registry);
        assert_eq!(cx.get(minimum), 50.0);
    }

    #[test]
    #[should_panic(expected = "not found in registry")]
    fn context_panics_on_unregistered_property() {
        let registry = PropertyRegistry::new();
        let store = PropertyStore::<u32>::new(1);
        let bogus: Property<f64> = Property::from_id(PropertyId::new(7));

        let cx = CoerceContext::new(&store, &registry);
        let _ = cx.get(bogus);
    }
}
