// Copyright 2025 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Property metadata definitions.
//!
//! This module provides [`PropertyMetadata`] for storing property configuration
//! and [`PropertyMetadataBuilder`] for ergonomic construction.

use alloc::boxed::Box;
use invalidation::ChannelSet;

use crate::coerce::CoerceContext;
use crate::id::PropertyId;
use crate::value::ErasedValue;

/// Callback invoked when a property value changes.
///
/// The callback receives the previous effective-local value and the new
/// effective-local value when invoked through notifying helpers such as
/// [`crate::DependencyObjectExt::set_local_notifying`].
pub type PropertyChangedCallback<T> = Box<dyn Fn(Option<&T>, &T) + Send + Sync>;

/// Callback for coercing a property value before it's stored.
///
/// This can be used to clamp values into range, snap to ticks, etc. The
/// callback receives a [`CoerceContext`] giving read access to the owning
/// object's other effective property values (e.g. `Slider.Value` clamping
/// against the current `Minimum`/`Maximum`), plus the proposed value, and
/// returns the coerced value.
///
/// Coercion **adjusts** a value; to **reject** one, use
/// [`ValidateValueCallback`] instead.
pub type CoerceValueCallback<T> = Box<dyn Fn(&CoerceContext<'_>, T) -> T + Send + Sync>;

/// Callback for validating a raw property value before coercion.
///
/// Returns `true` if the value is acceptable. Validation **rejects** invalid
/// values (the write fails with [`ValidationError`]); it must not depend on
/// other property values and runs on the *raw* incoming value, before
/// coercion. Coercion, by contrast, **adjusts** values.
pub type ValidateValueCallback<T> = Box<dyn Fn(&T) -> bool + Send + Sync>;

/// Error returned when a write is rejected by a property's validate callback.
///
/// The rejected raw value is handed back to the caller so it isn't lost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError<T> {
    /// The property whose validate callback rejected the value.
    pub property: PropertyId,
    /// The rejected raw value (before coercion).
    pub rejected: T,
}

impl<T> core::fmt::Display for ValidationError<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "value for property {} was rejected by its validate callback",
            self.property
        )
    }
}

impl<T: core::fmt::Debug> core::error::Error for ValidationError<T> {}

/// Error returned when a type-erased checked write is rejected by validation.
///
/// This is the erased counterpart to [`ValidationError`]. It is used by
/// adapter/runtime code that carries [`PropertyId`] plus [`ErasedValue`] rather
/// than a typed [`crate::Property<T>`] handle. The rejected raw value is
/// returned so callers can surface or recover the input that failed.
#[derive(Debug, Clone)]
pub struct ErasedValidationError {
    /// The property whose validate callback rejected the value.
    pub property: PropertyId,
    /// The rejected raw erased value (before coercion).
    pub rejected: ErasedValue,
}

impl core::fmt::Display for ErasedValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "value for property {} was rejected by its validate callback",
            self.property
        )
    }
}

impl core::error::Error for ErasedValidationError {}

/// Metadata for a dependency property.
///
/// This contains the configuration for a property including its default value,
/// whether it inherits, which dirty channels it affects, and optional callbacks.
///
/// # Example
///
/// ```rust
/// use understory_property::PropertyMetadataBuilder;
/// use invalidation::Channel;
///
/// const LAYOUT: Channel = Channel::new(0);
///
/// let metadata = PropertyMetadataBuilder::new(100.0_f64)
///     .inherits(true)
///     .affects_channels(LAYOUT.into_set())
///     .build();
///
/// assert_eq!(metadata.default_value(), &100.0);
/// assert!(metadata.inherits());
/// ```
pub struct PropertyMetadata<T: Clone + 'static> {
    default_value: T,
    inherits: bool,
    affects_channels: ChannelSet,
    changed_callback: Option<PropertyChangedCallback<T>>,
    coerce_callback: Option<CoerceValueCallback<T>>,
    validate_callback: Option<ValidateValueCallback<T>>,
}

