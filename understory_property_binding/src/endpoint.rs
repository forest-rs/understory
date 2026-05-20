// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

use understory_property::{Property, PropertyId};

/// Identifier for a registered binding.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingId(u32);

impl BindingId {
    /// Creates a binding id from a raw integer.
    ///
    /// This is primarily useful for tests and diagnostics. [`crate::BindingSet`]
    /// assigns ids when bindings are registered.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw integer id.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    pub(crate) fn index(self) -> Option<usize> {
        usize::try_from(self.0).ok()
    }
}

/// Untyped key for one property endpoint on one host object.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EndpointKey<K> {
    owner: K,
    property: PropertyId,
}

impl<K> EndpointKey<K> {
    /// Creates an untyped endpoint key.
    #[must_use]
    pub const fn new(owner: K, property: PropertyId) -> Self {
        Self { owner, property }
    }

    /// Returns the property id for this endpoint.
    #[must_use]
    pub const fn property(&self) -> PropertyId {
        self.property
    }
}

impl<K: Copy> EndpointKey<K> {
    /// Returns the host-defined owner key for this endpoint.
    #[must_use]
    pub const fn owner(self) -> K {
        self.owner
    }
}

/// Typed endpoint for one [`Property`] on one host object.
pub struct PropertyEndpoint<K, T> {
    owner: K,
    property: Property<T>,
}

impl<K, T> PropertyEndpoint<K, T> {
    /// Creates a typed property endpoint.
    #[must_use]
    pub const fn new(owner: K, property: Property<T>) -> Self {
        Self { owner, property }
    }

    /// Returns the property handle for this endpoint.
    #[must_use]
    pub const fn property(&self) -> Property<T> {
        self.property
    }
}

impl<K: Copy, T> PropertyEndpoint<K, T> {
    /// Returns the host-defined owner key for this endpoint.
    #[must_use]
    pub const fn owner(self) -> K {
        self.owner
    }

    /// Erases the value type and returns the runtime endpoint key.
    #[must_use]
    pub const fn key(self) -> EndpointKey<K> {
        EndpointKey::new(self.owner, self.property.id())
    }
}

impl<K: Copy, T> Copy for PropertyEndpoint<K, T> {}

impl<K: Copy, T> Clone for PropertyEndpoint<K, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: fmt::Debug, T> fmt::Debug for PropertyEndpoint<K, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PropertyEndpoint")
            .field("owner", &self.owner)
            .field("property", &self.property.id())
            .field("value_type", &core::any::type_name::<T>())
            .finish()
    }
}
