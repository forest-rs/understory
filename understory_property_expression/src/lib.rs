// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Understory Property Expression: typed pure property expressions.
//!
//! This crate owns a small expression IR that can derive values from dependency
//! properties, theme resources, literals, conditionals, and registered
//! functions. It deliberately does not own property storage, style matching,
//! tree traversal, scheduling, writes, or parser vocabulary.
//!
//! Expressions are pull-based and side-effect-free. Hosts provide an
//! [`ExprEvalCx`] when evaluating so this crate can remain independent of any
//! concrete UI tree or theme type.
//! [`ErasedExpr::nodes`] exposes the compact expression arena for cold-path
//! inspection, diagnostics, and dependency tooling.
//!
//! In the presentation stack, this crate is a primitive: it owns expression
//! construction, dependency facts, defaults, and function registration.
//! `understory_style` owns the style-aware presentation policy that evaluates
//! those expressions during property resolution.
//!
//! Numeric expressions do not sanitize host values. Built-in numeric helpers
//! preserve invalid floating-point results as `NaN`; domain crates that require
//! finite scalars should reject non-finite values at their public boundaries.
//!
//! ## Canonical construction
//!
//! Build app-facing expressions through the [`expr`] module. Arithmetic and
//! boolean negation use Rust operators; named helpers are kept for operations
//! that operators cannot express, such as comparisons, clamping, and color
//! functions.
//!
//! ```rust
//! use understory_property::{PropertyMetadataBuilder, PropertyRegistry};
//! use understory_property_expression::{ExpressionLayer, expr};
//!
//! let mut registry = PropertyRegistry::new();
//! let scale = registry.register("Scale", PropertyMetadataBuilder::new(1.0_f64).build());
//! let padding = registry.register("Padding", PropertyMetadataBuilder::new(0.0_f64).build());
//!
//! let mut expressions = ExpressionLayer::new();
//! expressions.set_default(padding, expr::prop(scale) * 2.0 + 4.0);
//!
//! let deps = expressions.defaults().expression_deps(padding.id()).unwrap();
//! assert_eq!(deps.properties.as_slice(), &[scale.id()]);
//! assert_eq!(deps.resources.as_slice(), &[]);
//! ```

#![no_std]

extern crate alloc;

pub mod builtins;
mod defaults;
mod deps;
mod error;
mod function;
mod ids;
mod ir;
mod layer;
mod stack;

pub use builtins::{Color, register_builtins};
pub use defaults::{ExpressionDefaultRef, ExpressionDefaults};
pub use deps::ExprDeps;
pub use error::{ExprBuildError, ExprError, FunctionRegistrationError};
pub use function::{FunctionRegistry, FunctionSignature};
pub use ids::{ExprId, ExprResourceKey, FunctionId};
pub use ir::{ErasedExpr, Expr, ExprEvalCx, ExprNode};
pub use layer::ExpressionLayer;
pub use stack::InFlightProperties;

