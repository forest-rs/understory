// Copyright 2025 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Per-object sparse property storage.
//!
//! This module provides [`PropertyStore`] for storing property values on objects,
//! using sparse storage to minimize memory for objects with few properties set.
//!
//! # Implementation
//!
//! Following the `WinUI` approach, we use a sorted vector with binary search rather
//! than a hash map. This provides:
//!
//! - Better cache locality (contiguous memory)
//! - Lower memory overhead (no hash buckets)
//! - O(log n) lookup, which is fast for typical property counts (5-20)
//! - Inline storage for small property sets via `SmallVec`
//!
//! # Scope
//!
//! `PropertyStore` handles **local storage only** — Local and Animation values.
//! Style resolution, theme resolution, and inheritance live in higher-level
//! crates (see `understory_style`); this store is intentionally narrower.
//!
//! # Layered Local Storage
//!
//! Within the local layer, each [`LocalValueSource`] is a *separate sparse
//! store*. A write goes to its source's slot and leaves the other slots
//! untouched, so clearing a higher source reveals any lower source that was
//! previously installed. See [`LocalValueSource`] for the precedence order.
//!
//! # Memory Footprint
//!
//! A [`PropertyStore`] carries one `SmallVec` (the `Local` slot, with inline
//! capacity for the common case) plus three `Vec`s (`TemplateBinding`,
//! `TemplateDefault`, and `Animation`). The three `Vec`s do not allocate until
//! their respective source writes — non-templated objects with no animations
//! pay only the size of three empty `Vec` headers, not three heap
//! allocations. This is a real but small per-object cost increase over the
//! pre-layering design; if it becomes load-bearing we can revisit (e.g. lazy
//! `Option<Box<...>>` for the template slots).

use alloc::vec::Vec;
use core::any::TypeId;
use invalidation::ChannelSet;
use smallvec::SmallVec;

use crate::id::{Property, PropertyId};
use crate::registry::PropertyRegistry;
use crate::value::ErasedValue;

/// Default inline capacity for property entries.
///
/// Most UI objects have fewer than 8 non-default properties set, so this avoids
/// heap allocation in the common case.
const INLINE_CAPACITY: usize = 8;

/// Identifies which writer installed a value in the local layer.
///
/// The local layer holds one value *per source per property*. When
/// [`PropertyStore::get_local`] resolves, it walks sources in precedence order
/// and returns the highest source that has a value. Clearing a higher source
/// reveals whatever a lower source had previously written.
///
/// Precedence (highest to lowest):
///
/// 1. [`Local`](Self::Local) — explicit user write via `set_local`.
/// 2. [`TemplateBinding`](Self::TemplateBinding) — a value pushed by a binding
///    subscription from a templated parent.
/// 3. [`TemplateDefault`](Self::TemplateDefault) — initial value written by a
///    control template at instantiation time.
///
/// This enum is `#[non_exhaustive]` so we can add sources later without a
/// breaking change.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LocalValueSource {
    /// Set by user code via [`PropertyStore::set_local`] or the notifying
    /// variants on [`crate::DependencyObjectExt`].
    Local,
    /// Forwarded by a template binding subscription (e.g. a templated parent
    /// pushed its property value to a template part).
    TemplateBinding,
    /// Written by a control template during instantiation as the part's initial
    /// value.
    TemplateDefault,
}

impl LocalValueSource {
    /// Returns the ordinal precedence of this source.
    ///
    /// Higher values win when [`PropertyStore::get_local`] resolves. See the
    /// [`LocalValueSource`] docs for the full order.
    #[must_use]
    #[inline]
    pub const fn precedence(self) -> u8 {
        match self {
            Self::Local => 3,
            Self::TemplateBinding => 2,
            Self::TemplateDefault => 1,
        }
    }
}

type Entry = (PropertyId, ErasedValue);

/// Type-id mismatch reported by checked type-erased setters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErasedTypeMismatch {
    /// The value type registered for the property.
    pub expected: TypeId,
    /// The value type carried by the supplied [`ErasedValue`].
    pub actual: TypeId,
}

/// Shared helpers for the sorted sparse layouts of [`Entry`] used by every
/// source's slot. Both `Vec<Entry>` and `SmallVec<[Entry; _]>` get the same
/// API via the macro impl below.
trait EntrySlice {
    fn entry_find(&self, id: PropertyId) -> Result<usize, usize>;
    fn entry_get(&self, id: PropertyId) -> Option<&ErasedValue>;
}

trait EntryStore: EntrySlice {
    fn entry_write(&mut self, id: PropertyId, value: ErasedValue);
    fn entry_clear(&mut self, id: PropertyId) -> bool;
}

macro_rules! impl_entry_store {
    ($ty:ty) => {
        impl EntrySlice for $ty {
            #[inline]
            fn entry_find(&self, id: PropertyId) -> Result<usize, usize> {
                self.binary_search_by_key(&id, |(pid, _)| *pid)
            }

            #[inline]
            fn entry_get(&self, id: PropertyId) -> Option<&ErasedValue> {
                self.entry_find(id).ok().map(|idx| &self[idx].1)
            }
        }

        impl EntryStore for $ty {
            fn entry_write(&mut self, id: PropertyId, value: ErasedValue) {
                match self.entry_find(id) {
                    Ok(idx) => self[idx].1 = value,
                    Err(idx) => self.insert(idx, (id, value)),
                }
            }

            fn entry_clear(&mut self, id: PropertyId) -> bool {
                if let Ok(idx) = self.entry_find(id) {
                    self.remove(idx);
                    true
                } else {
                    false
                }
            }
        }
    };
}

impl_entry_store!(Vec<Entry>);
impl_entry_store!(SmallVec<[Entry; INLINE_CAPACITY]>);

