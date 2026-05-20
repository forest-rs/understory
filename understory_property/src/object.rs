// Copyright 2025 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Dependency object traits.
//!
//! This module provides the [`DependencyObject`] trait for objects that can
//! have dependency properties, and [`DependencyObjectExt`] for convenient
//! property access methods including inheritance resolution.

use invalidation::ChannelSet;

use crate::id::Property;
use crate::registry::PropertyRegistry;
use crate::store::{LocalValueSource, PropertyStore};

/// A lookup mechanism for walking parent chains for inheritance.
///
/// Given an object key, returns its [`PropertyStore`] and its parent key.
///
/// This is used by inheritance walking helpers such as [`walk_inherited`] and
/// [`walk_inherited_ref`], and by higher-level resolution layers.
pub trait ParentLookup<'a, K: Copy + Eq + 'a> {
    /// Looks up the store and parent key for `key`.
    fn lookup(&self, key: K) -> Option<(&'a PropertyStore<K>, Option<K>)>;
}

impl<'a, K, F> ParentLookup<'a, K> for F
where
    K: Copy + Eq + 'a,
    F: Fn(K) -> Option<(&'a PropertyStore<K>, Option<K>)>,
{
    #[inline]
    fn lookup(&self, key: K) -> Option<(&'a PropertyStore<K>, Option<K>)> {
        self(key)
    }
}

/// Walks the parent chain looking for an inherited value.
///
/// This is the canonical implementation of inheritance walking, checking
/// Animation → Local at each ancestor. Used by [`DependencyObjectExt::get_inherited`]
/// and can be reused by higher-level crates (e.g., `understory_style`).
///
/// Returns the first value found, or `None` if no ancestor has the property set.
///
/// # Arguments
///
/// * `start_key` - The key to start walking from (typically `parent_key()`)
/// * `property` - The property to look for
/// * `store_lookup` - Function returning (store, `parent_key`) for a given key
pub fn walk_inherited<'a, K, T, F>(
    mut current_key: Option<K>,
    property: Property<T>,
    store_lookup: &F,
) -> Option<T>
where
    K: Copy + Eq + 'a,
    T: Clone + 'static,
    F: ParentLookup<'a, K> + ?Sized,
{
    while let Some(key) = current_key {
        if let Some((parent_store, next_parent)) = store_lookup.lookup(key) {
            // Check parent's values (Animation > Local)
            if let Some(value) = parent_store.get_animation(property) {
                return Some(value.clone());
            }
            if let Some(value) = parent_store.get_local(property) {
                return Some(value.clone());
            }
            current_key = next_parent;
        } else {
            break;
        }
    }
    None
}

/// Walks the parent chain looking for an inherited value, returning a reference.
///
/// This is the borrowed variant of [`walk_inherited`]. It checks Animation → Local at each
/// ancestor and returns the first value found.
///
/// Returns `None` if no ancestor has the property set.
///
/// # Arguments
///
/// * `start_key` - The key to start walking from (typically `parent_key()`)
/// * `property` - The property to look for
/// * `store_lookup` - Function returning (store, `parent_key`) for a given key
pub fn walk_inherited_ref<'a, K, T, F>(
    mut current_key: Option<K>,
    property: Property<T>,
    store_lookup: &F,
) -> Option<&'a T>
where
    K: Copy + Eq + 'a,
    T: Clone + 'static,
    F: ParentLookup<'a, K> + ?Sized,
{
    while let Some(key) = current_key {
        if let Some((parent_store, next_parent)) = store_lookup.lookup(key) {
            // Check parent's values (Animation > Local)
            if let Some(value) = parent_store.get_animation(property) {
                return Some(value);
            }
            if let Some(value) = parent_store.get_local(property) {
                return Some(value);
            }
            current_key = next_parent;
        } else {
            break;
        }
    }
    None
}

/// A trait for objects that can have dependency properties.
///
/// This trait provides access to the object's property store and key,
/// enabling the extension methods in [`DependencyObjectExt`].
///
/// # Example
///
/// ```rust
/// use understory_property::{DependencyObject, PropertyStore};
///
/// struct MyElement {
///     key: u32,
///     parent: Option<u32>,
///     store: PropertyStore<u32>,
/// }
///
/// impl DependencyObject<u32> for MyElement {
///     fn property_store(&self) -> &PropertyStore<u32> {
///         &self.store
///     }
///
///     fn property_store_mut(&mut self) -> &mut PropertyStore<u32> {
///         &mut self.store
///     }
///
///     fn key(&self) -> u32 {
///         self.key
///     }
///
///     fn parent_key(&self) -> Option<u32> {
///         self.parent
///     }
/// }
/// ```
pub trait DependencyObject<K: Copy + Eq> {
    /// Returns a reference to the object's property store.
    fn property_store(&self) -> &PropertyStore<K>;

    /// Returns a mutable reference to the object's property store.
    fn property_store_mut(&mut self) -> &mut PropertyStore<K>;

    /// Returns the key that identifies this object.
    fn key(&self) -> K;

    /// Returns the parent's key, if this object has a parent.
    ///
    /// This is used for property inheritance resolution.
    fn parent_key(&self) -> Option<K>;
}

