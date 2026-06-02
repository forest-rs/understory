// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use understory_property::ErasedValue;

use crate::endpoint::{EndpointKey, PropertyEndpoint};
use crate::report::BindingWrite;

/// Object-safe host boundary used by binding evaluation.
///
/// Hosts decide how endpoint keys map to their property stores, how binding
/// contexts affect target writes, and which application invalidation channels
/// a changed write dirties.
///
/// `C` is binding-owned write context. The binding set stores one context value
/// per binding and passes it back to the host on every target write. Hosts with
/// layered property stores can use this to choose a write layer, priority, or
/// provenance without maintaining a side table keyed by target endpoint. Hosts
/// that do not need per-binding metadata use the default `()` context.
///
/// For example, Overstory has multiple local value layers for the same
/// property. An app-authored binding should write as an explicit local value,
/// but a control-template binding should write into a lower-priority template
/// layer so the app can still override it. The UI can model that as:
///
/// ```rust,ignore
/// enum LocalValueSource {
///     Local,
///     TemplateBinding,
/// }
///
/// let mut bindings = BindingSet::<BindingOwner, LocalValueSource>::new(BINDING);
/// bindings.bind(app_model, button_text, LocalValueSource::Local)?;
/// bindings.bind(template_owner, template_part, LocalValueSource::TemplateBinding)?;
///
/// impl BindingHost<BindingOwner, LocalValueSource> for Ui {
///     fn set_erased(
///         &mut self,
///         endpoint: EndpointKey<BindingOwner>,
///         value: ErasedValue,
///         source: LocalValueSource,
///     ) -> BindingWrite {
///         self.set_local_erased_with_source(endpoint, value, source)
///     }
/// }
/// ```
///
/// The binding set does not know what `LocalValueSource` means; it only stores
/// the context with the binding and returns it to the host during evaluation.
pub trait BindingHost<K: Copy, C: Copy = ()> {
    /// Reads an endpoint as an erased value.
    fn get_erased(&self, endpoint: EndpointKey<K>) -> Option<ErasedValue>;

    /// Writes an erased target value with binding-owned write context and
    /// reports whether it changed.
    ///
    /// Hosts with layered property stores can use `context` to choose the
    /// target write layer. Hosts that do not need metadata use `()`.
    fn set_erased(
        &mut self,
        endpoint: EndpointKey<K>,
        value: ErasedValue,
        context: C,
    ) -> BindingWrite;
}

/// Typed convenience methods for [`BindingHost`].
pub trait BindingHostExt<K: Copy, C: Copy = ()>: BindingHost<K, C> {
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
        context: C,
    ) -> BindingWrite {
        self.set_erased(endpoint.key(), ErasedValue::new(value), context)
    }
}

impl<K, C, H> BindingHostExt<K, C> for H
where
    K: Copy,
    C: Copy,
    H: BindingHost<K, C> + ?Sized,
{
}
