<div align="center">

# Understory Property Binding

**Small one-way property binding primitives for Understory**

[![Latest published version.](https://img.shields.io/crates/v/understory_property_binding.svg)](https://crates.io/crates/understory_property_binding)
[![Documentation build status.](https://img.shields.io/docsrs/understory_property_binding.svg)](https://docs.rs/understory_property_binding)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/forest-rs/understory/ci.yml?logo=github&label=CI)](https://github.com/forest-rs/understory/actions)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=understory_property_binding --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Understory Property Binding: small one-way property binding primitives.

This crate provides deterministic one-way binding evaluation for
`understory_property` endpoints. It owns binding declarations, endpoint
indexes, dirty binding selection, dependency ordering, cycle checks, and
drain reports.

The intended first use is control-template glue: one property endpoint feeds
another property endpoint, and the host decides how those endpoints map onto
retained objects.

This crate deliberately does **not** own property storage, style or theme
resolution, opinion composition, widget trees, host scheduling, expression
parsing, two-way binding policy, or application invalidation graph ownership.

## Overview

Goal:
connect host-owned property endpoints with deterministic one-way
propagation.

Non-goals:
provide a property store, style engine, scheduler, expression language, or
application-level invalidation coordinator.

## Concepts and glossary

- [`PropertyEndpoint`]: typed `(owner, property)` endpoint used when a
  binding is declared.
- [`EndpointKey`]: type-erased endpoint key passed across the host boundary.
- [`BindingHost`]: host adapter that reads and writes erased endpoint values.
- [`BindingSet`]: registered bindings plus their dirty state and dependency
  graph.
- [`BindingWrite`]: host-reported result of writing a target endpoint.
- [`BindingReport`]: summary returned after dirty bindings are drained.
- [`BindingDrainError`]: drain error plus the partial report for writes that
  completed before the error.
- [`BindingStats`]: structural snapshot for diagnostics and integration
  tests.

## Evaluation model

A binding is one-way: source endpoint to target endpoint. The source and
target may have the same value type via [`BindingSet::bind`], or a mapped
value type via [`BindingSet::bind_map`].

Hosts mark source endpoints dirty with
[`BindingSet::mark_source_changed`] or
[`BindingSet::mark_endpoint_changed`]. [`BindingSet::drain`] evaluates dirty
bindings in dependency order. If a target write changes the observable
value, downstream bindings that read that target are marked dirty for a
later pass.

Direct self-bindings are rejected. Multiple active writers for one target
endpoint are rejected by default. Binding cycles are rejected at registration
time so draining remains deterministic.

Bindings can be removed with [`BindingSet::unbind`], or in groups with
[`BindingSet::clear_endpoint`] and [`BindingSet::clear_owner`]. Binding ids
remain stable and are not reused.

## Invalidation boundary

Bindings use an internal [`invalidation::InvalidationTracker`] keyed by
[`BindingId`]. This tracker is binding-local; it does not replace the
application's own invalidation graph.

[`BindingSet::drain`] returns the application channels reported by target
writes. The host remains responsible for marking its own
application-level invalidation tracker with those returned channels.

## Gotchas and risks

- Missing source values stop the drain with [`BindingError::MissingSource`].
- Runtime type mismatches stop the drain with
  [`BindingError::SourceTypeMismatch`].
- Drain errors return [`BindingDrainError`]. Writes that already happened
  are not rolled back, and the error's partial report still carries their
  affected channels. The failed binding and the rest of the current dirty
  batch remain dirty for a later retry.
- A binding runs only after its source endpoint has been marked dirty; adding
  a binding does not immediately copy the source value.
- The binding set stores closures for mapped bindings, so mapped evaluators
  should stay small and deterministic.

## Minimal example

```rust
use invalidation::{Channel, ChannelSet};
use understory_property_binding::{
    BindingHost, BindingSet, BindingWrite, EndpointKey, PropertyEndpoint,
};
use understory_property::{ErasedValue, PropertyMetadataBuilder, PropertyRegistry};

const BINDING: Channel = Channel::new(0);
const LAYOUT: Channel = Channel::new(1);

struct Host {
    source: ErasedValue,
    target: Option<ErasedValue>,
}

impl BindingHost<u32> for Host {
    fn get_erased(&self, endpoint: EndpointKey<u32>) -> Option<ErasedValue> {
        match endpoint.owner() {
            1 => Some(self.source.clone()),
            _ => None,
        }
    }

    fn set_erased(&mut self, endpoint: EndpointKey<u32>, value: ErasedValue) -> BindingWrite {
        if endpoint.owner() == 2 {
            self.target = Some(value);
            BindingWrite::new(true, LAYOUT.into_set())
        } else {
            BindingWrite::unchanged()
        }
    }
}

let mut registry = PropertyRegistry::new();
let width = registry.register("Width", PropertyMetadataBuilder::new(0_u32).build());

let mut bindings = BindingSet::new(BINDING);
bindings
    .bind(
        PropertyEndpoint::new(1, width),
        PropertyEndpoint::new(2, width),
    )
    .unwrap();

let mut host = Host {
    source: ErasedValue::new(42_u32),
    target: None,
};

bindings.mark_source_changed(PropertyEndpoint::new(1, width));
let report = bindings.drain(&mut host).unwrap();

assert_eq!(report.evaluated_bindings(), 1);
assert!(report.affected_channels().contains(LAYOUT));
assert_eq!(
    host.target.as_ref().and_then(ErasedValue::downcast_ref::<u32>),
    Some(&42),
);
```

<!-- cargo-rdme end -->

[`BindingError::MissingSource`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/enum.BindingError.html#variant.MissingSource
[`BindingError::SourceTypeMismatch`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/enum.BindingError.html#variant.SourceTypeMismatch
[`BindingDrainError`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.BindingDrainError.html
[`BindingHost`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/trait.BindingHost.html
[`BindingId`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.BindingId.html
[`BindingReport`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.BindingReport.html
[`BindingSet`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.BindingSet.html
[`BindingSet::bind`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.BindingSet.html#method.bind
[`BindingSet::bind_map`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.BindingSet.html#method.bind_map
[`BindingSet::clear_endpoint`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.BindingSet.html#method.clear_endpoint
[`BindingSet::clear_owner`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.BindingSet.html#method.clear_owner
[`BindingSet::drain`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.BindingSet.html#method.drain
[`BindingSet::mark_endpoint_changed`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.BindingSet.html#method.mark_endpoint_changed
[`BindingSet::mark_source_changed`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.BindingSet.html#method.mark_source_changed
[`BindingSet::unbind`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.BindingSet.html#method.unbind
[`BindingStats`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.BindingStats.html
[`BindingWrite`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.BindingWrite.html
[`EndpointKey`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.EndpointKey.html
[`invalidation::InvalidationTracker`]: https://docs.rs/invalidation/latest/invalidation/struct.InvalidationTracker.html
[`PropertyEndpoint`]: https://docs.rs/understory_property_binding/latest/understory_property_binding/struct.PropertyEndpoint.html

## Minimum supported Rust Version (MSRV)

This crate has been verified to compile with **Rust 1.88** and later.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE] or <http://www.apache.org/licenses/LICENSE-2.0>), or
- MIT license ([LICENSE-MIT] or <http://opensource.org/licenses/MIT>),

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you,
as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Contribution

Contributions are welcome by pull request. The [Rust code of conduct] applies.
Please feel free to add your name to the [AUTHORS] file in any substantive pull request.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you,
as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

[LICENSE-APACHE]: https://github.com/forest-rs/understory/blob/main/LICENSE-APACHE
[LICENSE-MIT]: https://github.com/forest-rs/understory/blob/main/LICENSE-MIT
[Rust code of conduct]: https://www.rust-lang.org/policies/code-of-conduct
[AUTHORS]: https://github.com/forest-rs/understory/blob/main/AUTHORS