/// Extension methods for [`DependencyObject`].
///
/// These methods provide convenient access to property values.
pub trait DependencyObjectExt<K: Copy + Eq>: DependencyObject<K> {
    /// Gets the local value only.
    ///
    /// Returns `None` if no local value is set.
    fn get_local_value<'a, T: Clone + 'static>(&'a self, property: Property<T>) -> Option<&'a T>
    where
        K: 'a,
    {
        self.property_store().get_local(property)
    }

    /// Gets the animation value only.
    ///
    /// Returns `None` if no animation value is set.
    fn get_animation_value<'a, T: Clone + 'static>(&'a self, property: Property<T>) -> Option<&'a T>
    where
        K: 'a,
    {
        self.property_store().get_animation(property)
    }

    /// Gets the effective local value (Animation → Local → default).
    ///
    /// Does **not** handle style or inheritance—those belong to higher-level APIs.
    fn get_effective_local<T: Clone + 'static>(
        &self,
        property: Property<T>,
        registry: &PropertyRegistry,
    ) -> T {
        self.property_store()
            .get_effective_local(property, registry)
    }

    /// Gets the effective local value (Animation → Local → default), borrowed.
    ///
    /// Does **not** handle style or inheritance—those belong to higher-level APIs.
    ///
    /// # Panics
    ///
    /// Panics if the property is not registered in the registry.
    fn get_effective_local_ref<'a, T: Clone + 'static>(
        &'a self,
        property: Property<T>,
        registry: &'a PropertyRegistry,
    ) -> &'a T
    where
        K: 'a,
    {
        self.property_store()
            .get_effective_local_ref(property, registry)
    }

    /// Gets the effective value with inheritance resolution.
    ///
    /// Resolution order:
    /// 1. This object's Animation value
    /// 2. This object's Local value
    /// 3. If property inherits: walk parent chain (Animation → Local at each level)
    /// 4. Registry default
    ///
    /// # Arguments
    ///
    /// * `property` - The property to get
    /// * `registry` - The property registry containing metadata
    /// * `store_lookup` - Returns (`PropertyStore`, `parent_key`) for a given key
    ///
    /// # Example
    ///
    /// ```rust
    /// use understory_property::{
    ///     DependencyObject, DependencyObjectExt, PropertyStore,
    ///     PropertyMetadataBuilder, PropertyRegistry,
    /// };
    /// use std::collections::HashMap;
    ///
    /// let mut registry = PropertyRegistry::new();
    /// let font_size = registry.register(
    ///     "FontSize",
    ///     PropertyMetadataBuilder::new(12.0_f64)
    ///         .inherits(true)
    ///         .build()
    /// );
    ///
    /// struct Element { key: u32, parent: Option<u32>, store: PropertyStore<u32> }
    /// impl DependencyObject<u32> for Element {
    ///     fn property_store(&self) -> &PropertyStore<u32> { &self.store }
    ///     fn property_store_mut(&mut self) -> &mut PropertyStore<u32> { &mut self.store }
    ///     fn key(&self) -> u32 { self.key }
    ///     fn parent_key(&self) -> Option<u32> { self.parent }
    /// }
    ///
    /// let mut parent = Element { key: 1, parent: None, store: PropertyStore::new(1) };
    /// let child = Element { key: 2, parent: Some(1), store: PropertyStore::new(2) };
    ///
    /// parent.store.set_local(font_size, 16.0);
    ///
    /// let elements: HashMap<u32, &Element> = [(1, &parent), (2, &child)].into_iter().collect();
    ///
    /// let value = child.get_inherited(font_size, &registry, &|key| {
    ///     elements.get(&key).map(|e| (e.property_store(), e.parent_key()))
    /// });
    /// assert_eq!(value, 16.0);
    /// ```
    fn get_inherited<'a, T, F>(
        &'a self,
        property: Property<T>,
        registry: &PropertyRegistry,
        store_lookup: &F,
    ) -> T
    where
        K: 'a,
        T: Clone + 'static,
        F: ParentLookup<'a, K> + ?Sized,
    {
        // Check our values first (Animation > Local)
        if let Some(value) = self.property_store().get_animation(property) {
            return value.clone();
        }
        if let Some(value) = self.property_store().get_local(property) {
            return value.clone();
        }

        // Check inheritance chain if property inherits
        if let Some(metadata) = registry.get_metadata::<T>(property) {
            if metadata.inherits()
                && let Some(value) = walk_inherited(self.parent_key(), property, store_lookup)
            {
                return value;
            }
            return metadata.default_value().clone();
        }

        panic!("Property {:?} not found in registry", property.id());
    }