/// Per-object sparse storage for property values.
///
/// Stores Local and Animation values only. Style/theme resolution and inheritance
/// are handled by higher-level APIs (see `understory_style`).
///
/// # Storage Strategy
///
/// Uses sorted sparse stores with binary search, following the `WinUI`
/// `vector_map` approach. This provides O(log n) lookup with excellent cache
/// locality. Each store is independent so a write to one source slot does not
/// touch any other.
///
/// # Precedence
///
/// `get_effective_local` resolves in this order: **Animation** → **local layer**
/// (highest source present) → registry default. Within the local layer, sources
/// are ranked by [`LocalValueSource::precedence`].
///
/// # Example
///
/// ```rust
/// use understory_property::{PropertyStore, PropertyMetadataBuilder, PropertyRegistry};
///
/// let mut registry = PropertyRegistry::new();
/// let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
///
/// let mut store = PropertyStore::<u32>::new(1);
///
/// // No value set - uses default
/// assert!(store.get_local(width).is_none());
///
/// // Set local value
/// store.set_local(width, 100.0);
/// assert_eq!(store.get_local(width), Some(&100.0));
///
/// // Animation overrides local
/// store.set_animation(width, 200.0);
/// let effective = store.get_effective_local(width, &registry);
/// assert_eq!(effective, 200.0);
/// ```
#[derive(Debug)]
pub struct PropertyStore<K> {
    /// Local slot — user-driven; highest precedence within the local layer.
    local_entries: SmallVec<[Entry; INLINE_CAPACITY]>,
    /// Template-binding slot — pushed by binding subscriptions.
    template_binding_entries: Vec<Entry>,
    /// Template-default slot — written by control templates on instantiation.
    template_default_entries: Vec<Entry>,
    /// Animation slot — highest precedence overall.
    ///
    /// Stored out-of-line so that objects with no animation values pay minimal
    /// per-object overhead.
    animation_entries: Vec<Entry>,
    owner: K,
}

impl<K: Copy + Eq> PropertyStore<K> {
    /// Creates a new property store for the given owner key.
    #[must_use]
    pub fn new(owner: K) -> Self {
        Self {
            local_entries: SmallVec::new(),
            template_binding_entries: Vec::new(),
            template_default_entries: Vec::new(),
            animation_entries: Vec::new(),
            owner,
        }
    }

    /// Returns the owner key of this store.
    #[must_use]
    #[inline]
    pub fn owner(&self) -> K {
        self.owner
    }