pub mod expr;

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;
    use core::any::TypeId;

    use understory_property::{ErasedValue, Property, PropertyId};

    use crate::{
        Color, ErasedExpr, Expr, ExprBuildError, ExprError, ExprEvalCx, ExprResourceKey,
        ExpressionDefaults, FunctionRegistry, FunctionSignature, InFlightProperties, builtins,
        expr,
    };

    const WIDTH: Property<f64> = Property::from_id(PropertyId::new(1));
    const HEIGHT: Property<f64> = Property::from_id(PropertyId::new(2));
    const NUMBER: ExprResourceKey = ExprResourceKey::new(4);
    const ACCENT: ExprResourceKey = ExprResourceKey::new(5);

    struct TestCx {
        functions: FunctionRegistry,
        properties: Vec<(PropertyId, ErasedValue)>,
        resources: Vec<(ExprResourceKey, ErasedValue)>,
    }

    impl TestCx {
        fn new() -> Self {
            Self {
                functions: FunctionRegistry::with_builtins(),
                properties: Vec::new(),
                resources: Vec::new(),
            }
        }

        fn with_functions(functions: FunctionRegistry) -> Self {
            Self {
                functions,
                properties: Vec::new(),
                resources: Vec::new(),
            }
        }

        fn set_property<T: Clone + 'static>(&mut self, property: Property<T>, value: T) {
            self.set_property_erased(property.id(), value);
        }

        fn set_property_erased<T: Clone + 'static>(&mut self, id: PropertyId, value: T) {
            let value = ErasedValue::new(value);
            match self.properties.binary_search_by_key(&id, |(id, _)| *id) {
                Ok(index) => self.properties[index].1 = value,
                Err(index) => self.properties.insert(index, (id, value)),
            }
        }

        fn set_resource<T: Clone + 'static>(&mut self, resource: ExprResourceKey, value: T) {
            let value = ErasedValue::new(value);
            match self
                .resources
                .binary_search_by_key(&resource, |(id, _)| *id)
            {
                Ok(index) => self.resources[index].1 = value,
                Err(index) => self.resources.insert(index, (resource, value)),
            }
        }
    }

    impl ExprEvalCx for TestCx {
        fn get_property(&mut self, property: PropertyId) -> Result<ErasedValue, ExprError> {
            self.properties
                .binary_search_by_key(&property, |(id, _)| *id)
                .map(|index| self.properties[index].1.clone())
                .map_err(|_| ExprError::MissingProperty { property })
        }

        fn get_resource(&mut self, resource: ExprResourceKey) -> Result<ErasedValue, ExprError> {
            self.resources
                .binary_search_by_key(&resource, |(id, _)| *id)
                .map(|index| self.resources[index].1.clone())
                .map_err(|_| ExprError::MissingResource { key: resource })
        }

        fn functions(&self) -> &FunctionRegistry {
            &self.functions
        }
    }

    #[test]
    fn evaluates_property_resource_and_call_dependencies() {
        let mut cx = TestCx::new();
        cx.set_property(WIDTH, 12.0);
        cx.set_resource(NUMBER, 30.0);

        let expr = Expr::<f64>::call2(
            &cx.functions,
            builtins::ADD,
            Expr::property(WIDTH),
            Expr::<f64>::token(NUMBER),
        )
        .unwrap();

        assert_eq!(expr.eval(&mut cx), Ok(42.0));
        assert_eq!(expr.deps().properties.as_slice(), &[WIDTH.id()]);
        assert_eq!(expr.deps().resources.as_slice(), &[NUMBER]);
    }

    #[test]
    fn conditional_is_lazy_but_dependencies_are_conservative() {
        let mut cx = TestCx::new();
        let expr = Expr::<f64>::conditional(
            Expr::literal(true),
            Expr::literal(7.0),
            Expr::property(HEIGHT),
        );

        assert_eq!(expr.eval(&mut cx), Ok(7.0));
        assert_eq!(expr.deps().properties.as_slice(), &[HEIGHT.id()]);
    }

    #[test]
    fn missing_reads_surface_host_errors() {
        let mut cx = TestCx::new();

        assert_eq!(
            Expr::<f64>::property(WIDTH).eval(&mut cx),
            Err(ExprError::MissingProperty {
                property: WIDTH.id()
            })
        );
        assert_eq!(
            Expr::<f64>::token(NUMBER).eval(&mut cx),
            Err(ExprError::MissingResource { key: NUMBER })
        );
    }

    #[test]
    fn call_builder_rejects_argument_and_return_type_mismatch() {
        let functions = FunctionRegistry::with_builtins();

        let argument_error =
            Expr::<f64>::call1(&functions, builtins::NEG, Expr::literal(true)).unwrap_err();
        assert_eq!(
            argument_error,
            ExprBuildError::ArgumentTypeMismatch {
                function: builtins::NEG,
                index: 0,
                expected: TypeId::of::<f64>(),
                actual: TypeId::of::<bool>(),
            }
        );

        let return_error = Expr::<bool>::call2(
            &functions,
            builtins::ADD,
            Expr::literal(1.0),
            Expr::literal(2.0),
        )
        .unwrap_err();
        assert_eq!(
            return_error,
            ExprBuildError::ReturnTypeMismatch {
                function: builtins::ADD,
                expected: TypeId::of::<bool>(),
                actual: TypeId::of::<f64>(),
            }
        );
    }

    #[test]
    fn evaluation_rejects_function_signature_drift() {
        let build_functions = FunctionRegistry::with_builtins();
        let expr = Expr::<f64>::call1(&build_functions, builtins::NEG, Expr::literal(3.0)).unwrap();

        let mut runtime_functions = FunctionRegistry::new();
        runtime_functions
            .register_erased(
                builtins::NEG,
                FunctionSignature::of1::<bool, bool>(),
                |args| {
                    let value = args[0].downcast_ref::<bool>().copied().unwrap();
                    Ok(ErasedValue::new(!value))
                },
            )
            .unwrap();
        let mut cx = TestCx::with_functions(runtime_functions);

        assert!(matches!(
            expr.eval(&mut cx),
            Err(ExprError::FunctionSignatureMismatch {
                function: builtins::NEG,
                ..
            })
        ));
    }

    #[test]
    fn expression_defaults_replace_by_property() {
        let mut defaults = ExpressionDefaults::new();
        let mut cx = TestCx::new();

        defaults.set(WIDTH, Expr::literal(10.0));
        defaults.set(WIDTH, Expr::literal(11.0));

        let default = defaults.get(WIDTH.id()).unwrap();
        assert_eq!(defaults.len(), 1);
        assert_eq!(eval_erased::<f64>(default, &mut cx), Ok(11.0));
    }

    #[test]
    fn expression_defaults_report_dependencies_by_property() {
        let mut defaults = ExpressionDefaults::new();

        defaults.set(
            WIDTH,
            expr::cond(
                expr::gt(expr::prop(HEIGHT), expr::token(NUMBER)),
                expr::prop(HEIGHT) * 2.0,
                expr::token(NUMBER) + 1.0,
            ),
        );

        let deps = defaults.expression_deps(WIDTH.id()).unwrap();
        assert_eq!(deps.properties.as_slice(), &[HEIGHT.id()]);
        assert_eq!(deps.resources.as_slice(), &[NUMBER]);
        assert!(defaults.expression_deps(HEIGHT.id()).is_none());
    }

    #[test]
    fn expression_default_entries_expose_targets_and_dependencies_in_property_order() {
        let mut defaults = ExpressionDefaults::new();

        defaults.set(HEIGHT, expr::token(ACCENT));
        defaults.set(WIDTH, expr::prop(HEIGHT) + expr::token(NUMBER));

        let entries = defaults.expression_entries().collect::<Vec<_>>();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].property(), WIDTH.id());
        assert_eq!(entries[0].deps().properties.as_slice(), &[HEIGHT.id()]);
        assert_eq!(entries[0].deps().resources.as_slice(), &[NUMBER]);
        assert_eq!(entries[1].property(), HEIGHT.id());
        assert_eq!(entries[1].deps().properties.as_slice(), &[]);
        assert_eq!(entries[1].deps().resources.as_slice(), &[ACCENT]);
        assert_eq!(entries[1].expression().type_id(), TypeId::of::<f64>());
    }

    #[test]
    fn in_flight_stack_reports_cycle_path() {
        let mut in_flight = InFlightProperties::new();

        in_flight.push(WIDTH.id()).unwrap();
        in_flight.push(HEIGHT.id()).unwrap();

        assert_eq!(
            in_flight.push(WIDTH.id()),
            Err(ExprError::Cycle {
                property: WIDTH.id(),
                stack: alloc::vec![WIDTH.id(), HEIGHT.id()],
            })
        );

        in_flight.pop();
        assert_eq!(in_flight.stack(), &[WIDTH.id()]);
    }

    #[test]
    fn built_in_numeric_and_color_functions_evaluate() {
        let mut cx = TestCx::new();
        let numeric = Expr::<f64>::call3(
            &cx.functions,
            builtins::CLAMP,
            Expr::literal(15.0),
            Expr::literal(0.0),
            Expr::literal(10.0),
        )
        .unwrap();
        assert_eq!(numeric.eval(&mut cx), Ok(10.0));

        let color = Color::new([0.2, 0.4, 0.6, 0.8]);
        cx.set_resource(ACCENT, color);
        let alpha = Expr::<Color>::call2(
            &cx.functions,
            builtins::WITH_ALPHA,
            Expr::<Color>::token(ACCENT),
            Expr::literal(0.25),
        )
        .unwrap()
        .eval(&mut cx)
        .unwrap();

        assert_eq!(alpha.components, [0.2, 0.4, 0.6, 0.25]);
    }

    #[test]
    fn property_expression_preserves_runtime_type_errors() {
        let mut cx = TestCx::new();
        cx.set_property_erased(WIDTH.id(), true);

        assert_eq!(
            Expr::<f64>::property(WIDTH).eval(&mut cx),
            Err(ExprError::TypeMismatch {
                expected: TypeId::of::<f64>(),
                actual: TypeId::of::<bool>(),
            })
        );
    }

    fn eval_erased<T: Clone + 'static>(
        expr: &ErasedExpr,
        cx: &mut dyn ExprEvalCx,
    ) -> Result<T, ExprError> {
        let value = expr.eval_erased(cx)?;
        value
            .downcast_ref::<T>()
            .cloned()
            .ok_or_else(|| ExprError::TypeMismatch {
                expected: TypeId::of::<T>(),
                actual: value.type_id(),
            })
    }
}