    /// Gets the effective value with inheritance resolution (borrowed).
    ///
    /// Resolution order:
    /// 1. This object's Animation value
    /// 2. This object's Local value
    /// 3. If property inherits: walk parent chain (Animation → Local at each level)
    /// 4. Registry default
    ///
    /// # Panics
    ///
    /// Panics if the property is not registered in the registry.
    fn get_inherited_ref<'a, T, F>(
        &'a self,
        property: Property<T>,
        registry: &'a PropertyRegistry,
        store_lookup: &F,
    ) -> &'a T
    where
        K: 'a,
        T: Clone + 'static,
        F: ParentLookup<'a, K> + ?Sized,
    {
        // Check our values first (Animation > Local)
        if let Some(value) = self.property_store().get_animation(property) {
            return value;
        }
        if let Some(value) = self.property_store().get_local(property) {
            return value;
        }

        // Check inheritance chain if property inherits
        if let Some(metadata) = registry.get_metadata::<T>(property) {
            if metadata.inherits()
                && let Some(value) = walk_inherited_ref(self.parent_key(), property, store_lookup)
            {
                return value;
            }
            return metadata.default_value();
        }

        panic!("Property {:?} not found in registry", property.id());
    }

    /// Sets the local value.
    fn set_local<T: Clone + 'static>(&mut self, property: Property<T>, value: T) {
        self.property_store_mut().set_local(property, value);
    }

    /// Sets the animation value.
    fn set_animation<T: Clone + 'static>(&mut self, property: Property<T>, value: T) {
        self.property_store_mut().set_animation(property, value);
    }

    /// Sets the local value with coercion and callbacks.
    ///
    /// This is the "blessed" API for setting property values. It:
    /// 1. Coerces the value using the property's coerce callback (if any)
    /// 2. Stores the value at the local layer
    /// 3. Computes the new effective local value (`Animation -> Local -> default`)
    /// 4. Calls the property's changed callback if that effective local value changed
    /// 5. Returns affected channels only when the effective local value changed
    ///
    /// This helper does **not** consult inheritance or style layers; it only
    /// reasons about the same effective-local resolution used by
    /// [`DependencyObjectExt::get_effective_local`].
    ///
    /// `T` must implement [`PartialEq`] so no-op effective changes can be
    /// suppressed reliably.
    ///
    /// The caller is responsible for routing dirty channels into its
    /// invalidation coordinator. Use `mark_with` when property changes should
    /// follow graph dependencies, channel cascades, or cross-channel edges; use
    /// `mark` only for deliberately local channels.
    ///
    /// ```ignore
    /// let channels = element.set_local_notifying(width, 100.0, &registry);
    /// for channel in channels {
    ///     tracker.mark_with(element.key(), channel, &LazyPolicy);
    /// }
    /// ```
    ///
    /// # Example
    ///
    /// ```rust
    /// use understory_property::{
    ///     DependencyObject, DependencyObjectExt, PropertyStore,
    ///     PropertyMetadataBuilder, PropertyRegistry,
    /// };
    /// use invalidation::Channel;
    ///
    /// const LAYOUT: Channel = Channel::new(0);
    ///
    /// let mut registry = PropertyRegistry::new();
    /// let width = registry.register(
    ///     "Width",
    ///     PropertyMetadataBuilder::new(0.0_f64)
    ///         .affects_channels(LAYOUT.into_set())
    ///         .coerce(|v| v.max(0.0))
    ///         .build()
    /// );
    ///
    /// struct Element { key: u32, parent: Option<u32>, store: PropertyStore<u32> }
    /// impl DependencyObject<u32> for Element {
    ///     fn property_store(&self) -> &PropertyStore<u32> { &self.store }
    ///     fn property_store_mut(&mut self) -> &mut PropertyStore<u32> { &mut self.store }
    ///     fn key(&self) -> u32 { self.key }
    ///     fn parent_key(&self) -> Option<u32> { self.parent }
    /// }
    ///
    /// let mut element = Element { key: 1, parent: None, store: PropertyStore::new(1) };
    ///
    /// // Set value - stores it and returns affected channels because the
    /// // effective local value changed from the default.
    /// let channels = element.set_local_notifying(width, 10.0, &registry);
    ///
    /// assert_eq!(element.property_store().get_local(width), Some(&10.0));
    ///
    /// // Caller marks dirty
    /// assert!(channels.contains(LAYOUT));
    /// ```
    fn set_local_notifying<T: Clone + PartialEq + 'static>(
        &mut self,
        property: Property<T>,
        value: T,
        registry: &PropertyRegistry,
    ) -> ChannelSet {
        let metadata = registry.get_metadata(property);
        let old_effective = metadata.map(|_| {
            self.property_store()
                .get_effective_local(property, registry)
        });

        // 1. Coerce the value
        let value = match metadata {
            Some(m) => m.coerce(value),
            None => value,
        };

        // 2. Store the value
        self.property_store_mut().set_local(property, value);

        // 3. Notify only when the effective local value changed.
        if let Some((m, old_effective)) = metadata.zip(old_effective) {
            let new_effective = self
                .property_store()
                .get_effective_local(property, registry);
            if old_effective != new_effective {
                m.on_changed(Some(&old_effective), &new_effective);
                return m.affects_channels();
            }
        }

        ChannelSet::empty()
    }

    /// Writes a value into the slot for `source`.
    ///
    /// Each source has its own sparse slot, so the write never overwrites a
    /// value held by another source. The newly-written value becomes the
    /// winning local value only if `source` has the highest precedence among
    /// the sources that currently have an entry for `property`. See
    /// [`LocalValueSource`].
    fn set_local_with_source<T: Clone + 'static>(
        &mut self,
        property: Property<T>,
        value: T,
        source: LocalValueSource,
    ) {
        self.property_store_mut()
            .set_local_with_source(property, value, source);
    }

    /// Sets the source-tagged value with coercion and notification.
    ///
    /// Mirrors [`DependencyObjectExt::set_local_notifying`]: coerces the value,
    /// writes it to `source`'s slot, then fires the changed callback and
    /// returns affected channels only if the effective local value actually
    /// changed. A write that lands in a slot shadowed by a higher-precedence
    /// source returns an empty `ChannelSet` (the effective value didn't move).
    fn set_local_with_source_notifying<T: Clone + PartialEq + 'static>(
        &mut self,
        property: Property<T>,
        value: T,
        source: LocalValueSource,
        registry: &PropertyRegistry,
    ) -> ChannelSet {
        let metadata = registry.get_metadata(property);
        let old_effective = metadata.map(|_| {
            self.property_store()
                .get_effective_local(property, registry)
        });

        let value = match metadata {
            Some(m) => m.coerce(value),
            None => value,
        };

        self.property_store_mut()
            .set_local_with_source(property, value, source);

        if let Some((m, old_effective)) = metadata.zip(old_effective) {
            let new_effective = self
                .property_store()
                .get_effective_local(property, registry);
            if old_effective != new_effective {
                m.on_changed(Some(&old_effective), &new_effective);
                return m.affects_channels();
            }
        }

        ChannelSet::empty()
    }

    /// Returns the [`LocalValueSource`] currently winning the local-layer
    /// resolution for `property`, if any source has a value.
    fn get_local_source<T: Clone + 'static>(
        &self,
        property: Property<T>,
    ) -> Option<LocalValueSource> {
        self.property_store().get_local_source(property)
    }

    /// Clears every value stored under `source`.
    ///
    /// Returns the number of entries removed. See
    /// [`PropertyStore::clear_local_by_source`] and
    /// [`Self::clear_local_by_source_notifying`] for the channel-returning
    /// variant.
    fn clear_local_by_source(&mut self, source: LocalValueSource) -> usize {
        self.property_store_mut().clear_local_by_source(source)
    }

    /// Bulk-clears every value stored under `source` and returns a conservative
    /// union of channels that *may* have changed.
    ///
    /// A property contributes its `affects_channels` to the result whenever
    /// (a) `source` is the current winning local-layer source for it, and
    /// (b) no animation value is masking it. This is the correct conservative
    /// answer to "which application channels did this teardown potentially
    /// invalidate?" — note that if the value at the next-lower source happens
    /// to equal the cleared value (e.g. `TemplateBinding(10)` shadowing
    /// `TemplateDefault(10)`), the effective value didn't actually move but
    /// this method still returns its channel. That's fine for invalidation
    /// (extra dirty-marks are safe) but means callers should not treat the
    /// return as a precise value-change signal.
    ///
    /// Per-property `on_changed` callbacks are **not** invoked by this method:
    /// the bulk-clear path doesn't have typed values for the post-clear
    /// effective on each property. If you need callbacks, use the per-property
    /// [`clear_local_at_source_notifying`](Self::clear_local_at_source_notifying)
    /// method instead.
    fn clear_local_by_source_notifying(
        &mut self,
        source: LocalValueSource,
        registry: &PropertyRegistry,
    ) -> ChannelSet {
        let mut affected = ChannelSet::empty();
        let store = self.property_store();
        // Collect ids potentially affected: those where `source` is the current
        // winner and no animation is masking the layer. If a lower source
        // shadowed by `source` happens to hold an equal value, this still
        // contributes channels — bulk-clear doesn't compare typed values.
        let ids_affected: alloc::vec::Vec<crate::id::PropertyId> = store
            .local_property_ids_at_source(source)
            .filter(|id| {
                // Animation masks the local layer entirely.
                if store.animation_entries_has(*id) {
                    return false;
                }
                // `source` must currently be the winner; clearing a shadowed
                // slot can't move the effective value.
                store.winning_local_source_for_id(*id) == Some(source)
            })
            .collect();

        for id in ids_affected {
            affected |= registry.affects_channels(id);
        }

        self.property_store_mut().clear_local_by_source(source);

        affected
    }

    /// Clears the value in the [`LocalValueSource::Local`] slot.
    ///
    /// Lower-precedence slots (`TemplateBinding`, `TemplateDefault`) are
    /// untouched and may become the new winning value. Use
    /// [`clear_local_at_source`](Self::clear_local_at_source) to target a
    /// different slot, or [`clear_all`](Self::clear_all) to drop every value
    /// (every local source plus animation) for one property.
    ///
    /// Returns `true` if a value was removed.
    fn clear_local<T: Clone + 'static>(&mut self, property: Property<T>) -> bool {
        self.property_store_mut().clear_local(property)
    }

    /// Clears the value at a specific local-layer source for `property`.
    ///
    /// Returns `true` if a value was removed.
    fn clear_local_at_source<T: Clone + 'static>(
        &mut self,
        property: Property<T>,
        source: LocalValueSource,
    ) -> bool {
        self.property_store_mut()
            .clear_local_at_source(property, source)
    }

    /// Clears every value (every local source and animation) for `property`.
    ///
    /// Returns `true` if any value was removed.
    fn clear_all<T: Clone + 'static>(&mut self, property: Property<T>) -> bool {
        self.property_store_mut().clear_all(property)
    }

    /// Clears the [`LocalValueSource::Local`] slot and notifies if the
    /// effective value changed.
    ///
    /// Only the `Local` slot is touched — to clear a different source slot
    /// with notification, use
    /// [`clear_local_at_source_notifying`](Self::clear_local_at_source_notifying).
    ///
    /// Compares the effective local value (`Animation -> winning local layer ->
    /// default`) before and after the clear and only returns dirty channels +
    /// fires the `on_changed` callback when that observable value moved.
    fn clear_local_notifying<T: Clone + PartialEq + 'static>(
        &mut self,
        property: Property<T>,
        registry: &PropertyRegistry,
    ) -> ChannelSet {
        if !self
            .property_store()
            .has_local_at_source(property, LocalValueSource::Local)
        {
            return ChannelSet::empty();
        }

        let metadata = registry.get_metadata(property);
        let old_effective = metadata.map(|_| {
            self.property_store()
                .get_effective_local(property, registry)
        });

        self.property_store_mut().clear_local(property);

        if let Some((m, old_effective)) = metadata.zip(old_effective) {
            let new_effective = self
                .property_store()
                .get_effective_local(property, registry);
            if old_effective != new_effective {
                m.on_changed(Some(&old_effective), &new_effective);
                return m.affects_channels();
            }
        }

        ChannelSet::empty()
    }

    /// Clears the value at `source` for `property` and notifies if the
    /// effective value changed.
    ///
    /// Mirrors [`clear_local_notifying`](Self::clear_local_notifying) but
    /// targets an arbitrary local-layer source. Compares effective values
    /// (`Animation -> winning local layer -> default`) before and after the
    /// clear and only returns dirty channels + fires the `on_changed` callback
    /// when that observable value moved.
    ///
    /// Clearing a slot that isn't currently winning (e.g. clearing
    /// `TemplateBinding` while `Local` shadows it) is correctly reported as
    /// a no-op: empty channels, no callback. Clearing a slot whose value
    /// happens to equal the next-lower source's value is also a no-op
    /// (effective didn't actually move).
    fn clear_local_at_source_notifying<T: Clone + PartialEq + 'static>(
        &mut self,
        property: Property<T>,
        source: LocalValueSource,
        registry: &PropertyRegistry,
    ) -> ChannelSet {
        if !self.property_store().has_local_at_source(property, source) {
            return ChannelSet::empty();
        }

        let metadata = registry.get_metadata(property);
        let old_effective = metadata.map(|_| {
            self.property_store()
                .get_effective_local(property, registry)
        });

        self.property_store_mut()
            .clear_local_at_source(property, source);

        if let Some((m, old_effective)) = metadata.zip(old_effective) {
            let new_effective = self
                .property_store()
                .get_effective_local(property, registry);
            if old_effective != new_effective {
                m.on_changed(Some(&old_effective), &new_effective);
                return m.affects_channels();
            }
        }

        ChannelSet::empty()
    }

    /// Clears the animation value.
    ///
    /// Returns `true` if a value was removed.
    fn clear_animation<T: Clone + 'static>(&mut self, property: Property<T>) -> bool {
        self.property_store_mut().clear_animation(property)
    }

    /// Clears the animation value and notifies if the effective local value changed.
    ///
    /// This compares the effective local value (`Animation -> Local -> default`)
    /// before and after the clear and only returns dirty channels when that
    /// observable value changed.
    fn clear_animation_notifying<T: Clone + PartialEq + 'static>(
        &mut self,
        property: Property<T>,
        registry: &PropertyRegistry,
    ) -> ChannelSet {
        if !self.property_store().has_animation(property) {
            return ChannelSet::empty();
        }

        let metadata = registry.get_metadata(property);
        let old_effective = metadata.map(|_| {
            self.property_store()
                .get_effective_local(property, registry)
        });

        self.property_store_mut().clear_animation(property);

        if let Some((m, old_effective)) = metadata.zip(old_effective) {
            let new_effective = self
                .property_store()
                .get_effective_local(property, registry);
            if old_effective != new_effective {
                m.on_changed(Some(&old_effective), &new_effective);
                return m.affects_channels();
            }
        }

        ChannelSet::empty()
    }

    /// Returns `true` if the property has any value (local or animation).
    fn has_value<T: Clone + 'static>(&self, property: Property<T>) -> bool {
        self.property_store().has_value(property)
    }

    /// Returns `true` if the property has a local value.
    fn has_local<T: Clone + 'static>(&self, property: Property<T>) -> bool {
        self.property_store().has_local(property)
    }

    /// Returns `true` if the property has an animation value.
    fn has_animation<T: Clone + 'static>(&self, property: Property<T>) -> bool {
        self.property_store().has_animation(property)
    }
}