    /// Returns `true` if no properties have explicit values set in any slot.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.local_entries.is_empty()
            && self.template_binding_entries.is_empty()
            && self.template_default_entries.is_empty()
            && self.animation_entries.is_empty()
    }

    /// Returns the number of unique properties with at least one value set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.property_ids().count()
    }

    /// Returns the property IDs that have values set, deduplicated across all
    /// source slots and the animation slot, in ascending order.
    pub fn property_ids(&self) -> impl Iterator<Item = PropertyId> + '_ {
        PropertyIds::new([
            self.local_entries.as_slice(),
            self.template_binding_entries.as_slice(),
            self.template_default_entries.as_slice(),
            self.animation_entries.as_slice(),
        ])
    }

    // =========================================================================
    // Local layer — internal helpers
    // =========================================================================

    /// Looks up the highest local-layer source that has a value for `id`.
    fn local_layer_lookup(&self, id: PropertyId) -> Option<(LocalValueSource, &ErasedValue)> {
        if let Some(v) = self.local_entries.entry_get(id) {
            return Some((LocalValueSource::Local, v));
        }
        if let Some(v) = self.template_binding_entries.entry_get(id) {
            return Some((LocalValueSource::TemplateBinding, v));
        }
        if let Some(v) = self.template_default_entries.entry_get(id) {
            return Some((LocalValueSource::TemplateDefault, v));
        }
        None
    }

    /// Returns the value at a specific local-layer source, if any.
    fn slot_get(&self, id: PropertyId, source: LocalValueSource) -> Option<&ErasedValue> {
        match source {
            LocalValueSource::Local => self.local_entries.entry_get(id),
            LocalValueSource::TemplateBinding => self.template_binding_entries.entry_get(id),
            LocalValueSource::TemplateDefault => self.template_default_entries.entry_get(id),
        }
    }

    fn slot_write(&mut self, id: PropertyId, source: LocalValueSource, value: ErasedValue) {
        match source {
            LocalValueSource::Local => self.local_entries.entry_write(id, value),
            LocalValueSource::TemplateBinding => {
                self.template_binding_entries.entry_write(id, value);
            }
            LocalValueSource::TemplateDefault => {
                self.template_default_entries.entry_write(id, value);
            }
        }
    }

    fn slot_clear_id(&mut self, id: PropertyId, source: LocalValueSource) -> bool {
        match source {
            LocalValueSource::Local => self.local_entries.entry_clear(id),
            LocalValueSource::TemplateBinding => self.template_binding_entries.entry_clear(id),
            LocalValueSource::TemplateDefault => self.template_default_entries.entry_clear(id),
        }
    }

    fn slot_property_ids(&self, source: LocalValueSource) -> impl Iterator<Item = PropertyId> + '_ {
        let slice: &[Entry] = match source {
            LocalValueSource::Local => self.local_entries.as_slice(),
            LocalValueSource::TemplateBinding => self.template_binding_entries.as_slice(),
            LocalValueSource::TemplateDefault => self.template_default_entries.as_slice(),
        };
        slice.iter().map(|(id, _)| *id)
    }

    // =========================================================================
    // Local value methods
    // =========================================================================

    /// Gets the local value, resolving the winning source within the local layer.
    #[must_use]
    #[inline]
    pub fn get_local<T: Clone + 'static>(&self, property: Property<T>) -> Option<&T> {
        self.local_layer_lookup(property.id())
            .and_then(|(_, v)| v.downcast_ref())
    }

    /// Returns the [`LocalValueSource`] currently winning the local-layer
    /// resolution for `property`, if any source has a value.
    #[must_use]
    #[inline]
    pub fn get_local_source<T: Clone + 'static>(
        &self,
        property: Property<T>,
    ) -> Option<LocalValueSource> {
        self.local_layer_lookup(property.id()).map(|(s, _)| s)
    }

    /// Gets the value at a specific local-layer source, if that source has one.
    #[must_use]
    #[inline]
    pub fn get_local_at_source<T: Clone + 'static>(
        &self,
        property: Property<T>,
        source: LocalValueSource,
    ) -> Option<&T> {
        self.slot_get(property.id(), source)
            .and_then(ErasedValue::downcast_ref)
    }

    /// Returns the erased value from the winning local-layer source for `id`.
    ///
    /// This walks the [`LocalValueSource`] precedence order
    /// (`Local` → `TemplateBinding` → `TemplateDefault`) and returns the first
    /// value present. It does not consult animation values; use
    /// [`effective_local_erased`](Self::effective_local_erased) for
    /// `Animation` → local-layer resolution.
    #[must_use]
    #[inline]
    pub fn local_winner_erased(&self, id: PropertyId) -> Option<&ErasedValue> {
        self.local_layer_lookup(id).map(|(_, value)| value)
    }

    /// Returns the erased animation value for `id`, if one is set.
    #[must_use]
    #[inline]
    pub fn animation_erased(&self, id: PropertyId) -> Option<&ErasedValue> {
        self.animation_entries.entry_get(id)
    }

    /// Returns the erased effective-local value for `id`, if one is set.
    ///
    /// This resolves only `Animation` → winning local-layer value. It does not
    /// include style, theme, inherited, or registry-default values; those are
    /// resolved by higher-level crates.
    #[must_use]
    #[inline]
    pub fn effective_local_erased(&self, id: PropertyId) -> Option<&ErasedValue> {
        self.animation_erased(id)
            .or_else(|| self.local_winner_erased(id))
    }

    /// Returns the erased value at a specific local-layer source slot.
    #[must_use]
    #[inline]
    pub fn local_erased_at_source(
        &self,
        id: PropertyId,
        source: LocalValueSource,
    ) -> Option<&ErasedValue> {
        self.slot_get(id, source)
    }

    /// Returns `true` if any local-layer source has a value for `property`.
    #[must_use]
    #[inline]
    pub fn has_local<T: Clone + 'static>(&self, property: Property<T>) -> bool {
        self.local_layer_lookup(property.id()).is_some()
    }

    /// Returns `true` if the specific source has a value for `property`.
    #[must_use]
    #[inline]
    pub fn has_local_at_source<T: Clone + 'static>(
        &self,
        property: Property<T>,
        source: LocalValueSource,
    ) -> bool {
        self.slot_get(property.id(), source).is_some()
    }

    /// Sets the local value at [`LocalValueSource::Local`].
    ///
    /// This is the user-code entry point. To write to another source slot, use
    /// [`set_local_with_source`](Self::set_local_with_source).
    ///
    /// Returns a reference to the stored value.
    pub fn set_local<T: Clone + 'static>(&mut self, property: Property<T>, value: T) -> &T {
        self.slot_write(
            property.id(),
            LocalValueSource::Local,
            ErasedValue::new(value),
        );
        // `get_local` returns the highest-source value; with Local just written,
        // that's the value we just stored.
        self.get_local(property).unwrap()
    }

    /// Writes a value to the slot for `source`.
    ///
    /// Each source has its own sparse slot, so this write never overwrites a
    /// value held by another source — it only replaces (or inserts) the entry
    /// in this source's slot. The new value becomes the winning local value
    /// only if `source` has the highest precedence among the sources that
    /// currently have an entry for this property.
    ///
    /// # Example
    ///
    /// ```rust
    /// use understory_property::{
    ///     LocalValueSource, Property, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
    /// };
    ///
    /// let mut registry = PropertyRegistry::new();
    /// let width: Property<f64> =
    ///     registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
    ///
    /// let mut store = PropertyStore::<u32>::new(1);
    ///
    /// // Template installs a default.
    /// store.set_local_with_source(width, 10.0, LocalValueSource::TemplateDefault);
    /// assert_eq!(store.get_local(width), Some(&10.0));
    ///
    /// // User writes a Local. Local wins.
    /// store.set_local_with_source(width, 50.0, LocalValueSource::Local);
    /// assert_eq!(store.get_local(width), Some(&50.0));
    ///
    /// // Clearing Local reveals TemplateDefault again.
    /// store.clear_local(width);
    /// assert_eq!(store.get_local(width), Some(&10.0));
    /// ```
    pub fn set_local_with_source<T: Clone + 'static>(
        &mut self,
        property: Property<T>,
        value: T,
        source: LocalValueSource,
    ) {
        self.slot_write(property.id(), source, ErasedValue::new(value));
    }

    /// Writes an erased value to the slot for `source`.
    ///
    /// # Type Contract
    ///
    /// `value.type_id()` must match the [`PropertyRegistration`] type for
    /// `id`. This unchecked setter does not validate the contract; callers
    /// should validate through [`PropertyRegistry::get`] or use
    /// [`set_local_erased_with_source_checked`](Self::set_local_erased_with_source_checked).
    ///
    /// [`PropertyRegistration`]: crate::PropertyRegistration
    pub fn set_local_erased_with_source(
        &mut self,
        id: PropertyId,
        value: ErasedValue,
        source: LocalValueSource,
    ) {
        self.slot_write(id, source, value);
    }

    /// Writes an erased value to the slot for `source` after checking its type.
    ///
    /// The value's [`TypeId`] is compared with the type registered for `id` in
    /// `registry`. On mismatch the store is left unchanged and
    /// [`ErasedTypeMismatch`] is returned.
    ///
    /// # Panics
    ///
    /// Panics if `id` is not registered in `registry`.
    pub fn set_local_erased_with_source_checked(
        &mut self,
        id: PropertyId,
        value: ErasedValue,
        source: LocalValueSource,
        registry: &PropertyRegistry,
    ) -> Result<(), ErasedTypeMismatch> {
        let expected = registry
            .get(id)
            .unwrap_or_else(|| panic!("Property {id:?} not found in registry"))
            .type_id();
        let actual = value.type_id();
        if actual != expected {
            return Err(ErasedTypeMismatch { expected, actual });
        }

        self.set_local_erased_with_source(id, value, source);
        Ok(())
    }

    /// Writes an erased source-tagged local value and returns affected channels.
    ///
    /// This uses the same type check as
    /// [`set_local_erased_with_source_checked`](Self::set_local_erased_with_source_checked).
    /// After a successful write it conservatively returns the full
    /// `affects_channels` set for `id`: the erased path cannot compare the old
    /// and new typed effective values, so it over-invalidates rather than
    /// missing a possible change. Callers that need exact no-op suppression
    /// should use the typed notifying path.
    ///
    /// # Panics
    ///
    /// Panics if `id` is not registered in `registry`.
    pub fn set_local_erased_with_source_notifying(
        &mut self,
        id: PropertyId,
        value: ErasedValue,
        source: LocalValueSource,
        registry: &PropertyRegistry,
    ) -> Result<ChannelSet, ErasedTypeMismatch> {
        self.set_local_erased_with_source_checked(id, value, source, registry)?;
        Ok(registry.affects_channels(id))
    }

    /// Clears the value at [`LocalValueSource::Local`] only.
    ///
    /// Other source slots (e.g. `TemplateDefault`) are untouched and may become
    /// the new winning value after this call. To remove a value at a different
    /// source, use [`clear_local_at_source`](Self::clear_local_at_source); to
    /// remove every source slot for one property, use
    /// [`clear_all`](Self::clear_all).
    ///
    /// Returns `true` if a Local entry was removed.
    pub fn clear_local<T: Clone + 'static>(&mut self, property: Property<T>) -> bool {
        self.slot_clear_id(property.id(), LocalValueSource::Local)
    }

    /// Clears the value at [`LocalValueSource::Local`] for a type-erased property id.
    ///
    /// This is the erased twin of [`Self::clear_local`]. It is useful for
    /// binding hosts and adapters that carry [`PropertyId`] values rather than
    /// typed [`Property<T>`] handles.
    ///
    /// Other source slots are untouched and may become the new winning local
    /// value after this call.
    ///
    /// Returns `true` if a Local entry was removed.
    pub fn clear_local_erased(&mut self, id: PropertyId) -> bool {
        self.slot_clear_id(id, LocalValueSource::Local)
    }

    /// Clears the value at a specific local-layer source for `property`.
    ///
    /// Returns `true` if a value was removed from that source's slot.
    pub fn clear_local_at_source<T: Clone + 'static>(
        &mut self,
        property: Property<T>,
        source: LocalValueSource,
    ) -> bool {
        self.slot_clear_id(property.id(), source)
    }

    /// Clears the value at a specific local-layer source for a type-erased property id.
    ///
    /// This is the erased twin of [`Self::clear_local_at_source`].
    ///
    /// Returns `true` if a value was removed from that source's slot.
    pub fn clear_local_erased_at_source(
        &mut self,
        id: PropertyId,
        source: LocalValueSource,
    ) -> bool {
        self.slot_clear_id(id, source)
    }

    /// Clears the Local slot for a type-erased property id and returns affected channels.
    ///
    /// This is the erased twin of [`Self::clear_local_erased`]. It returns a
    /// conservative `affects_channels` set when clearing the Local slot may
    /// change the effective local value. It does not run typed
    /// `on_changed` callbacks or compare the value revealed underneath; callers
    /// that need exact callback behavior should use typed
    /// [`DependencyObjectExt::clear_local_notifying`](crate::DependencyObjectExt::clear_local_notifying).
    pub fn clear_local_erased_notifying(
        &mut self,
        id: PropertyId,
        registry: &PropertyRegistry,
    ) -> ChannelSet {
        self.clear_local_erased_at_source_notifying(id, LocalValueSource::Local, registry)
    }

    /// Clears a source slot for a type-erased property id and returns affected channels.
    ///
    /// A property contributes channels only when `source` currently wins the
    /// local layer and no animation value masks it. The result is conservative:
    /// if clearing `source` reveals an equal value from a lower source, this
    /// still returns the property's channels because the erased path cannot do
    /// typed equality. Use typed
    /// [`DependencyObjectExt::clear_local_at_source_notifying`](crate::DependencyObjectExt::clear_local_at_source_notifying)
    /// for exact value comparison and changed callbacks.
    pub fn clear_local_erased_at_source_notifying(
        &mut self,
        id: PropertyId,
        source: LocalValueSource,
        registry: &PropertyRegistry,
    ) -> ChannelSet {
        if self.slot_get(id, source).is_none() {
            return ChannelSet::empty();
        }

        let may_affect =
            !self.animation_entries_has(id) && self.winning_local_source_for_id(id) == Some(source);
        self.slot_clear_id(id, source);

        if may_affect {
            registry.affects_channels(id)
        } else {
            ChannelSet::empty()
        }
    }

    /// Clears every value held by the given source.
    ///
    /// Returns the number of entries removed. This is the bulk cleanup hook —
    /// e.g. a template tear-down calling
    /// `clear_local_by_source(LocalValueSource::TemplateDefault)` (and
    /// `TemplateBinding`) drops every value the template installed without
    /// touching anything stored under `Local`.
    ///
    /// For change notification, see
    /// [`DependencyObjectExt::clear_local_by_source_notifying`].
    ///
    /// [`DependencyObjectExt::clear_local_by_source_notifying`]:
    /// crate::DependencyObjectExt::clear_local_by_source_notifying
    pub fn clear_local_by_source(&mut self, source: LocalValueSource) -> usize {
        match source {
            LocalValueSource::Local => {
                let n = self.local_entries.len();
                self.local_entries.clear();
                n
            }
            LocalValueSource::TemplateBinding => {
                let n = self.template_binding_entries.len();
                self.template_binding_entries.clear();
                n
            }
            LocalValueSource::TemplateDefault => {
                let n = self.template_default_entries.len();
                self.template_default_entries.clear();
                n
            }
        }
    }

    /// Returns the property ids that currently have an entry under `source`,
    /// in ascending order.
    pub fn local_property_ids_at_source(
        &self,
        source: LocalValueSource,
    ) -> impl Iterator<Item = PropertyId> + '_ {
        self.slot_property_ids(source)
    }

    /// Returns the winning local-layer source for `id`, if any source has a
    /// value. This is the type-erased twin of [`get_local_source`].
    ///
    /// [`get_local_source`]: Self::get_local_source
    #[must_use]
    pub fn winning_local_source_for_id(&self, id: PropertyId) -> Option<LocalValueSource> {
        self.local_layer_lookup(id).map(|(s, _)| s)
    }

    /// Returns `true` if the animation slot holds an entry for `id`. This is
    /// the type-erased twin of [`has_animation`].
    ///
    /// [`has_animation`]: Self::has_animation
    #[must_use]
    pub fn animation_entries_has(&self, id: PropertyId) -> bool {
        self.animation_entries.entry_find(id).is_ok()
    }

    // =========================================================================
    // Animation value methods
    // =========================================================================

    /// Gets the animation value, if set.
    #[must_use]
    #[inline]
    pub fn get_animation<T: Clone + 'static>(&self, property: Property<T>) -> Option<&T> {
        self.animation_entries
            .entry_get(property.id())
            .and_then(ErasedValue::downcast_ref)
    }

    /// Sets the animation value.
    ///
    /// Returns a reference to the stored value.
    pub fn set_animation<T: Clone + 'static>(&mut self, property: Property<T>, value: T) -> &T {
        let id = property.id();
        self.animation_entries
            .entry_write(id, ErasedValue::new(value));
        self.get_animation(property).unwrap()
    }

    /// Clears the animation value.
    ///
    /// Returns `true` if a value was removed.
    pub fn clear_animation<T: Clone + 'static>(&mut self, property: Property<T>) -> bool {
        self.animation_entries.entry_clear(property.id())
    }

    /// Returns `true` if the property has an animation value.
    #[must_use]
    #[inline]
    pub fn has_animation<T: Clone + 'static>(&self, property: Property<T>) -> bool {
        self.animation_entries.entry_find(property.id()).is_ok()
    }

    // =========================================================================
    // Effective value resolution
    // =========================================================================

    /// Gets the effective local value (Animation → local layer → registry default).
    ///
    /// Resolves the local layer by walking sources from highest to lowest
    /// precedence.
    ///
    /// # Panics
    ///
    /// Panics if the property is not registered in the registry.
    #[must_use]
    pub fn get_effective_local<T: Clone + 'static>(
        &self,
        property: Property<T>,
        registry: &PropertyRegistry,
    ) -> T {
        let id = property.id();
        if let Some(v) = self.animation_entries.entry_get(id)
            && let Some(v) = v.downcast_ref::<T>()
        {
            return v.clone();
        }
        if let Some((_, v)) = self.local_layer_lookup(id)
            && let Some(v) = v.downcast_ref::<T>()
        {
            return v.clone();
        }
        if let Some(metadata) = registry.get_metadata::<T>(property) {
            return metadata.default_value().clone();
        }
        panic!("Property {:?} not found in registry", property.id());
    }

    /// Gets the effective local value, borrowed.
    ///
    /// # Panics
    ///
    /// Panics if the property is not registered in the registry.
    #[must_use]
    #[inline]
    pub fn get_effective_local_ref<'a, T: Clone + 'static>(
        &'a self,
        property: Property<T>,
        registry: &'a PropertyRegistry,
    ) -> &'a T {
        let id = property.id();
        if let Some(v) = self.animation_entries.entry_get(id)
            && let Some(v) = v.downcast_ref::<T>()
        {
            return v;
        }
        if let Some((_, v)) = self.local_layer_lookup(id)
            && let Some(v) = v.downcast_ref::<T>()
        {
            return v;
        }
        if let Some(metadata) = registry.get_metadata::<T>(property) {
            return metadata.default_value();
        }
        panic!("Property {:?} not found in registry", property.id());
    }

    /// Returns `true` if any local-layer source or the animation slot has a
    /// value for `property`.
    #[must_use]
    #[inline]
    pub fn has_value<T: Clone + 'static>(&self, property: Property<T>) -> bool {
        self.has_local(property) || self.has_animation(property)
    }

    /// Clears every value (every local source and animation) for `property`.
    ///
    /// Returns `true` if any value was removed.
    pub fn clear_all<T: Clone + 'static>(&mut self, property: Property<T>) -> bool {
        let id = property.id();
        let mut removed = self.local_entries.entry_clear(id);
        removed |= self.template_binding_entries.entry_clear(id);
        removed |= self.template_default_entries.entry_clear(id);
        removed |= self.animation_entries.entry_clear(id);
        removed
    }

    /// Clears all animation values across all properties.
    ///
    /// Returns the number of animation values removed.
    pub fn clear_all_animations(&mut self) -> usize {
        let len = self.animation_entries.len();
        self.animation_entries.clear();
        len
    }
}

