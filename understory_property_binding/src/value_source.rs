// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Single-value binding sources for hosts that do not need a full property store.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;

use understory_property::{ErasedValue, Property, PropertyId};

use crate::endpoint::PropertyEndpoint;

/// Synthetic property id used by single-value binding sources.
///
/// The id is only meaningful for owner keys that a host routes to a
/// [`ValueSourceStore`]. It does not need to be registered in an
/// [`understory_property::PropertyRegistry`].
pub const VALUE_SOURCE_PROPERTY_ID: PropertyId = PropertyId::new(u16::MAX);

/// Returns the synthetic property used by single-value binding sources.
///
/// This allows a stored value or external source to participate in the normal
/// [`PropertyEndpoint`] graph even though it is not a full property-store-backed
/// object.
#[must_use]
pub const fn value_source_property<T>() -> Property<T> {
    Property::from_id(VALUE_SOURCE_PROPERTY_ID)
}

/// Stable id for a stored single-value binding source.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ValueSourceId(usize);

impl ValueSourceId {
    /// Returns the source index inside the owning [`ValueSourceStore`].
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Stable id for a pull-based external binding source.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ExternalSourceId(usize);

impl ExternalSourceId {
    /// Returns the source index inside the owning [`ValueSourceStore`].
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Typed handle for a stored single-value binding source.
///
/// A `ValueSource<T>` is not a property store. It is a lightweight source for
/// cases where one value should feed the binding graph without creating a full
/// host object.
pub struct ValueSource<T> {
    id: ValueSourceId,
    _marker: PhantomData<fn() -> T>,
}

impl<T> ValueSource<T> {
    const fn new(id: ValueSourceId) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }

    /// Returns this source's stable id.
    #[must_use]
    pub const fn id(self) -> ValueSourceId {
        self.id
    }

    /// Returns this source as a typed endpoint for the given host owner key.
    ///
    /// The owner key should identify this value source to the host's
    /// [`crate::BindingHost`] implementation.
    #[must_use]
    pub const fn endpoint<K>(self, owner: K) -> PropertyEndpoint<K, T> {
        PropertyEndpoint::new(owner, value_source_property())
    }
}

impl<T> Copy for ValueSource<T> {}

impl<T> Clone for ValueSource<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> fmt::Debug for ValueSource<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValueSource")
            .field("id", &self.id)
            .field("value_type", &core::any::type_name::<T>())
            .finish()
    }
}

/// Typed handle for a pull-based external binding source.
///
/// External sources are read through a callback when the host drains bindings.
/// The caller remains responsible for marking the corresponding endpoint dirty
/// when the external value may have changed.
pub struct ExternalSource<T> {
    id: ExternalSourceId,
    _marker: PhantomData<fn() -> T>,
}

impl<T> ExternalSource<T> {
    const fn new(id: ExternalSourceId) -> Self {
        Self {
            id,
            _marker: PhantomData,
        }
    }

    /// Returns this source's stable id.
    #[must_use]
    pub const fn id(self) -> ExternalSourceId {
        self.id
    }

    /// Returns this source as a typed endpoint for the given host owner key.
    ///
    /// The owner key should identify this external source to the host's
    /// [`crate::BindingHost`] implementation.
    #[must_use]
    pub const fn endpoint<K>(self, owner: K) -> PropertyEndpoint<K, T> {
        PropertyEndpoint::new(owner, value_source_property())
    }
}

impl<T> Copy for ExternalSource<T> {}

impl<T> Clone for ExternalSource<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> fmt::Debug for ExternalSource<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExternalSource")
            .field("id", &self.id)
            .field("value_type", &core::any::type_name::<T>())
            .finish()
    }
}

/// Storage for single-value sources that feed a binding host.
///
/// This store is optional infrastructure for hosts that want local stored
/// values or pull-based external values to participate in the same binding
/// graph as property-store-backed endpoints. It does not own dirty tracking;
/// callers still mark the relevant endpoint dirty on their [`crate::BindingSet`].
#[derive(Default)]
pub struct ValueSourceStore {
    values: Vec<ValueEntry>,
    external_sources: Vec<ExternalEntry>,
}