impl<T: Clone + 'static> PropertyMetadata<T> {
    /// Creates new property metadata with the given default value.
    ///
    /// All other fields use their defaults:
    /// - `inherits`: `false`
    /// - `affects_channels`: empty
    /// - `changed_callback`: `None`
    /// - `coerce_callback`: `None`
    /// - `validate_callback`: `None`
    #[must_use]
    pub fn new(default_value: T) -> Self {
        Self {
            default_value,
            inherits: false,
            affects_channels: ChannelSet::empty(),
            changed_callback: None,
            coerce_callback: None,
            validate_callback: None,
        }
    }

    /// Returns a reference to the default value.
    #[must_use]
    #[inline]
    pub fn default_value(&self) -> &T {
        &self.default_value
    }

    /// Returns whether this property inherits from parent objects.
    #[must_use]
    #[inline]
    pub fn inherits(&self) -> bool {
        self.inherits
    }

    /// Returns the dirty channels affected by changes to this property.
    #[must_use]
    #[inline]
    pub fn affects_channels(&self) -> ChannelSet {
        self.affects_channels
    }

    /// Invokes the changed callback if one is set.
    #[inline]
    pub fn on_changed(&self, old_value: Option<&T>, new_value: &T) {
        if let Some(callback) = &self.changed_callback {
            callback(old_value, new_value);
        }
    }

    /// Coerces a value using the coerce callback if one is set.
    ///
    /// The [`CoerceContext`] lets the callback read the owning object's other
    /// effective property values (e.g. clamping `Value` against the current
    /// `Minimum`/`Maximum`).
    #[inline]
    pub fn coerce(&self, cx: &CoerceContext<'_>, value: T) -> T {
        if let Some(callback) = &self.coerce_callback {
            callback(cx, value)
        } else {
            value
        }
    }

    /// Validates a raw value using the validate callback if one is set.
    ///
    /// Returns `true` when no validate callback is registered.
    #[must_use]
    #[inline]
    pub fn validate(&self, value: &T) -> bool {
        self.validate_callback.as_ref().is_none_or(|f| f(value))
    }

    /// Returns whether a changed callback is set.
    #[must_use]
    #[inline]
    pub fn has_changed_callback(&self) -> bool {
        self.changed_callback.is_some()
    }

    /// Returns whether a coerce callback is set.
    #[must_use]
    #[inline]
    pub fn has_coerce_callback(&self) -> bool {
        self.coerce_callback.is_some()
    }

    /// Returns whether a validate callback is set.
    #[must_use]
    #[inline]
    pub fn has_validate_callback(&self) -> bool {
        self.validate_callback.is_some()
    }
}

// Manual Debug impl since callbacks aren't Debug
impl<T: Clone + core::fmt::Debug + 'static> core::fmt::Debug for PropertyMetadata<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PropertyMetadata")
            .field("default_value", &self.default_value)
            .field("inherits", &self.inherits)
            .field("affects_channels", &self.affects_channels)
            .field("has_changed_callback", &self.changed_callback.is_some())
            .field("has_coerce_callback", &self.coerce_callback.is_some())
            .field("has_validate_callback", &self.validate_callback.is_some())
            .finish()
    }
}

/// Builder for [`PropertyMetadata`].
///
/// # Example
///
/// ```rust
/// use understory_property::PropertyMetadataBuilder;
/// use invalidation::Channel;
///
/// const LAYOUT: Channel = Channel::new(0);
/// const PAINT: Channel = Channel::new(1);
///
/// let metadata = PropertyMetadataBuilder::new(0.0_f64)
///     .inherits(true)
///     .affects_channels(LAYOUT.into_set() | PAINT.into_set())
///     .coerce(|v| v.clamp(0.0, 100.0))
///     .build();
/// ```
pub struct PropertyMetadataBuilder<T: Clone + 'static> {
    default_value: T,
    inherits: bool,
    affects_channels: ChannelSet,
    changed_callback: Option<PropertyChangedCallback<T>>,
    coerce_callback: Option<CoerceValueCallback<T>>,
    validate_callback: Option<ValidateValueCallback<T>>,
}