impl<K: Copy + Eq> Clone for PropertyStore<K> {
    fn clone(&self) -> Self {
        Self {
            local_entries: self.local_entries.clone(),
            template_binding_entries: self.template_binding_entries.clone(),
            template_default_entries: self.template_default_entries.clone(),
            animation_entries: self.animation_entries.clone(),
            owner: self.owner,
        }
    }
}

/// Merging iterator over four sorted [`Entry`] slices that yields each
/// `PropertyId` once, in ascending order.
struct PropertyIds<'a> {
    slices: [&'a [Entry]; 4],
    cursors: [usize; 4],
}

impl<'a> PropertyIds<'a> {
    fn new(slices: [&'a [Entry]; 4]) -> Self {
        Self {
            slices,
            cursors: [0; 4],
        }
    }
}

impl Iterator for PropertyIds<'_> {
    type Item = PropertyId;

    fn next(&mut self) -> Option<Self::Item> {
        let mut min: Option<PropertyId> = None;
        for slot in 0..4 {
            if let Some((id, _)) = self.slices[slot].get(self.cursors[slot]) {
                min = Some(match min {
                    Some(m) if m <= *id => m,
                    _ => *id,
                });
            }
        }
        let next = min?;
        // Advance every cursor that's currently at `next` (dedup across slots).
        for slot in 0..4 {
            if matches!(self.slices[slot].get(self.cursors[slot]), Some((id, _)) if *id == next) {
                self.cursors[slot] += 1;
            }
        }
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::PropertyMetadataBuilder;
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    use core::any::TypeId;
    use invalidation::Channel;

    fn setup_registry() -> (PropertyRegistry, Property<f64>, Property<i32>) {
        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
        let count = registry.register("Count", PropertyMetadataBuilder::new(0_i32).build());
        (registry, width, count)
    }

    #[test]
    fn store_new() {
        let store = PropertyStore::<u32>::new(1);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.owner(), 1);
    }