impl ValueSourceStore {
    /// Creates an empty source store.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            values: Vec::new(),
            external_sources: Vec::new(),
        }
    }

    /// Stores `value` and returns a typed binding source handle.
    pub fn push_value<T>(&mut self, value: T) -> ValueSource<T>
    where
        T: Clone + 'static,
    {
        let id = ValueSourceId(self.values.len());
        self.values.push(ValueEntry {
            value: ErasedValue::new(value),
        });
        ValueSource::new(id)
    }

    /// Registers a pull-based external binding source.
    pub fn push_external<T, F>(&mut self, read: F) -> ExternalSource<T>
    where
        T: Clone + 'static,
        F: Fn() -> T + 'static,
    {
        let id = ExternalSourceId(self.external_sources.len());
        self.external_sources.push(ExternalEntry {
            read: Box::new(move || ErasedValue::new(read())),
        });
        ExternalSource::new(id)
    }

    /// Reads a stored source by typed handle.
    #[must_use]
    pub fn value<T>(&self, source: ValueSource<T>) -> Option<T>
    where
        T: Clone + 'static,
    {
        self.values
            .get(source.id.index())?
            .value
            .downcast_ref::<T>()
            .cloned()
    }

    /// Writes a stored source by typed handle.
    ///
    /// Returns `true` when the observable value changed.
    pub fn set_value<T>(&mut self, source: ValueSource<T>, value: T) -> bool
    where
        T: Clone + PartialEq + 'static,
    {
        let Some(entry) = self.values.get_mut(source.id.index()) else {
            return false;
        };
        if entry.value.downcast_ref::<T>() == Some(&value) {
            return false;
        }
        entry.value = ErasedValue::new(value);
        true
    }

    /// Reads a stored source as an erased endpoint value.
    #[must_use]
    pub fn value_erased(&self, id: ValueSourceId, property: PropertyId) -> Option<ErasedValue> {
        if property != VALUE_SOURCE_PROPERTY_ID {
            return None;
        }
        Some(self.values.get(id.index())?.value.clone())
    }

    /// Writes a stored source as an erased endpoint value.
    ///
    /// Returns `false` when `property` is not [`VALUE_SOURCE_PROPERTY_ID`] or
    /// `id` does not exist. Erased writes cannot compare values, so a valid
    /// write is reported as changed.
    pub fn set_value_erased(
        &mut self,
        id: ValueSourceId,
        property: PropertyId,
        value: ErasedValue,
    ) -> bool {
        if property != VALUE_SOURCE_PROPERTY_ID {
            return false;
        }
        let Some(entry) = self.values.get_mut(id.index()) else {
            return false;
        };
        entry.value = value;
        true
    }

    /// Reads an external source as an erased endpoint value.
    #[must_use]
    pub fn external_erased(
        &self,
        id: ExternalSourceId,
        property: PropertyId,
    ) -> Option<ErasedValue> {
        if property != VALUE_SOURCE_PROPERTY_ID {
            return None;
        }
        let entry = self.external_sources.get(id.index())?;
        Some((entry.read)())
    }
}

impl fmt::Debug for ValueSourceStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValueSourceStore")
            .field("values", &self.values.len())
            .field("external_sources", &self.external_sources.len())
            .finish()
    }
}

struct ValueEntry {
    value: ErasedValue,
}