// Manual Debug impl since callbacks aren't Debug
impl<T: Clone + core::fmt::Debug + 'static> core::fmt::Debug for PropertyMetadataBuilder<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PropertyMetadataBuilder")
            .field("default_value", &self.default_value)
            .field("inherits", &self.inherits)
            .field("affects_channels", &self.affects_channels)
            .field("has_changed_callback", &self.changed_callback.is_some())
            .field("has_coerce_callback", &self.coerce_callback.is_some())
            .field("has_validate_callback", &self.validate_callback.is_some())
            .finish()
    }
}

impl<T: Clone + 'static> PropertyMetadataBuilder<T> {
    /// Creates a new builder with the given default value.
    #[must_use]
    pub fn new(default_value: T) -> Self {
        Self {
            default_value,
            inherits: false,
            affects_channels: ChannelSet::empty(),
            changed_callback: None,
            coerce_callback: None,
            validate_callback: None,
        }
    }

    /// Sets whether this property inherits from parent objects.
    ///
    /// When `true`, `get_effective` will walk up the parent chain to find
    /// a value if none is set locally.
    #[must_use]
    pub fn inherits(mut self, inherits: bool) -> Self {
        self.inherits = inherits;
        self
    }

    /// Sets the dirty channels affected by changes to this property.
    ///
    /// When the property changes, these channels will be marked dirty.
    #[must_use]
    pub fn affects_channels(mut self, channels: ChannelSet) -> Self {
        self.affects_channels = channels;
        self
    }

    /// Sets a callback to be invoked when the property value changes.
    #[must_use]
    pub fn on_changed<F>(mut self, callback: F) -> Self
    where
        F: Fn(Option<&T>, &T) + Send + Sync + 'static,
    {
        self.changed_callback = Some(Box::new(callback));
        self
    }

    /// Sets a context-free callback to coerce values before they are stored.
    ///
    /// Use this for static adjustments such as clamping to a fixed range,
    /// snapping, or normalizing non-finite values. It is shorthand for
    /// [`coerce_with`](Self::coerce_with) with a callback that ignores the
    /// [`CoerceContext`].
    #[must_use]
    pub fn coerce<F>(mut self, callback: F) -> Self
    where
        F: Fn(T) -> T + Send + Sync + 'static,
    {
        self.coerce_callback = Some(Box::new(move |_cx: &CoerceContext<'_>, value| {
            callback(value)
        }));
        self
    }

    /// Sets a context-aware callback to coerce values before they are stored.
    ///
    /// The callback receives a [`CoerceContext`] with read access to the
    /// owning object's other effective property values, enabling
    /// cross-property coercion such as clamping `Slider.Value` against the
    /// current `Minimum`/`Maximum`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use understory_property::{CoerceContext, PropertyMetadataBuilder, PropertyRegistry};
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
    /// ```
    #[must_use]
    pub fn coerce_with<F>(mut self, callback: F) -> Self
    where
        F: Fn(&CoerceContext<'_>, T) -> T + Send + Sync + 'static,
    {
        self.coerce_callback = Some(Box::new(callback));
        self
    }

    /// Sets a callback to validate raw values before coercion.
    ///
    /// Validation **rejects**: a write whose value fails validation does not
    /// happen and reports [`ValidationError`] (or panics, on the infallible
    /// notifying paths). It runs on the raw incoming value, *before*
    /// coercion, and deliberately receives no [`CoerceContext`]: validation
    /// is for intrinsic input well-formedness, while cross-property adjustment
    /// belongs in [`coerce_with`](Self::coerce_with).
    ///
    /// The property's default value must itself pass validation;
    /// [`PropertyRegistry::register`](crate::PropertyRegistry::register)
    /// panics otherwise.
    #[must_use]
    pub fn validate<F>(mut self, callback: F) -> Self
    where
        F: Fn(&T) -> bool + Send + Sync + 'static,
    {
        self.validate_callback = Some(Box::new(callback));
        self
    }

    /// Builds the [`PropertyMetadata`].
    #[must_use]
    pub fn build(self) -> PropertyMetadata<T> {
        PropertyMetadata {
            default_value: self.default_value,
            inherits: self.inherits,
            affects_channels: self.affects_channels,
            changed_callback: self.changed_callback,
            coerce_callback: self.coerce_callback,
            validate_callback: self.validate_callback,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicBool, Ordering};
    use invalidation::Channel;

    const LAYOUT: Channel = Channel::new(0);
    const PAINT: Channel = Channel::new(1);

    /// Builds a context over an empty store/registry for callback tests.
    fn with_empty_context<R>(f: impl FnOnce(&CoerceContext<'_>) -> R) -> R {
        let registry = crate::PropertyRegistry::new();
        let store = crate::PropertyStore::<u32>::new(1);
        let cx = CoerceContext::new(&store, &registry);
        f(&cx)
    }

    #[test]
    fn metadata_defaults() {
        let metadata = PropertyMetadata::new(42_i32);
        assert_eq!(metadata.default_value(), &42);
        assert!(!metadata.inherits());
        assert!(metadata.affects_channels().is_empty());
        assert!(!metadata.has_changed_callback());
        assert!(!metadata.has_coerce_callback());
        assert!(!metadata.has_validate_callback());
    }

    #[test]
    fn metadata_builder() {
        let metadata = PropertyMetadataBuilder::new(100.0_f64)
            .inherits(true)
            .affects_channels(LAYOUT.into_set() | PAINT.into_set())
            .build();

        assert_eq!(metadata.default_value(), &100.0);
        assert!(metadata.inherits());
        assert!(metadata.affects_channels().contains(LAYOUT));
        assert!(metadata.affects_channels().contains(PAINT));
    }

    #[test]
    fn metadata_coerce() {
        let metadata = PropertyMetadataBuilder::new(0.0_f64)
            .coerce(|v| v.clamp(0.0, 100.0))
            .build();

        with_empty_context(|cx| {
            assert_eq!(metadata.coerce(cx, -10.0), 0.0);
            assert_eq!(metadata.coerce(cx, 50.0), 50.0);
            assert_eq!(metadata.coerce(cx, 150.0), 100.0);
        });
    }

    #[test]
    fn metadata_coerce_with_reads_context() {
        let mut registry = crate::PropertyRegistry::new();
        let minimum = registry.register("Minimum", PropertyMetadataBuilder::new(0.0_f64).build());

        let metadata = PropertyMetadataBuilder::new(0.0_f64)
            .coerce_with(move |cx: &CoerceContext<'_>, v: f64| v.max(cx.get(minimum)))
            .build();
        assert!(metadata.has_coerce_callback());

        let mut store = crate::PropertyStore::<u32>::new(1);
        store.set_local(minimum, 10.0);

        let cx = CoerceContext::new(&store, &registry);
        assert_eq!(metadata.coerce(&cx, 5.0), 10.0);
        assert_eq!(metadata.coerce(&cx, 42.0), 42.0);
    }

    #[test]
    fn metadata_validate() {
        let metadata = PropertyMetadataBuilder::new(0.0_f64)
            .validate(|v| v.is_finite())
            .build();

        assert!(metadata.has_validate_callback());
        assert!(metadata.validate(&1.5));
        assert!(!metadata.validate(&f64::NAN));
        assert!(!metadata.validate(&f64::INFINITY));
    }

    #[test]
    fn metadata_validate_without_callback_accepts_everything() {
        let metadata = PropertyMetadata::new(0.0_f64);
        assert!(metadata.validate(&f64::NAN));
    }

    #[test]
    fn metadata_changed_callback() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let metadata = PropertyMetadataBuilder::new(0_i32)
            .on_changed(move |_, _| {
                called_clone.store(true, Ordering::SeqCst);
            })
            .build();

        assert!(metadata.has_changed_callback());
        assert!(!called.load(Ordering::SeqCst));

        metadata.on_changed(None, &42);
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn metadata_debug() {
        let metadata = PropertyMetadataBuilder::new(42_i32).inherits(true).build();

        let debug = format!("{:?}", metadata);
        assert!(debug.contains("PropertyMetadata"));
        assert!(debug.contains("42"));
        assert!(debug.contains("true"));
    }
}
