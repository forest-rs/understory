// Copyright 2025 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// After you edit the crate's doc comment, run this command, then check README.md for any missing links
// cargo rdme --workspace-project=understory_property --heading-base-level=0

//! Understory Property: Core dependency property storage.
//!
//! This crate provides the foundation for a dependency property system,
//! handling local and animation value storage. Style resolution, theme
//! support, and full precedence handling are provided by `understory_style`.
//!
//! ## Core Concepts
//!
//! ### Property Storage
//!
//! [`PropertyStore`] holds Local and Animation values per-object:
//!
//! - **Local** — explicitly set values, layered by [`LocalValueSource`]
//! - **Animation** — temporary animation overrides (highest precedence)
//!
//! Inheritance is handled by [`DependencyObjectExt::get_inherited`].
//! Style and theme resolution are handled externally by `understory_style`.
//!
//! ### Layered Local Values
//!
//! The local layer is split into one sparse slot per [`LocalValueSource`]:
//! `Local`, `TemplateBinding`, `TemplateDefault`. A write goes to its source's
//! own slot; reads resolve the highest source that currently has a value.
//! Clearing the winning source therefore reveals whatever a lower source had
//! previously written. The
//! [`clear_local_by_source`](PropertyStore::clear_local_by_source) hook empties
//! exactly one source's slot — useful when a template tears down and should
//! drop only the values it installed. See [`LocalValueSource`] for the
//! precedence order.
//!
//! ### Key Operations
//!
//! - `set_local(property, value)` - set a local value (raw; no callbacks)
//! - `clear_local(property)` - clear the ordinary Local source slot
//! - `set_animation(property, value)` - set an animation value (raw)
//! - `get_effective_local(property, registry)` - Animation → Local → default
//! - `set_local_notifying(property, value, registry)` - validate, coerce, and
//!   set a local value; return dirty channels when the effective local value
//!   changes
//! - `try_set_local_notifying(property, value, registry)` - fallible variant
//!   that reports [`ValidationError`] instead of panicking on invalid values
//! - `set_local_erased_with_source_checked(id, value, source, registry)` -
//!   checked runtime/adapter entry point for a type-erased local write; reports
//!   [`ErasedWriteError`] on type mismatch or validation rejection
//! - `coerce_value_notifying(property, registry)` - re-run coercion after a
//!   coercion input changed (see below)
//! - `clear_local_erased_notifying(id, registry)` - clear a Local slot from
//!   type-erased binding/adaptor code and return conservative dirty channels
//!
//! ### Validation and Coercion
//!
//! [`PropertyMetadata`] carries two distinct per-property callbacks:
//!
//! - **Validation rejects.** [`PropertyMetadataBuilder::validate`] installs a
//!   `Fn(&T) -> bool` over the *raw* incoming value. A write whose value fails
//!   validation does not happen: the `try_*` paths return [`ValidationError`]
//!   (carrying the rejected value), the infallible notifying paths panic, and
//!   the prior stored value is left intact. The default value itself must
//!   pass validation — [`PropertyRegistry::register`] panics otherwise.
//! - **Coercion adjusts.** [`PropertyMetadataBuilder::coerce`] installs a
//!   context-free `Fn(T) -> T` for static adjustments.
//!   [`PropertyMetadataBuilder::coerce_with`] installs a
//!   `Fn(&CoerceContext<'_>, T) -> T` whose [`CoerceContext`] can read the
//!   owning object's *other* effective property values, enabling behavior such
//!   as `Slider.Value` clamping against the current `Minimum`/`Maximum`.
//!
//! Ordering is fixed: the raw value is validated first, then coerced, then
//! stored. Coercion output is trusted and is not re-validated.
//!
//! **When callbacks run.** Validation and coercion run **on write**, through
//! the checked paths only: the notifying setters on [`DependencyObjectExt`]
//! and the `try_set_local` / `try_set_local_with_source` / `try_set_animation` /
//! `set_local_erased_with_source_checked` methods on [`PropertyStore`]. Stored
//! values written through those APIs are therefore already validated and
//! coerced, and effective-value *reads* never run callbacks — reads stay cheap
//! and can return borrowed values. The raw setters (`set_local`,
//! `set_local_with_source`, `set_animation`, and
//! `set_local_erased_with_source`) are plumbing that bypasses both callbacks;
//! hosts and adapters that use them take responsibility for writing acceptable
//! values.
//!
//! ### Host Runtime Integration
//!
//! UI runtimes usually have two write shapes:
//!
//! - Typed application/control writes, which should go through
//!   [`DependencyObjectExt::set_local_notifying`] so they get validation,
//!   coercion, changed callbacks, and precise dirty-channel reporting.
//! - Type-erased adapter writes, such as binding targets, template defaults,
//!   style outputs, or deserialized updates, which usually carry
//!   [`PropertyId`] plus [`ErasedValue`]. These should go through
//!   [`PropertyStore::set_local_erased_with_source_notifying`] when the host
//!   needs invalidation channels, or
//!   [`PropertyStore::set_local_erased_with_source_checked`] when it only
//!   needs storage. Both checked erased paths enforce the same validation and
//!   coercion contract as typed checked writes, including
//!   [`PropertyMetadataBuilder::coerce_with`] callbacks that read sibling
//!   properties through [`CoerceContext`].
//!
//! ```rust
//! use invalidation::Channel;
//! use understory_property::{
//!     CoerceContext, ErasedValue, LocalValueSource, PropertyMetadataBuilder, PropertyRegistry,
//!     PropertyStore,
//! };
//!
//! const LAYOUT: Channel = Channel::new(0);
//!
//! let mut registry = PropertyRegistry::new();
//! let minimum = registry.register("Minimum", PropertyMetadataBuilder::new(0.0_f64).build());
//! let maximum = registry.register("Maximum", PropertyMetadataBuilder::new(100.0_f64).build());
//! let value = registry.register(
//!     "Value",
//!     PropertyMetadataBuilder::new(0.0_f64)
//!         .affects_channels(LAYOUT.into_set())
//!         .validate(|v| v.is_finite())
//!         .coerce_with(move |cx: &CoerceContext<'_>, v: f64| {
//!             v.max(cx.get(minimum)).min(cx.get(maximum))
//!         })
//!         .build(),
//! );
//!
//! let mut store = PropertyStore::<u32>::new(1);
//! store.try_set_local(minimum, 10.0, &registry).unwrap();
//! store.try_set_local(maximum, 20.0, &registry).unwrap();
//!
//! // Binding/template/runtime code often arrives erased. The checked erased
//! // path still rejects invalid values and coerces against sibling properties.
//! let channels = store
//!     .set_local_erased_with_source_notifying(
//!         value.id(),
//!         ErasedValue::new(30.0_f64),
//!         LocalValueSource::TemplateBinding,
//!         &registry,
//!     )
//!     .unwrap();
//!
//! assert_eq!(store.get_local(value), Some(&20.0));
//! assert!(channels.contains(LAYOUT));
//!
//! let err = store
//!     .set_local_erased_with_source_notifying(
//!         value.id(),
//!         ErasedValue::new(f64::NAN),
//!         LocalValueSource::TemplateBinding,
//!         &registry,
//!     )
//!     .unwrap_err();
//! assert_eq!(err.property(), value.id());
//! assert_eq!(store.get_local(value), Some(&20.0));
//! ```
//!
//! The raw setters remain useful inside a host for already-normalized values
//! or low-level storage work, but they are not the right boundary for user
//! input, binding write-back, style output, or other untrusted runtime data.
//!
//! **Re-coercion.** Because coercion runs at write time against the object's
//! state *at that moment*, a stored value can become stale when a coercion
//! input changes later (raise `Minimum` above the stored `Value` and the
//! stored `Value` is now out of range). This crate deliberately does not
//! track coercion dependencies — that is host policy. Instead the host calls
//! [`PropertyStore::coerce_value`] (or
//! [`DependencyObjectExt::coerce_value_notifying`], which also fires
//! `on_changed` and returns dirty channels) on each dependent property when
//! it knows an input changed:
//!
//! ```rust
//! use understory_property::{
//!     CoerceContext, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
//! };
//!
//! let mut registry = PropertyRegistry::new();
//! let minimum = registry.register("Minimum", PropertyMetadataBuilder::new(0.0_f64).build());
//! let maximum = registry.register("Maximum", PropertyMetadataBuilder::new(100.0_f64).build());
//! let value = registry.register(
//!     "Value",
//!     PropertyMetadataBuilder::new(0.0_f64)
//!         .validate(|v| v.is_finite())
//!         .coerce_with(move |cx: &CoerceContext<'_>, v: f64| {
//!             v.max(cx.get(minimum)).min(cx.get(maximum))
//!         })
//!         .build(),
//! );
//!
//! let mut store = PropertyStore::<u32>::new(1);
//! store.try_set_local(value, 30.0, &registry).unwrap();
//!
//! // Minimum moves above the stored Value. The host (a Slider) knows Value
//! // coerces against Minimum, so it re-coerces and invalidates on change.
//! store.set_local(minimum, 50.0);
//! assert!(store.coerce_value(value, &registry));
//! assert_eq!(store.get_effective_local(value, &registry), 50.0);
//! ```
//!
//! Note that re-coercion rewrites stored values in place: the pre-coercion
//! "desired" value is not retained, so lowering `Minimum` again does not
//! restore the original `Value`. Hosts that want restore-on-relax semantics
//! should keep desired values themselves.
//!
//! ## Invalidation Integration
//!
//! This crate does not own an invalidation graph or scheduler. Metadata stores
//! the [`invalidation::ChannelSet`] affected by each property, and notifying
//! helpers return that set when a write changes the effective local value.
//!
//! Treat the returned channels as dirty roots for your application-level
//! invalidation coordinator:
//!
//! - Use [`invalidation::InvalidationTracker::mark_with`] when property changes
//!   should follow graph dependencies, channel cascades, or cross-channel
//!   edges.
//! - Use [`invalidation::InvalidationTracker::mark`] only for deliberately
//!   local channels where direct marking is enough.
//!
//! ## Quick Start
//!
//! ```rust
//! use understory_property::{
//!     Property, PropertyMetadataBuilder, PropertyRegistry, PropertyStore,
//! };
//! use invalidation::Channel;
//!
//! const LAYOUT: Channel = Channel::new(0);
//!
//! // Create a registry and register properties
//! let mut registry = PropertyRegistry::new();
//! let width: Property<f64> = registry.register(
//!     "Width",
//!     PropertyMetadataBuilder::new(0.0_f64)
//!         .affects_channels(LAYOUT.into_set())
//!         .build()
//! );
//!
//! // Create a property store for an object
//! let mut store = PropertyStore::<u32>::new(1);
//!
//! // Set and get local values
//! store.set_local(width, 100.0);
//! assert_eq!(store.get_local(width), Some(&100.0));
//!
//! // Get effective value (Animation → Local → default)
//! let effective = store.get_effective_local(width, &registry);
//! assert_eq!(effective, 100.0);
//!
//! // Animation overrides local
//! store.set_animation(width, 200.0);
//! let effective = store.get_effective_local(width, &registry);
//! assert_eq!(effective, 200.0);
//! ```
//!
//! ## Memory Optimizations
//!
//! | Optimization | Description |
//! |--------------|-------------|
//! | **Sparse storage** | `PropertyStore` only allocates for non-default properties |
//! | **Shared defaults** | Default values stored in registry, not per-object |
//! | **Inline storage** | `SmallVec` for small property counts |
//! | **`PropertyId` as u16** | Compact property identification |
//!
//! ## Inheritance
//!
//! [`DependencyObjectExt::get_inherited`] provides inheritance resolution
//! by walking the parent chain. This is separate from style resolution.
//!
//! ## `no_std` Support
//!
//! This crate is `no_std` and uses `alloc`. It does not depend on `std`.

#![no_std]

extern crate alloc;

mod coerce;
mod id;
mod metadata;
mod object;
mod registry;
mod store;
mod value;

pub use coerce::{CoerceContext, CoerceRead};
pub use id::{Property, PropertyId};
pub use metadata::{
    CoerceValueCallback, ErasedValidationError, PropertyChangedCallback, PropertyMetadata,
    PropertyMetadataBuilder, ValidateValueCallback, ValidationError,
};
pub use object::{
    DependencyObject, DependencyObjectExt, ParentLookup, walk_inherited, walk_inherited_ref,
};
pub use registry::{PropertyRegistration, PropertyRegistry};
pub use store::{ErasedTypeMismatch, ErasedWriteError, LocalValueSource, PropertyStore};
pub use value::ErasedValue;