    #[test]
    fn store_set_get_local() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        assert!(store.get_local(width).is_none());

        store.set_local(width, 100.0);
        assert_eq!(store.get_local(width), Some(&100.0));
        assert!(!store.is_empty());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn store_set_get_animation() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        assert!(store.get_animation(width).is_none());

        store.set_animation(width, 200.0);
        assert_eq!(store.get_animation(width), Some(&200.0));
    }

    #[test]
    fn store_animation_precedence() {
        let (registry, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local(width, 100.0);
        assert_eq!(store.get_effective_local(width, &registry), 100.0);

        store.set_animation(width, 200.0);
        assert_eq!(store.get_effective_local(width, &registry), 200.0);

        store.clear_animation(width);
        assert_eq!(store.get_effective_local(width, &registry), 100.0);
    }

    #[test]
    fn store_effective_local_ref_precedence_and_sources() {
        let (registry, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        let default_ref = store.get_effective_local_ref(width, &registry);
        let metadata_default = registry.get_metadata(width).unwrap().default_value();
        assert!(core::ptr::eq(default_ref, metadata_default));

        store.set_local(width, 100.0);
        let local_ref = store.get_effective_local_ref(width, &registry);
        assert!(core::ptr::eq(local_ref, store.get_local(width).unwrap()));

        store.set_animation(width, 200.0);
        let anim_ref = store.get_effective_local_ref(width, &registry);
        assert!(core::ptr::eq(anim_ref, store.get_animation(width).unwrap()));
    }

    #[test]
    fn store_clear_local() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local(width, 100.0);
        assert!(store.has_local(width));

        assert!(store.clear_local(width));
        assert!(!store.has_local(width));
        assert!(store.is_empty());

        assert!(!store.clear_local(width));
    }

    #[test]
    fn store_clear_animation() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_animation(width, 200.0);
        assert!(store.has_animation(width));

        assert!(store.clear_animation(width));
        assert!(!store.has_animation(width));
        assert!(store.is_empty());
    }

    #[test]
    fn store_clear_all_animations() {
        let (_, width, count) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local(width, 100.0);
        store.set_animation(width, 200.0);
        store.set_animation(count, 5);

        let removed = store.clear_all_animations();
        assert_eq!(removed, 2);

        assert!(!store.has_animation(width));
        assert!(!store.has_animation(count));
        assert!(store.has_local(width));
        assert!(!store.has_value(count));
    }

    #[test]
    fn store_default_value() {
        let (registry, width, _) = setup_registry();
        let store = PropertyStore::<u32>::new(1);
        assert_eq!(store.get_effective_local(width, &registry), 0.0);
    }

    #[test]
    fn store_clone() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);
        store.set_local(width, 100.0);

        let cloned = store.clone();
        assert_eq!(cloned.get_local(width), Some(&100.0));
        assert_eq!(cloned.owner(), 1);
    }

    #[test]
    fn store_property_ids() {
        let (_, width, count) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local(width, 100.0);
        store.set_local(count, 5);

        let ids: Vec<_> = store.property_ids().collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&width.id()));
        assert!(ids.contains(&count.id()));
    }

    #[test]
    fn store_sorted_order() {
        let mut registry = PropertyRegistry::new();
        let c: Property<i32> = registry.register("C", PropertyMetadataBuilder::new(0).build());
        let a: Property<i32> = registry.register("A", PropertyMetadataBuilder::new(0).build());
        let b: Property<i32> = registry.register("B", PropertyMetadataBuilder::new(0).build());

        let mut store = PropertyStore::<u32>::new(1);

        store.set_local(b, 2);
        store.set_local(c, 3);
        store.set_local(a, 1);

        let ids: Vec<_> = store.property_ids().collect();
        assert_eq!(ids.len(), 3);

        for i in 1..ids.len() {
            assert!(ids[i - 1].index() < ids[i].index());
        }
    }

    #[test]
    fn store_binary_search_correctness() {
        let mut registry = PropertyRegistry::new();
        let props: Vec<Property<i32>> = (0..20)
            .map(|i| {
                registry.register(
                    Box::leak(alloc::format!("Prop{i}").into_boxed_str()),
                    PropertyMetadataBuilder::new(0).build(),
                )
            })
            .collect();

        let mut store = PropertyStore::<u32>::new(1);

        for (i, prop) in props.iter().enumerate() {
            if i % 2 == 0 {
                let value = i32::try_from(i).unwrap();
                store.set_local(*prop, value);
            }
        }

        for (i, prop) in props.iter().enumerate() {
            if i % 2 == 0 {
                let value = i32::try_from(i).unwrap();
                assert_eq!(store.get_local(*prop), Some(&value));
            } else {
                assert!(store.get_local(*prop).is_none());
            }
        }
    }

    #[test]
    fn store_local_and_animation_together() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local(width, 100.0);
        store.set_animation(width, 200.0);

        assert_eq!(store.get_local(width), Some(&100.0));
        assert_eq!(store.get_animation(width), Some(&200.0));

        store.clear_local(width);
        assert!(store.get_local(width).is_none());
        assert_eq!(store.get_animation(width), Some(&200.0));
        assert!(store.has_value(width));

        store.clear_animation(width);
        assert!(!store.has_value(width));
        assert!(store.is_empty());
    }

    // -------------------------------------------------------------------------
    // LocalValueSource — layered behavior
    // -------------------------------------------------------------------------

    #[test]
    fn local_value_source_precedence_order() {
        assert!(
            LocalValueSource::Local.precedence() > LocalValueSource::TemplateBinding.precedence()
        );
        assert!(
            LocalValueSource::TemplateBinding.precedence()
                > LocalValueSource::TemplateDefault.precedence()
        );
    }

    #[test]
    fn set_local_records_local_source() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local(width, 100.0);
        assert_eq!(store.get_local_source(width), Some(LocalValueSource::Local));
    }

    #[test]
    fn set_local_with_source_records_source() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local_with_source(width, 10.0, LocalValueSource::TemplateDefault);
        assert_eq!(store.get_local(width), Some(&10.0));
        assert_eq!(
            store.get_local_source(width),
            Some(LocalValueSource::TemplateDefault)
        );
    }

    #[test]
    fn highest_source_wins_read() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local_with_source(width, 10.0, LocalValueSource::TemplateDefault);
        store.set_local_with_source(width, 20.0, LocalValueSource::TemplateBinding);
        store.set_local(width, 30.0); // Local

        assert_eq!(store.get_local(width), Some(&30.0));
        assert_eq!(store.get_local_source(width), Some(LocalValueSource::Local));
    }

    #[test]
    fn clear_template_binding_reveals_template_default() {
        // Reviewer-requested: TemplateDefault → TemplateBinding → clear TemplateBinding
        // must reveal TemplateDefault.
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local_with_source(width, 10.0, LocalValueSource::TemplateDefault);
        store.set_local_with_source(width, 20.0, LocalValueSource::TemplateBinding);
        assert_eq!(store.get_local(width), Some(&20.0));

        assert!(store.clear_local_at_source(width, LocalValueSource::TemplateBinding));
        assert_eq!(store.get_local(width), Some(&10.0));
        assert_eq!(
            store.get_local_source(width),
            Some(LocalValueSource::TemplateDefault)
        );
    }

    #[test]
    fn clear_local_reveals_template_binding() {
        // Reviewer-requested: TemplateBinding → Local → clear Local
        // must reveal TemplateBinding.
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local_with_source(width, 20.0, LocalValueSource::TemplateBinding);
        store.set_local(width, 30.0);
        assert_eq!(store.get_local(width), Some(&30.0));

        assert!(store.clear_local(width));
        assert_eq!(store.get_local(width), Some(&20.0));
        assert_eq!(
            store.get_local_source(width),
            Some(LocalValueSource::TemplateBinding)
        );
    }

    #[test]
    fn clear_local_erased_reveals_template_binding() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local_with_source(width, 20.0, LocalValueSource::TemplateBinding);
        store.set_local(width, 30.0);

        assert!(store.clear_local_erased(width.id()));
        assert_eq!(store.get_local(width), Some(&20.0));
        assert_eq!(
            store.get_local_source(width),
            Some(LocalValueSource::TemplateBinding)
        );
        assert!(!store.clear_local_erased(width.id()));
    }

    #[test]
    fn clear_local_erased_at_source_targets_one_slot() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local_with_source(width, 10.0, LocalValueSource::TemplateDefault);
        store.set_local_with_source(width, 20.0, LocalValueSource::TemplateBinding);

        assert!(store.clear_local_erased_at_source(width.id(), LocalValueSource::TemplateBinding));
        assert_eq!(store.get_local(width), Some(&10.0));
        assert_eq!(
            store.get_local_source(width),
            Some(LocalValueSource::TemplateDefault)
        );
    }

    #[test]
    fn clear_local_erased_notifying_reports_visible_local_change() {
        const LAYOUT: Channel = Channel::new(0);

        let mut registry = PropertyRegistry::new();
        let width: Property<f64> = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .build(),
        );
        let mut store = PropertyStore::<u32>::new(1);
        store.set_local(width, 30.0);

        let channels = store.clear_local_erased_notifying(width.id(), &registry);

        assert!(channels.contains(LAYOUT));
        assert!(!store.has_local(width));
    }

    #[test]
    fn clear_local_erased_at_source_notifying_skips_masked_slots() {
        const LAYOUT: Channel = Channel::new(0);

        let mut registry = PropertyRegistry::new();
        let width: Property<f64> = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .build(),
        );
        let mut store = PropertyStore::<u32>::new(1);
        store.set_local(width, 30.0);
        store.set_local_with_source(width, 20.0, LocalValueSource::TemplateBinding);

        let channels = store.clear_local_erased_at_source_notifying(
            width.id(),
            LocalValueSource::TemplateBinding,
            &registry,
        );

        assert!(channels.is_empty());
        assert_eq!(store.get_local(width), Some(&30.0));
        assert!(!store.has_local_at_source(width, LocalValueSource::TemplateBinding));
    }

    #[test]
    fn clear_local_erased_notifying_skips_animation_masked_local() {
        const LAYOUT: Channel = Channel::new(0);

        let mut registry = PropertyRegistry::new();
        let width: Property<f64> = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .build(),
        );
        let mut store = PropertyStore::<u32>::new(1);
        store.set_local(width, 30.0);
        store.set_animation(width, 50.0);

        let channels = store.clear_local_erased_notifying(width.id(), &registry);

        assert!(channels.is_empty());
        assert!(!store.has_local_at_source(width, LocalValueSource::Local));
        assert_eq!(store.get_effective_local(width, &registry), 50.0);
    }

    #[test]
    fn set_to_lower_source_does_not_change_winning_value() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local(width, 100.0);
        // Writing a lower source goes to that source's slot but doesn't win.
        store.set_local_with_source(width, 50.0, LocalValueSource::TemplateBinding);

        assert_eq!(store.get_local(width), Some(&100.0));
        assert_eq!(store.get_local_source(width), Some(LocalValueSource::Local));
        // The lower-source value is preserved underneath.
        assert_eq!(
            store.get_local_at_source(width, LocalValueSource::TemplateBinding),
            Some(&50.0)
        );
    }

    #[test]
    fn clear_local_by_source_removes_only_that_slot() {
        let (_, width, count) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local_with_source(width, 1.0, LocalValueSource::TemplateDefault);
        store.set_local_with_source(count, 2_i32, LocalValueSource::TemplateDefault);
        store.set_local(width, 99.0);

        let removed = store.clear_local_by_source(LocalValueSource::TemplateDefault);
        assert_eq!(removed, 2);

        // `width` still has its Local value.
        assert_eq!(store.get_local(width), Some(&99.0));
        assert_eq!(store.get_local_source(width), Some(LocalValueSource::Local));
        // `count` had only TemplateDefault; nothing remains.
        assert!(!store.has_local(count));
    }

    #[test]
    fn clear_local_by_source_returns_zero_when_empty() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);
        store.set_local(width, 100.0);

        assert_eq!(
            store.clear_local_by_source(LocalValueSource::TemplateDefault),
            0
        );
        assert!(store.has_local(width));
    }

    #[test]
    fn has_local_at_source_reflects_specific_slot() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local_with_source(width, 7.0, LocalValueSource::TemplateBinding);

        assert!(store.has_local_at_source(width, LocalValueSource::TemplateBinding));
        assert!(!store.has_local_at_source(width, LocalValueSource::Local));
        assert!(!store.has_local_at_source(width, LocalValueSource::TemplateDefault));
    }

    #[test]
    fn property_ids_dedupes_across_slots() {
        let (_, width, count) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        // Same property has entries in multiple slots; should only appear once.
        store.set_local_with_source(width, 1.0, LocalValueSource::TemplateDefault);
        store.set_local_with_source(width, 2.0, LocalValueSource::TemplateBinding);
        store.set_local(width, 3.0);
        store.set_animation(width, 4.0);
        // `count` only in one slot.
        store.set_local(count, 99);

        let ids: Vec<_> = store.property_ids().collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&width.id()));
        assert!(ids.contains(&count.id()));
    }

    #[test]
    fn local_winner_erased_returns_highest_source_entry() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local_with_source(width, 10.0, LocalValueSource::TemplateDefault);
        store.set_local_with_source(width, 20.0, LocalValueSource::TemplateBinding);

        let erased = store.local_winner_erased(width.id()).unwrap();
        assert_eq!(erased.downcast_ref::<f64>(), Some(&20.0));
        assert_eq!(store.get_local(width), Some(&20.0));

        store.set_local(width, 30.0);
        let erased = store.local_winner_erased(width.id()).unwrap();
        assert_eq!(erased.downcast_ref::<f64>(), Some(&30.0));
        assert_eq!(store.get_local(width), Some(&30.0));
    }

    #[test]
    fn local_winner_erased_returns_none_without_local_entries() {
        let (_, width, _) = setup_registry();
        let store = PropertyStore::<u32>::new(1);

        assert!(store.local_winner_erased(width.id()).is_none());
    }

    #[test]
    fn effective_local_erased_prefers_animation_over_local_winner() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local(width, 100.0);
        store.set_animation(width, 200.0);

        let erased = store.effective_local_erased(width.id()).unwrap();
        assert_eq!(erased.downcast_ref::<f64>(), Some(&200.0));
    }

    #[test]
    fn animation_erased_returns_animation_slot_entry() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        assert!(store.animation_erased(width.id()).is_none());

        store.set_local(width, 100.0);
        store.set_animation(width, 200.0);

        let erased = store.animation_erased(width.id()).unwrap();
        assert_eq!(erased.downcast_ref::<f64>(), Some(&200.0));
    }

    #[test]
    fn local_erased_at_source_reads_specific_slot() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local_with_source(width, 10.0, LocalValueSource::TemplateDefault);
        store.set_local(width, 30.0);

        let erased = store
            .local_erased_at_source(width.id(), LocalValueSource::TemplateDefault)
            .unwrap();
        assert_eq!(erased.downcast_ref::<f64>(), Some(&10.0));
        let erased = store
            .local_erased_at_source(width.id(), LocalValueSource::Local)
            .unwrap();
        assert_eq!(erased.downcast_ref::<f64>(), Some(&30.0));
    }

    #[test]
    fn set_local_erased_with_source_checked_rejects_mismatch_and_leaves_store() {
        let (registry, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);
        store.set_local_with_source(width, 10.0, LocalValueSource::TemplateDefault);

        let err = store
            .set_local_erased_with_source_checked(
                width.id(),
                ErasedValue::new(42_i32),
                LocalValueSource::TemplateDefault,
                &registry,
            )
            .unwrap_err();

        assert_eq!(err.expected, TypeId::of::<f64>());
        assert_eq!(err.actual, TypeId::of::<i32>());
        assert_eq!(store.get_local(width), Some(&10.0));
    }

    #[test]
    fn set_local_erased_with_source_round_trips_through_typed_read() {
        let (_, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);

        store.set_local_erased_with_source(
            width.id(),
            ErasedValue::new(42.0_f64),
            LocalValueSource::TemplateBinding,
        );

        assert_eq!(store.get_local(width), Some(&42.0));
        assert_eq!(
            store.get_local_source(width),
            Some(LocalValueSource::TemplateBinding)
        );
    }

    #[test]
    fn set_local_erased_with_source_notifying_returns_channels_even_for_same_value() {
        const LAYOUT: Channel = Channel::new(0);

        let mut registry = PropertyRegistry::new();
        let width: Property<f64> = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .build(),
        );
        let mut store = PropertyStore::<u32>::new(1);

        let first = store
            .set_local_erased_with_source_notifying(
                width.id(),
                ErasedValue::new(10.0_f64),
                LocalValueSource::Local,
                &registry,
            )
            .unwrap();
        let second = store
            .set_local_erased_with_source_notifying(
                width.id(),
                ErasedValue::new(10.0_f64),
                LocalValueSource::Local,
                &registry,
            )
            .unwrap();

        assert!(first.contains(LAYOUT));
        assert!(second.contains(LAYOUT));
        assert_eq!(store.get_local(width), Some(&10.0));
    }

    #[test]
    fn set_local_erased_with_source_notifying_mismatch_leaves_store_untouched() {
        let (registry, width, _) = setup_registry();
        let mut store = PropertyStore::<u32>::new(1);
        store.set_local_with_source(width, 10.0, LocalValueSource::TemplateDefault);

        let err = store
            .set_local_erased_with_source_notifying(
                width.id(),
                ErasedValue::new(42_i32),
                LocalValueSource::Local,
                &registry,
            )
            .unwrap_err();

        assert_eq!(err.expected, TypeId::of::<f64>());
        assert_eq!(err.actual, TypeId::of::<i32>());
        assert!(!store.has_local_at_source(width, LocalValueSource::Local));
        assert_eq!(
            store.get_local_at_source(width, LocalValueSource::TemplateDefault),
            Some(&10.0)
        );
        assert_eq!(store.get_local(width), Some(&10.0));
    }
}
