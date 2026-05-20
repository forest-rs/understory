// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use understory_property::ErasedValue;

use crate::endpoint::{EndpointKey, PropertyEndpoint};
use crate::report::BindingWrite;

/// Object-safe host boundary used by binding evaluation.
///
/// Hosts decide how endpoint keys map to their property stores and how target
/// writes produce application invalidation channels.
pub trait BindingHost<K: Copy> {
    /// Reads an endpoint as an erased value.
    fn get_erased(&self, endpoint: EndpointKey<K>) -> Option<ErasedValue>;

    /// Writes an erased target value and reports whether it changed.
    fn set_erased(&mut self, endpoint: EndpointKey<K>, value: ErasedValue) -> BindingWrite;
}

/// Typed convenience methods for [`BindingHost`].
pub trait BindingHostExt<K: Copy>: BindingHost<K> {
    /// Reads and downcasts an endpoint value.
    #[must_use]
    fn get<T: Clone + 'static>(&self, endpoint: PropertyEndpoint<K, T>) -> Option<T> {
        self.get_erased(endpoint.key())
            .and_then(|value| value.downcast_ref::<T>().cloned())
    }

    /// Writes a typed target endpoint value.
    fn set<T: Clone + 'static>(
        &mut self,
        endpoint: PropertyEndpoint<K, T>,
        value: T,
    ) -> BindingWrite {
        self.set_erased(endpoint.key(), ErasedValue::new(value))
    }
}

impl<K, H> BindingHostExt<K> for H
where
    K: Copy,
    H: BindingHost<K> + ?Sized,
{
}