struct ExternalEntry {
    read: Box<dyn Fn() -> ErasedValue>,
}

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::rc::Rc;
    use alloc::string::String;
    use core::cell::Cell;

    use super::*;
    use crate::{BindingHost, BindingSet, BindingWrite, EndpointKey};

    #[test]
    fn value_source_endpoint_uses_synthetic_property() {
        let mut store = ValueSourceStore::new();
        let source = store.push_value(12_u32);
        let endpoint = source.endpoint("source");

        assert_eq!(endpoint.owner(), "source");
        assert_eq!(endpoint.property().id(), VALUE_SOURCE_PROPERTY_ID);
    }

    #[test]
    fn stored_values_can_be_read_and_written() {
        let mut store = ValueSourceStore::new();
        let source = store.push_value(12_u32);

        assert_eq!(store.value(source), Some(12));
        assert!(!store.set_value(source, 12));
        assert!(store.set_value(source, 18));
        assert_eq!(store.value(source), Some(18));

        let erased = store
            .value_erased(source.id(), VALUE_SOURCE_PROPERTY_ID)
            .expect("source should have an erased value");
        assert_eq!(erased.downcast_ref::<u32>(), Some(&18));
    }

    #[test]
    fn erased_writes_reject_other_properties() {
        let mut store = ValueSourceStore::new();
        let source = store.push_value(12_u32);

        assert!(!store.set_value_erased(
            source.id(),
            PropertyId::new(42),
            ErasedValue::new(18_u32),
        ));
        assert_eq!(store.value(source), Some(12));

        assert!(store.set_value_erased(
            source.id(),
            VALUE_SOURCE_PROPERTY_ID,
            ErasedValue::new(18_u32),
        ));
        assert_eq!(store.value(source), Some(18));
    }

    #[test]
    fn external_sources_read_latest_value() {
        let value = Rc::new(Cell::new(4_u32));
        let source_value = Rc::clone(&value);
        let mut store = ValueSourceStore::new();
        let source = store.push_external(move || source_value.get());

        let first = store
            .external_erased(source.id(), VALUE_SOURCE_PROPERTY_ID)
            .expect("external source should read");
        assert_eq!(first.downcast_ref::<u32>(), Some(&4));

        value.set(9);
        let second = store
            .external_erased(source.id(), VALUE_SOURCE_PROPERTY_ID)
            .expect("external source should read");
        assert_eq!(second.downcast_ref::<u32>(), Some(&9));
    }

    #[test]
    fn stored_values_bind_through_normal_binding_set() {
        const BINDING: invalidation::Channel = invalidation::Channel::new(0);

        #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
        enum Owner {
            Source(ValueSourceId),
            Target,
        }

        struct Host {
            sources: ValueSourceStore,
            target: Option<ErasedValue>,
        }

        impl BindingHost<Owner> for Host {
            fn get_erased(&self, endpoint: EndpointKey<Owner>) -> Option<ErasedValue> {
                match endpoint.owner() {
                    Owner::Source(id) => self.sources.value_erased(id, endpoint.property()),
                    Owner::Target => self.target.clone(),
                }
            }

            fn set_erased(
                &mut self,
                endpoint: EndpointKey<Owner>,
                value: ErasedValue,
            ) -> BindingWrite {
                match endpoint.owner() {
                    Owner::Target => {
                        self.target = Some(value);
                        BindingWrite::changed(invalidation::ChannelSet::empty())
                    }
                    Owner::Source(_) => BindingWrite::unchanged(),
                }
            }
        }

        let mut host = Host {
            sources: ValueSourceStore::new(),
            target: None,
        };
        let source = host.sources.push_value(7_u32);
        let target = Property::<String>::from_id(PropertyId::new(4));
        let source_endpoint = source.endpoint(Owner::Source(source.id()));
        let target_endpoint = PropertyEndpoint::new(Owner::Target, target);

        let mut bindings = BindingSet::new(BINDING);
        bindings
            .bind_map(source_endpoint, target_endpoint, |value| {
                format_value(*value)
            })
            .unwrap();
        bindings.mark_source_changed(source_endpoint);

        let report = bindings.drain(&mut host).unwrap();
        assert_eq!(report.evaluated_bindings(), 1);
        assert_eq!(report.changed_bindings(), 1);
        assert_eq!(
            host.target
                .as_ref()
                .and_then(|value| value.downcast_ref::<String>())
                .map(String::as_str),
            Some("value: 7"),
        );
    }

    fn format_value(value: u32) -> String {
        format!("value: {value}")
    }
}
