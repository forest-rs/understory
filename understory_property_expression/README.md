<div align="center">

# Understory Property Expression

**Typed pure expression IR for Understory dependency properties**

[![Latest published version.](https://img.shields.io/crates/v/understory_property_expression.svg)](https://crates.io/crates/understory_property_expression)
[![Documentation build status.](https://img.shields.io/docsrs/understory_property_expression.svg)](https://docs.rs/understory_property_expression)
[![Apache 2.0 or MIT license.](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](#license)
\
[![GitHub Actions CI status.](https://img.shields.io/github/actions/workflow/status/forest-rs/understory/ci.yml?logo=github&label=CI)](https://github.com/forest-rs/understory/actions)

</div>

<!-- We use cargo-rdme to update the README with the contents of lib.rs.
To edit the following section, update it in lib.rs, then run:
cargo rdme --workspace-project=understory_property_expression --heading-base-level=0
Full documentation at https://github.com/orium/cargo-rdme -->

<!-- Intra-doc links used in lib.rs may be evaluated here. -->

<!-- cargo-rdme start -->

Understory Property Expression: typed pure property expressions.

This crate owns a small expression IR that can derive values from dependency
properties, theme resources, literals, conditionals, and registered
functions. It deliberately does not own property storage, style matching,
tree traversal, scheduling, writes, or parser vocabulary.

Expressions are pull-based and side-effect-free. Hosts provide an
[`ExprEvalCx`] when evaluating so this crate can remain independent of any
concrete UI tree or theme type.
[`ErasedExpr::nodes`] exposes the compact expression arena for cold-path
inspection, diagnostics, and dependency tooling.

In the presentation stack, this crate is a primitive: it owns expression
construction, dependency facts, defaults, and function registration.
`understory_style` owns the style-aware presentation policy that evaluates
those expressions during property resolution.

Numeric expressions do not sanitize host values. Built-in numeric helpers
preserve invalid floating-point results as `NaN`; domain crates that require
finite scalars should reject non-finite values at their public boundaries.

## Canonical construction

Build app-facing expressions through the [`expr`] module. Arithmetic and
boolean negation use Rust operators; named helpers are kept for operations
that operators cannot express, such as comparisons, clamping, and color
functions.

```rust
use understory_property::{PropertyMetadataBuilder, PropertyRegistry};
use understory_property_expression::{ExpressionLayer, expr};

let mut registry = PropertyRegistry::new();
let scale = registry.register("Scale", PropertyMetadataBuilder::new(1.0_f64).build());
let padding = registry.register("Padding", PropertyMetadataBuilder::new(0.0_f64).build());

let mut expressions = ExpressionLayer::new();
expressions.set_default(padding, expr::prop(scale) * 2.0 + 4.0);

let deps = expressions.defaults().expression_deps(padding.id()).unwrap();
assert_eq!(deps.properties.as_slice(), &[scale.id()]);
assert_eq!(deps.resources.as_slice(), &[]);
```

<!-- cargo-rdme end -->

## Minimum supported Rust Version (MSRV)

This version of Understory has been verified to compile with **Rust 1.88** and
later.

## Community

Discussion of Understory development happens in the Linebender Zulip, at
<https://xi.zulipchat.com/>.

## License

Licensed under either of

- Apache License, Version 2.0
  ([LICENSE-APACHE](../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license
  ([LICENSE-MIT](../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