// Blanket implementation for all DependencyObject types
impl<K: Copy + Eq, T: DependencyObject<K>> DependencyObjectExt<K> for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PropertyRegistry;
    use crate::metadata::PropertyMetadataBuilder;

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

    fn setup_registry() -> (PropertyRegistry, Property<f64>) {
        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
        (registry, width)
    }

    #[test]
    fn ext_get_set_local() {
        let (_, width) = setup_registry();
        let mut element = TestElement::new(1, None);

        assert!(element.get_local_value(width).is_none());

        element.set_local(width, 100.0);
        assert_eq!(element.get_local_value(width), Some(&100.0));
    }

    #[test]
    fn ext_get_set_animation() {
        let (_, width) = setup_registry();
        let mut element = TestElement::new(1, None);

        assert!(element.get_animation_value(width).is_none());

        element.set_animation(width, 200.0);
        assert_eq!(element.get_animation_value(width), Some(&200.0));
    }

    #[test]
    fn ext_animation_precedence() {
        let (registry, width) = setup_registry();
        let mut element = TestElement::new(1, None);

        element.set_local(width, 100.0);
        element.set_animation(width, 200.0);

        // Animation wins
        assert_eq!(element.get_effective_local(width, &registry), 200.0);

        // Clear animation
        element.clear_animation(width);
        assert_eq!(element.get_effective_local(width, &registry), 100.0);
    }

    #[test]
    fn ext_effective_local_ref_borrows_from_store() {
        let (registry, width) = setup_registry();
        let mut element = TestElement::new(1, None);

        element.set_local(width, 100.0);
        element.set_animation(width, 200.0);

        let value_ref = element.get_effective_local_ref(width, &registry);
        assert!(core::ptr::eq(
            value_ref,
            element.property_store().get_animation(width).unwrap()
        ));
    }

    #[test]
    fn ext_clear_local() {
        let (_, width) = setup_registry();
        let mut element = TestElement::new(1, None);

        element.set_local(width, 100.0);
        assert!(element.has_local(width));

        assert!(element.clear_local(width));
        assert!(!element.has_local(width));
    }

    #[test]
    fn dependency_object_key() {
        let element = TestElement::new(42, Some(1));
        assert_eq!(element.key(), 42);
        assert_eq!(element.parent_key(), Some(1));
    }

    #[test]
    fn ext_inheritance_from_parent() {
        let mut registry = PropertyRegistry::new();
        let font_size = registry.register(
            "FontSize",
            PropertyMetadataBuilder::new(12.0_f64)
                .inherits(true)
                .build(),
        );

        let mut parent = TestElement::new(1, None);
        let child = TestElement::new(2, Some(1));

        parent.set_local(font_size, 16.0);

        let elements: alloc::collections::BTreeMap<u32, &TestElement> =
            [(1, &parent), (2, &child)].into_iter().collect();

        let value = child.get_inherited(font_size, &registry, &|key| {
            elements
                .get(&key)
                .map(|e| (e.property_store(), e.parent_key()))
        });
        assert_eq!(value, 16.0);
    }

    #[test]
    fn ext_inheritance_from_parent_ref() {
        let mut registry = PropertyRegistry::new();
        let font_size = registry.register(
            "FontSize",
            PropertyMetadataBuilder::new(12.0_f64)
                .inherits(true)
                .build(),
        );

        let mut parent = TestElement::new(1, None);
        let child = TestElement::new(2, Some(1));

        parent.set_local(font_size, 16.0);

        let elements: alloc::collections::BTreeMap<u32, &TestElement> =
            [(1, &parent), (2, &child)].into_iter().collect();

        let value_ref = child.get_inherited_ref(font_size, &registry, &|key| {
            elements
                .get(&key)
                .map(|e| (e.property_store(), e.parent_key()))
        });

        assert!(core::ptr::eq(
            value_ref,
            parent.property_store().get_local(font_size).unwrap()
        ));
        assert_eq!(*value_ref, 16.0);
    }

    #[test]
    fn ext_local_overrides_inherited() {
        let mut registry = PropertyRegistry::new();
        let font_size = registry.register(
            "FontSize",
            PropertyMetadataBuilder::new(12.0_f64)
                .inherits(true)
                .build(),
        );

        let mut parent = TestElement::new(1, None);
        let mut child = TestElement::new(2, Some(1));

        parent.set_local(font_size, 16.0);
        child.set_local(font_size, 20.0);

        let elements: alloc::collections::BTreeMap<u32, &TestElement> =
            [(1, &parent), (2, &child)].into_iter().collect();

        let value = child.get_inherited(font_size, &registry, &|key| {
            elements
                .get(&key)
                .map(|e| (e.property_store(), e.parent_key()))
        });
        assert_eq!(value, 20.0);
    }

    #[test]
    fn ext_non_inherited_uses_default() {
        let mut registry = PropertyRegistry::new();
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(100.0_f64)
                .inherits(false)
                .build(),
        );

        let mut parent = TestElement::new(1, None);
        let child = TestElement::new(2, Some(1));

        parent.set_local(width, 200.0);

        let elements: alloc::collections::BTreeMap<u32, &TestElement> =
            [(1, &parent), (2, &child)].into_iter().collect();

        let value = child.get_inherited(width, &registry, &|key| {
            elements
                .get(&key)
                .map(|e| (e.property_store(), e.parent_key()))
        });
        assert_eq!(value, 100.0); // Default, not parent's 200.0
    }

    #[test]
    fn ext_set_local_notifying() {
        use invalidation::Channel;

        const LAYOUT: Channel = Channel::new(0);

        let mut registry = PropertyRegistry::new();
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .coerce(|v| v.max(0.0))
                .build(),
        );

        let mut element = TestElement::new(1, None);

        let channels = element.set_local_notifying(width, 10.0, &registry);

        // Value was stored
        assert_eq!(element.get_local_value(width), Some(&10.0));

        // Returns affected channels
        assert!(channels.contains(LAYOUT));
    }

    #[test]
    fn ext_set_local_notifying_skips_noop_effective_changes() {
        use alloc::sync::Arc;
        use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

        let mut registry = PropertyRegistry::new();
        let callback_count = Arc::new(AtomicUsize::new(0));
        let last_old = Arc::new(AtomicU64::new(f64::NAN.to_bits()));
        let last_new = Arc::new(AtomicU64::new(f64::NAN.to_bits()));
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .on_changed({
                    let callback_count = Arc::clone(&callback_count);
                    let last_old = Arc::clone(&last_old);
                    let last_new = Arc::clone(&last_new);
                    move |old, new| {
                        callback_count.fetch_add(1, Ordering::Relaxed);
                        last_old.store(
                            old.copied().unwrap_or(f64::NAN).to_bits(),
                            Ordering::Relaxed,
                        );
                        last_new.store(new.to_bits(), Ordering::Relaxed);
                    }
                })
                .build(),
        );

        let mut element = TestElement::new(1, None);

        let first = element.set_local_notifying(width, 10.0, &registry);
        assert!(first.is_empty());
        assert_eq!(callback_count.load(Ordering::Relaxed), 1);
        assert_eq!(f64::from_bits(last_old.load(Ordering::Relaxed)), 0.0);
        assert_eq!(f64::from_bits(last_new.load(Ordering::Relaxed)), 10.0);

        let second = element.set_local_notifying(width, 10.0, &registry);
        assert!(second.is_empty());
        assert_eq!(callback_count.load(Ordering::Relaxed), 1);
        assert_eq!(f64::from_bits(last_old.load(Ordering::Relaxed)), 0.0);
        assert_eq!(f64::from_bits(last_new.load(Ordering::Relaxed)), 10.0);
    }

    #[test]
    fn ext_set_local_notifying_uses_effective_local_when_animation_masks_local() {
        use alloc::sync::Arc;
        use core::sync::atomic::{AtomicUsize, Ordering};
        use invalidation::Channel;

        let mut registry = PropertyRegistry::new();
        let callback_count = Arc::new(AtomicUsize::new(0));
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(Channel::new(0).into_set())
                .on_changed({
                    let callback_count = Arc::clone(&callback_count);
                    move |_, _| {
                        callback_count.fetch_add(1, Ordering::Relaxed);
                    }
                })
                .build(),
        );

        let mut element = TestElement::new(1, None);
        element.set_animation(width, 50.0);

        let channels = element.set_local_notifying(width, 10.0, &registry);

        assert!(channels.is_empty());
        assert_eq!(callback_count.load(Ordering::Relaxed), 0);
        assert_eq!(element.get_local_value(width), Some(&10.0));
        assert_eq!(element.get_effective_local(width, &registry), 50.0);
    }

    #[test]
    fn ext_clear_local_notifying_reports_effective_change() {
        use alloc::sync::Arc;
        use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
        use invalidation::Channel;

        const LAYOUT: Channel = Channel::new(0);

        let mut registry = PropertyRegistry::new();
        let callback_count = Arc::new(AtomicUsize::new(0));
        let last_old = Arc::new(AtomicU64::new(f64::NAN.to_bits()));
        let last_new = Arc::new(AtomicU64::new(f64::NAN.to_bits()));
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .on_changed({
                    let callback_count = Arc::clone(&callback_count);
                    let last_old = Arc::clone(&last_old);
                    let last_new = Arc::clone(&last_new);
                    move |old, new| {
                        callback_count.fetch_add(1, Ordering::Relaxed);
                        last_old.store(
                            old.copied().unwrap_or(f64::NAN).to_bits(),
                            Ordering::Relaxed,
                        );
                        last_new.store(new.to_bits(), Ordering::Relaxed);
                    }
                })
                .build(),
        );

        let mut element = TestElement::new(1, None);
        element.set_local(width, 10.0);

        let channels = element.clear_local_notifying(width, &registry);

        assert!(channels.contains(LAYOUT));
        assert!(element.get_local_value(width).is_none());
        assert_eq!(element.get_effective_local(width, &registry), 0.0);
        assert_eq!(callback_count.load(Ordering::Relaxed), 1);
        assert_eq!(f64::from_bits(last_old.load(Ordering::Relaxed)), 10.0);
        assert_eq!(f64::from_bits(last_new.load(Ordering::Relaxed)), 0.0);
    }

    #[test]
    fn ext_clear_local_notifying_skips_masked_changes() {
        use alloc::sync::Arc;
        use core::sync::atomic::{AtomicUsize, Ordering};
        use invalidation::Channel;

        let mut registry = PropertyRegistry::new();
        let callback_count = Arc::new(AtomicUsize::new(0));
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(Channel::new(0).into_set())
                .on_changed({
                    let callback_count = Arc::clone(&callback_count);
                    move |_, _| {
                        callback_count.fetch_add(1, Ordering::Relaxed);
                    }
                })
                .build(),
        );

        let mut element = TestElement::new(1, None);
        element.set_animation(width, 50.0);
        element.set_local(width, 10.0);

        let channels = element.clear_local_notifying(width, &registry);

        assert!(channels.is_empty());
        assert!(element.get_local_value(width).is_none());
        assert_eq!(element.get_effective_local(width, &registry), 50.0);
        assert_eq!(callback_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn ext_clear_animation_notifying_reports_effective_change() {
        use alloc::sync::Arc;
        use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
        use invalidation::Channel;

        const LAYOUT: Channel = Channel::new(0);

        let mut registry = PropertyRegistry::new();
        let callback_count = Arc::new(AtomicUsize::new(0));
        let last_old = Arc::new(AtomicU64::new(f64::NAN.to_bits()));
        let last_new = Arc::new(AtomicU64::new(f64::NAN.to_bits()));
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .on_changed({
                    let callback_count = Arc::clone(&callback_count);
                    let last_old = Arc::clone(&last_old);
                    let last_new = Arc::clone(&last_new);
                    move |old, new| {
                        callback_count.fetch_add(1, Ordering::Relaxed);
                        last_old.store(
                            old.copied().unwrap_or(f64::NAN).to_bits(),
                            Ordering::Relaxed,
                        );
                        last_new.store(new.to_bits(), Ordering::Relaxed);
                    }
                })
                .build(),
        );

        let mut element = TestElement::new(1, None);
        element.set_local(width, 10.0);
        element.set_animation(width, 20.0);

        let channels = element.clear_animation_notifying(width, &registry);

        assert!(channels.contains(LAYOUT));
        assert!(element.get_animation_value(width).is_none());
        assert_eq!(element.get_effective_local(width, &registry), 10.0);
        assert_eq!(callback_count.load(Ordering::Relaxed), 1);
        assert_eq!(f64::from_bits(last_old.load(Ordering::Relaxed)), 20.0);
        assert_eq!(f64::from_bits(last_new.load(Ordering::Relaxed)), 10.0);
    }

    #[test]
    fn ext_clear_animation_notifying_skips_noop_effective_changes() {
        use alloc::sync::Arc;
        use core::sync::atomic::{AtomicUsize, Ordering};
        use invalidation::Channel;

        let mut registry = PropertyRegistry::new();
        let callback_count = Arc::new(AtomicUsize::new(0));
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(Channel::new(0).into_set())
                .on_changed({
                    let callback_count = Arc::clone(&callback_count);
                    move |_, _| {
                        callback_count.fetch_add(1, Ordering::Relaxed);
                    }
                })
                .build(),
        );

        let mut element = TestElement::new(1, None);
        element.set_local(width, 20.0);
        element.set_animation(width, 20.0);

        let channels = element.clear_animation_notifying(width, &registry);

        assert!(channels.is_empty());
        assert!(element.get_animation_value(width).is_none());
        assert_eq!(element.get_effective_local(width, &registry), 20.0);
        assert_eq!(callback_count.load(Ordering::Relaxed), 0);
    }

    // -------------------------------------------------------------------------
    // Source-tagged Local writes
    // -------------------------------------------------------------------------

    #[test]
    fn ext_set_local_with_source_writes_to_slot() {
        let (_, width) = setup_registry();
        let mut element = TestElement::new(1, None);

        element.set_local_with_source(width, 10.0, LocalValueSource::TemplateDefault);
        assert_eq!(
            element.get_local_source(width),
            Some(LocalValueSource::TemplateDefault)
        );

        // Writing a higher source takes over the winning value but leaves the
        // lower slot intact.
        element.set_local_with_source(width, 20.0, LocalValueSource::TemplateBinding);
        assert_eq!(element.get_local_value(width), Some(&20.0));
        assert_eq!(
            element.get_local_source(width),
            Some(LocalValueSource::TemplateBinding)
        );
        assert_eq!(
            element
                .property_store()
                .get_local_at_source(width, LocalValueSource::TemplateDefault),
            Some(&10.0)
        );
    }

    #[test]
    fn ext_set_local_with_source_notifying_returns_channels_on_effective_change() {
        use invalidation::Channel;

        const LAYOUT: Channel = Channel::new(0);

        let mut registry = PropertyRegistry::new();
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .build(),
        );

        let mut element = TestElement::new(1, None);
        let channels = element.set_local_with_source_notifying(
            width,
            10.0,
            LocalValueSource::TemplateDefault,
            &registry,
        );

        assert!(channels.contains(LAYOUT));
        assert_eq!(element.get_local_value(width), Some(&10.0));
        assert_eq!(
            element.get_local_source(width),
            Some(LocalValueSource::TemplateDefault)
        );
    }

    #[test]
    fn ext_set_local_with_source_notifying_shadowed_write_returns_empty() {
        use alloc::sync::Arc;
        use core::sync::atomic::{AtomicUsize, Ordering};
        use invalidation::Channel;

        let mut registry = PropertyRegistry::new();
        let callback_count = Arc::new(AtomicUsize::new(0));
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(Channel::new(0).into_set())
                .on_changed({
                    let callback_count = Arc::clone(&callback_count);
                    move |_, _| {
                        callback_count.fetch_add(1, Ordering::Relaxed);
                    }
                })
                .build(),
        );

        let mut element = TestElement::new(1, None);
        // Seed with Local (highest precedence).
        let _ = element.set_local_notifying(width, 100.0, &registry);
        let cb_before = callback_count.load(Ordering::Relaxed);

        // Writing to a lower slot writes the value but doesn't move the
        // effective value, so no channels and no callback.
        let channels = element.set_local_with_source_notifying(
            width,
            7.0,
            LocalValueSource::TemplateBinding,
            &registry,
        );

        assert!(channels.is_empty());
        assert_eq!(element.get_local_value(width), Some(&100.0));
        assert_eq!(
            element.get_local_source(width),
            Some(LocalValueSource::Local)
        );
        assert_eq!(callback_count.load(Ordering::Relaxed), cb_before);
        // The shadowed write still landed in its slot.
        assert_eq!(
            element
                .property_store()
                .get_local_at_source(width, LocalValueSource::TemplateBinding),
            Some(&7.0)
        );
    }

    #[test]
    fn ext_set_local_with_source_notifying_no_effective_change_returns_empty() {
        use invalidation::Channel;

        const LAYOUT: Channel = Channel::new(0);

        let mut registry = PropertyRegistry::new();
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .build(),
        );

        let mut element = TestElement::new(1, None);
        element.set_local_with_source(width, 10.0, LocalValueSource::TemplateDefault);

        // Promote to a higher source with the same value: effective value
        // didn't move, so no channels.
        let channels = element.set_local_with_source_notifying(
            width,
            10.0,
            LocalValueSource::TemplateBinding,
            &registry,
        );

        assert!(channels.is_empty());
        assert_eq!(
            element.get_local_source(width),
            Some(LocalValueSource::TemplateBinding),
            "the higher slot should still have been written"
        );
    }

    #[test]
    fn ext_clear_local_by_source_drops_only_matching_entries() {
        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
        let height = registry.register("Height", PropertyMetadataBuilder::new(0.0_f64).build());

        let mut element = TestElement::new(1, None);
        element.set_local_with_source(width, 10.0, LocalValueSource::TemplateDefault);
        element.set_local_with_source(height, 20.0, LocalValueSource::TemplateBinding);
        element.set_local(width, 99.0); // Adds a Local on top of width's TemplateDefault.

        let removed = element.clear_local_by_source(LocalValueSource::TemplateBinding);
        assert_eq!(removed, 1);
        assert!(element.has_local(width));
        assert!(!element.has_local(height));
        assert_eq!(
            element.get_local_source(width),
            Some(LocalValueSource::Local)
        );
        // Width's TemplateDefault is still there, masked by Local.
        assert_eq!(
            element
                .property_store()
                .get_local_at_source(width, LocalValueSource::TemplateDefault),
            Some(&10.0)
        );
    }

    // -------------------------------------------------------------------------
    // clear_local_by_source_notifying — channel reporting for bulk teardown
    // -------------------------------------------------------------------------

    #[test]
    fn ext_clear_local_by_source_notifying_returns_channels_on_visible_change() {
        use invalidation::Channel;

        const LAYOUT: Channel = Channel::new(0);
        const PAINT: Channel = Channel::new(1);

        let mut registry = PropertyRegistry::new();
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .build(),
        );
        let height = registry.register(
            "Height",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(PAINT.into_set())
                .build(),
        );

        let mut element = TestElement::new(1, None);
        // TemplateBinding is the winner for both properties.
        element.set_local_with_source(width, 10.0, LocalValueSource::TemplateBinding);
        element.set_local_with_source(height, 20.0, LocalValueSource::TemplateBinding);

        let channels =
            element.clear_local_by_source_notifying(LocalValueSource::TemplateBinding, &registry);

        // Both properties had visible changes; both contribute channels.
        assert!(channels.contains(LAYOUT));
        assert!(channels.contains(PAINT));
        assert!(!element.has_local(width));
        assert!(!element.has_local(height));
    }

    #[test]
    fn ext_clear_local_by_source_notifying_skips_masked_lower_source() {
        use invalidation::Channel;

        const LAYOUT: Channel = Channel::new(0);

        let mut registry = PropertyRegistry::new();
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .build(),
        );

        let mut element = TestElement::new(1, None);
        // TemplateBinding is below the user's Local; the visible value is the
        // Local, so clearing TemplateBinding should *not* report channels.
        element.set_local_with_source(width, 10.0, LocalValueSource::TemplateBinding);
        element.set_local(width, 99.0);

        let channels =
            element.clear_local_by_source_notifying(LocalValueSource::TemplateBinding, &registry);

        assert!(channels.is_empty());
        // Local is still there.
        assert_eq!(element.get_local_value(width), Some(&99.0));
        // The TemplateBinding slot was emptied.
        assert!(
            !element
                .property_store()
                .has_local_at_source(width, LocalValueSource::TemplateBinding)
        );
    }

    #[test]
    fn ext_clear_local_by_source_notifying_skips_animation_masked() {
        use invalidation::Channel;

        const LAYOUT: Channel = Channel::new(0);

        let mut registry = PropertyRegistry::new();
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .build(),
        );

        let mut element = TestElement::new(1, None);
        element.set_local_with_source(width, 10.0, LocalValueSource::TemplateBinding);
        element.set_animation(width, 50.0); // Animation masks the entire local layer.

        let channels =
            element.clear_local_by_source_notifying(LocalValueSource::TemplateBinding, &registry);

        // Animation still wins; visible value didn't change.
        assert!(channels.is_empty());
        assert_eq!(element.get_effective_local(width, &registry), 50.0);
    }

    // -------------------------------------------------------------------------
    // clear_local_at_source_notifying — per-property, per-source, with callback
    // -------------------------------------------------------------------------

    #[test]
    fn ext_clear_local_at_source_notifying_reveals_lower_source() {
        use alloc::sync::Arc;
        use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
        use invalidation::Channel;

        const LAYOUT: Channel = Channel::new(0);

        let mut registry = PropertyRegistry::new();
        let callback_count = Arc::new(AtomicUsize::new(0));
        let last_old = Arc::new(AtomicU64::new(f64::NAN.to_bits()));
        let last_new = Arc::new(AtomicU64::new(f64::NAN.to_bits()));
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .on_changed({
                    let callback_count = Arc::clone(&callback_count);
                    let last_old = Arc::clone(&last_old);
                    let last_new = Arc::clone(&last_new);
                    move |old, new| {
                        callback_count.fetch_add(1, Ordering::Relaxed);
                        last_old.store(
                            old.copied().unwrap_or(f64::NAN).to_bits(),
                            Ordering::Relaxed,
                        );
                        last_new.store(new.to_bits(), Ordering::Relaxed);
                    }
                })
                .build(),
        );

        let mut element = TestElement::new(1, None);
        element.set_local_with_source(width, 10.0, LocalValueSource::TemplateDefault);
        element.set_local_with_source(width, 20.0, LocalValueSource::TemplateBinding);

        let channels = element.clear_local_at_source_notifying(
            width,
            LocalValueSource::TemplateBinding,
            &registry,
        );

        assert!(channels.contains(LAYOUT));
        assert_eq!(element.get_effective_local(width, &registry), 10.0);
        assert_eq!(callback_count.load(Ordering::Relaxed), 1);
        assert_eq!(f64::from_bits(last_old.load(Ordering::Relaxed)), 20.0);
        assert_eq!(f64::from_bits(last_new.load(Ordering::Relaxed)), 10.0);
    }

    #[test]
    fn ext_clear_local_at_source_notifying_masked_by_local_no_notify() {
        use alloc::sync::Arc;
        use core::sync::atomic::{AtomicUsize, Ordering};
        use invalidation::Channel;

        let mut registry = PropertyRegistry::new();
        let callback_count = Arc::new(AtomicUsize::new(0));
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(Channel::new(0).into_set())
                .on_changed({
                    let callback_count = Arc::clone(&callback_count);
                    move |_, _| {
                        callback_count.fetch_add(1, Ordering::Relaxed);
                    }
                })
                .build(),
        );

        let mut element = TestElement::new(1, None);
        element.set_local_with_source(width, 20.0, LocalValueSource::TemplateBinding);
        element.set_local(width, 99.0); // Local masks the TemplateBinding.

        let channels = element.clear_local_at_source_notifying(
            width,
            LocalValueSource::TemplateBinding,
            &registry,
        );

        assert!(channels.is_empty());
        assert_eq!(callback_count.load(Ordering::Relaxed), 0);
        // Local is unchanged; the TemplateBinding slot was emptied.
        assert_eq!(element.get_effective_local(width, &registry), 99.0);
        assert!(
            !element
                .property_store()
                .has_local_at_source(width, LocalValueSource::TemplateBinding)
        );
    }

    #[test]
    fn ext_clear_local_at_source_notifying_equal_underneath_no_notify() {
        use alloc::sync::Arc;
        use core::sync::atomic::{AtomicUsize, Ordering};
        use invalidation::Channel;

        let mut registry = PropertyRegistry::new();
        let callback_count = Arc::new(AtomicUsize::new(0));
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(Channel::new(0).into_set())
                .on_changed({
                    let callback_count = Arc::clone(&callback_count);
                    move |_, _| {
                        callback_count.fetch_add(1, Ordering::Relaxed);
                    }
                })
                .build(),
        );

        let mut element = TestElement::new(1, None);
        // TemplateDefault(10) shadowed by TemplateBinding(10): clearing the
        // binding reveals the same value beneath, so effective doesn't move.
        element.set_local_with_source(width, 10.0, LocalValueSource::TemplateDefault);
        element.set_local_with_source(width, 10.0, LocalValueSource::TemplateBinding);

        let channels = element.clear_local_at_source_notifying(
            width,
            LocalValueSource::TemplateBinding,
            &registry,
        );

        assert!(
            channels.is_empty(),
            "exact value comparison: effective didn't change"
        );
        assert_eq!(callback_count.load(Ordering::Relaxed), 0);
        assert_eq!(element.get_effective_local(width, &registry), 10.0);
        // TemplateDefault is now the winner.
        assert_eq!(
            element.get_local_source(width),
            Some(LocalValueSource::TemplateDefault)
        );
    }

    #[test]
    fn ext_clear_local_at_source_notifying_empty_slot_returns_empty() {
        use invalidation::Channel;

        let mut registry = PropertyRegistry::new();
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(Channel::new(0).into_set())
                .build(),
        );

        let mut element = TestElement::new(1, None);
        // No value at TemplateBinding to clear.
        let channels = element.clear_local_at_source_notifying(
            width,
            LocalValueSource::TemplateBinding,
            &registry,
        );

        assert!(channels.is_empty());
    }

    #[test]
    fn ext_clear_local_by_source_notifying_is_conservative_on_equal_shadow() {
        // Pin the intentional conservative bulk behavior: clearing
        // TemplateBinding(10) when TemplateDefault(10) is underneath still
        // returns the channel even though the effective value didn't move.
        // Bulk clear doesn't do typed value comparison; callers should use
        // `clear_local_at_source_notifying` if they need exact detection.
        use invalidation::Channel;

        const LAYOUT: Channel = Channel::new(0);

        let mut registry = PropertyRegistry::new();
        let width = registry.register(
            "Width",
            PropertyMetadataBuilder::new(0.0_f64)
                .affects_channels(LAYOUT.into_set())
                .build(),
        );

        let mut element = TestElement::new(1, None);
        element.set_local_with_source(width, 10.0, LocalValueSource::TemplateDefault);
        element.set_local_with_source(width, 10.0, LocalValueSource::TemplateBinding);

        let channels =
            element.clear_local_by_source_notifying(LocalValueSource::TemplateBinding, &registry);

        assert!(
            channels.contains(LAYOUT),
            "bulk clear is conservative: returns the channel even with equal shadowed value"
        );
        // TemplateDefault is now the winner.
        assert_eq!(element.get_effective_local(width, &registry), 10.0);
        assert_eq!(
            element.get_local_source(width),
            Some(LocalValueSource::TemplateDefault)
        );
    }

    // -------------------------------------------------------------------------
    // clear_all — full per-property wipe across all slots
    // -------------------------------------------------------------------------

    #[test]
    fn ext_clear_all_drops_every_slot_for_one_property() {
        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
        let count = registry.register("Count", PropertyMetadataBuilder::new(0_i32).build());

        let mut element = TestElement::new(1, None);
        // Width has every slot populated.
        element.set_local_with_source(width, 1.0, LocalValueSource::TemplateDefault);
        element.set_local_with_source(width, 2.0, LocalValueSource::TemplateBinding);
        element.set_local(width, 3.0);
        element.set_animation(width, 4.0);
        // Count is a different property that should be untouched.
        element.set_local(count, 99);

        let removed = element.clear_all(width);
        assert!(removed);

        // Every slot for `width` is gone.
        assert!(!element.property_store().has_value(width));
        assert!(
            !element
                .property_store()
                .has_local_at_source(width, LocalValueSource::Local)
        );
        assert!(
            !element
                .property_store()
                .has_local_at_source(width, LocalValueSource::TemplateBinding)
        );
        assert!(
            !element
                .property_store()
                .has_local_at_source(width, LocalValueSource::TemplateDefault)
        );
        assert!(!element.has_animation(width));

        // `count` is untouched.
        assert_eq!(element.get_local_value(count), Some(&99));
    }
}
