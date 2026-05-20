// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// After you edit the crate's doc comment, run this command, then check README.md for any missing links
// cargo rdme --workspace-project=understory_property_binding --heading-base-level=0

//! Understory Property Binding: small one-way property binding primitives.
//!
//! This crate provides deterministic one-way binding evaluation for
//! `understory_property` endpoints. It owns binding declarations, endpoint
//! indexes, dirty binding selection, dependency ordering, cycle checks, and
//! drain reports.
//!
//! The intended first use is control-template glue: one property endpoint feeds
//! another property endpoint, and the host decides how those endpoints map onto
//! retained objects.
//!
//! This crate deliberately does **not** own property storage, style or theme
//! resolution, opinion composition, widget trees, host scheduling, expression
//! parsing, two-way binding policy, or application invalidation graph ownership.
//!
//! ## Overview
//!
//! Goal:
//! connect host-owned property endpoints with deterministic one-way
//! propagation.
//!
//! Non-goals:
//! provide a property store, style engine, scheduler, expression language, or
//! application-level invalidation coordinator.
//!
//! ## Concepts and glossary
//!
//! - [`PropertyEndpoint`]: typed `(owner, property)` endpoint used when a
//!   binding is declared.
//! - [`EndpointKey`]: type-erased endpoint key passed across the host boundary.
//! - [`BindingHost`]: host adapter that reads and writes erased endpoint values.
//! - [`BindingSet`]: registered bindings plus their dirty state and dependency
//!   graph.
//! - [`BindingWrite`]: host-reported result of writing a target endpoint.
//! - [`BindingReport`]: summary returned after dirty bindings are drained.
//! - [`BindingDrainError`]: drain error plus the partial report for writes that
//!   completed before the error.
//! - [`BindingStats`]: structural snapshot for diagnostics and integration
//!   tests.
//!
//! ## Evaluation model
//!
//! A binding is one-way: source endpoint to target endpoint. The source and
//! target may have the same value type via [`BindingSet::bind`], or a mapped
//! value type via [`BindingSet::bind_map`].
//!
//! Hosts mark source endpoints dirty with
//! [`BindingSet::mark_source_changed`] or
//! [`BindingSet::mark_endpoint_changed`]. [`BindingSet::drain`] evaluates dirty
//! bindings in dependency order. If a target write changes the observable
//! value, downstream bindings that read that target are marked dirty for a
//! later pass.
//!
//! Direct self-bindings are rejected. Multiple active writers for one target
//! endpoint are rejected by default. Binding cycles are rejected at registration
//! time so draining remains deterministic.
//!
//! Bindings can be removed with [`BindingSet::unbind`], or in groups with
//! [`BindingSet::clear_endpoint`] and [`BindingSet::clear_owner`]. Binding ids
//! remain stable and are not reused.
//!
//! ## Invalidation boundary
//!
//! Bindings use an internal [`invalidation::InvalidationTracker`] keyed by
//! [`BindingId`]. This tracker is binding-local; it does not replace the
//! application's own invalidation graph.
//!
//! [`BindingSet::drain`] returns the application channels reported by target
//! writes. The host remains responsible for marking its own
//! application-level invalidation tracker with those returned channels.
//!
//! ## Gotchas and risks
//!
//! - Missing source values stop the drain with [`BindingError::MissingSource`].
//! - Runtime type mismatches stop the drain with
//!   [`BindingError::SourceTypeMismatch`].
//! - Drain errors return [`BindingDrainError`]. Writes that already happened
//!   are not rolled back, and the error's partial report still carries their
//!   affected channels. The failed binding and the rest of the current dirty
//!   batch remain dirty for a later retry.
//! - A binding runs only after its source endpoint has been marked dirty; adding
//!   a binding does not immediately copy the source value.
//! - The binding set stores closures for mapped bindings, so mapped evaluators
//!   should stay small and deterministic.
//!
//! ## Minimal example
//!
//! ```rust
//! use invalidation::{Channel, ChannelSet};
//! use understory_property_binding::{
//!     BindingHost, BindingSet, BindingWrite, EndpointKey, PropertyEndpoint,
//! };
//! use understory_property::{ErasedValue, PropertyMetadataBuilder, PropertyRegistry};
//!
//! const BINDING: Channel = Channel::new(0);
//! const LAYOUT: Channel = Channel::new(1);
//!
//! struct Host {
//!     source: ErasedValue,
//!     target: Option<ErasedValue>,
//! }
//!
//! impl BindingHost<u32> for Host {
//!     fn get_erased(&self, endpoint: EndpointKey<u32>) -> Option<ErasedValue> {
//!         match endpoint.owner() {
//!             1 => Some(self.source.clone()),
//!             _ => None,
//!         }
//!     }
//!
//!     fn set_erased(&mut self, endpoint: EndpointKey<u32>, value: ErasedValue) -> BindingWrite {
//!         if endpoint.owner() == 2 {
//!             self.target = Some(value);
//!             BindingWrite::new(true, LAYOUT.into_set())
//!         } else {
//!             BindingWrite::unchanged()
//!         }
//!     }
//! }
//!
//! let mut registry = PropertyRegistry::new();
//! let width = registry.register("Width", PropertyMetadataBuilder::new(0_u32).build());
//!
//! let mut bindings = BindingSet::new(BINDING);
//! bindings
//!     .bind(
//!         PropertyEndpoint::new(1, width),
//!         PropertyEndpoint::new(2, width),
//!     )
//!     .unwrap();
//!
//! let mut host = Host {
//!     source: ErasedValue::new(42_u32),
//!     target: None,
//! };
//!
//! bindings.mark_source_changed(PropertyEndpoint::new(1, width));
//! let report = bindings.drain(&mut host).unwrap();
//!
//! assert_eq!(report.evaluated_bindings(), 1);
//! assert!(report.affected_channels().contains(LAYOUT));
//! assert_eq!(
//!     host.target.as_ref().and_then(ErasedValue::downcast_ref::<u32>),
//!     Some(&42),
//! );
//! ```

#![no_std]

extern crate alloc;

mod endpoint;
mod error;
mod host;
mod report;
mod set;

pub use endpoint::{BindingId, EndpointKey, PropertyEndpoint};
pub use error::{BindingDrainError, BindingError};
pub use host::{BindingHost, BindingHostExt};
pub use report::{BindingReport, BindingStats, BindingWrite};
pub use set::BindingSet;
