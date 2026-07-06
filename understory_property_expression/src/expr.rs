// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Ergonomic expression constructors and built-in combinators.
//!
//! Prefer Rust operators for arithmetic and boolean negation:
//! `lhs + rhs`, `lhs - rhs`, `lhs * rhs`, `lhs / rhs`, `-value`, and `!value`.
//! Named helpers are reserved for operations that Rust operators cannot express
//! with an `Expr<bool>` or `Expr<Color>` result, such as comparisons,
//! clamping, and color functions.

use core::ops::{Add, Div, Mul, Neg, Not, Sub};

use understory_property::Property;

use crate::{Color, Expr, ExprResourceKey, FunctionId, builtins};

/// Creates an expression from a literal value.
#[must_use]
pub fn lit<T: Clone + 'static>(value: T) -> Expr<T> {
    Expr::literal(value)
}

/// Creates an expression that reads a dependency property.
#[must_use]
pub fn prop<T: Clone + 'static>(property: Property<T>) -> Expr<T> {
    Expr::property(property)
}

/// Creates an expression that reads a theme resource.
#[must_use]
pub fn token<T: Clone + 'static>(key: impl Into<ExprResourceKey>) -> Expr<T> {
    Expr::token(key.into())
}

/// Creates a lazy conditional expression.
#[must_use]
pub fn cond<T: Clone + 'static>(
    condition: Expr<bool>,
    then_expr: Expr<T>,
    else_expr: Expr<T>,
) -> Expr<T> {
    Expr::conditional(condition, then_expr, else_expr)
}

/// Numeric negation.
#[must_use]
fn neg(value: Expr<f64>) -> Expr<f64> {
    call1(builtins::NEG, value)
}

/// Numeric addition.
#[must_use]
fn add(lhs: Expr<f64>, rhs: Expr<f64>) -> Expr<f64> {
    call2(builtins::ADD, lhs, rhs)
}

/// Numeric subtraction.
#[must_use]
fn sub(lhs: Expr<f64>, rhs: Expr<f64>) -> Expr<f64> {
    call2(builtins::SUB, lhs, rhs)
}

/// Numeric multiplication.
#[must_use]
fn mul(lhs: Expr<f64>, rhs: Expr<f64>) -> Expr<f64> {
    call2(builtins::MUL, lhs, rhs)
}

/// Numeric division.
#[must_use]
fn div(lhs: Expr<f64>, rhs: Expr<f64>) -> Expr<f64> {
    call2(builtins::DIV, lhs, rhs)
}

/// Numeric minimum.
#[must_use]
pub fn min(lhs: Expr<f64>, rhs: Expr<f64>) -> Expr<f64> {
    call2(builtins::MIN, lhs, rhs)
}

/// Numeric maximum.
#[must_use]
pub fn max(lhs: Expr<f64>, rhs: Expr<f64>) -> Expr<f64> {
    call2(builtins::MAX, lhs, rhs)
}

/// Numeric clamp.
#[must_use]
pub fn clamp(value: Expr<f64>, min: Expr<f64>, max: Expr<f64>) -> Expr<f64> {
    call3(builtins::CLAMP, value, min, max)
}

/// Numeric absolute value.
#[must_use]
pub fn abs(value: Expr<f64>) -> Expr<f64> {
    call1(builtins::ABS, value)
}

/// Numeric floor.
#[must_use]
pub fn floor(value: Expr<f64>) -> Expr<f64> {
    call1(builtins::FLOOR, value)
}

/// Numeric ceiling.
#[must_use]
pub fn ceil(value: Expr<f64>) -> Expr<f64> {
    call1(builtins::CEIL, value)
}

/// Numeric round.
#[must_use]
pub fn round(value: Expr<f64>) -> Expr<f64> {
    call1(builtins::ROUND, value)
}

/// Numeric less-than comparison.
#[must_use]
pub fn lt(lhs: Expr<f64>, rhs: Expr<f64>) -> Expr<bool> {
    call2(builtins::LT, lhs, rhs)
}

/// Numeric less-than-or-equal comparison.
#[must_use]
pub fn le(lhs: Expr<f64>, rhs: Expr<f64>) -> Expr<bool> {
    call2(builtins::LE, lhs, rhs)
}

/// Numeric greater-than comparison.
#[must_use]
pub fn gt(lhs: Expr<f64>, rhs: Expr<f64>) -> Expr<bool> {
    call2(builtins::GT, lhs, rhs)
}

/// Numeric greater-than-or-equal comparison.
#[must_use]
pub fn ge(lhs: Expr<f64>, rhs: Expr<f64>) -> Expr<bool> {
    call2(builtins::GE, lhs, rhs)
}

/// Numeric equality comparison.
#[must_use]
pub fn eq(lhs: Expr<f64>, rhs: Expr<f64>) -> Expr<bool> {
    call2(builtins::EQ, lhs, rhs)
}

/// Numeric inequality comparison.
#[must_use]
pub fn ne(lhs: Expr<f64>, rhs: Expr<f64>) -> Expr<bool> {
    call2(builtins::NE, lhs, rhs)
}

/// Boolean conjunction.
#[must_use]
pub fn and(lhs: Expr<bool>, rhs: Expr<bool>) -> Expr<bool> {
    call2(builtins::AND, lhs, rhs)
}

/// Boolean disjunction.
#[must_use]
pub fn or(lhs: Expr<bool>, rhs: Expr<bool>) -> Expr<bool> {
    call2(builtins::OR, lhs, rhs)
}

/// Boolean negation.
#[must_use]
fn not(value: Expr<bool>) -> Expr<bool> {
    call1(builtins::NOT, value)
}

/// Color interpolation.
#[must_use]
pub fn mix(first: Expr<Color>, second: Expr<Color>, t: Expr<f64>) -> Expr<Color> {
    call3(builtins::MIX, first, second, t)
}

/// Replaces a color's alpha channel.
#[must_use]
pub fn with_alpha(color: Expr<Color>, alpha: Expr<f64>) -> Expr<Color> {
    call2(builtins::WITH_ALPHA, color, alpha)
}

/// Multiplies a color's alpha channel.
#[must_use]
pub fn multiply_alpha(color: Expr<Color>, alpha: Expr<f64>) -> Expr<Color> {
    call2(builtins::MULTIPLY_ALPHA, color, alpha)
}

/// Lightens a color by mapping lightness in Oklab/Oklch space.
#[must_use]
pub fn lighten(color: Expr<Color>, amount: Expr<f64>) -> Expr<Color> {
    call2(builtins::LIGHTEN, color, amount)
}

/// Darkens a color by mapping lightness in Oklab/Oklch space.
#[must_use]
pub fn darken(color: Expr<Color>, amount: Expr<f64>) -> Expr<Color> {
    call2(builtins::DARKEN, color, amount)
}

impl Add for Expr<f64> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        add(self, rhs)
    }
}

impl Add<f64> for Expr<f64> {
    type Output = Self;

    fn add(self, rhs: f64) -> Self::Output {
        add(self, lit(rhs))
    }
}

impl Sub for Expr<f64> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        sub(self, rhs)
    }
}

impl Sub<f64> for Expr<f64> {
    type Output = Self;

    fn sub(self, rhs: f64) -> Self::Output {
        sub(self, lit(rhs))
    }
}

impl Mul for Expr<f64> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        mul(self, rhs)
    }
}

impl Mul<f64> for Expr<f64> {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        mul(self, lit(rhs))
    }
}

impl Div for Expr<f64> {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        div(self, rhs)
    }
}

impl Div<f64> for Expr<f64> {
    type Output = Self;

    fn div(self, rhs: f64) -> Self::Output {
        div(self, lit(rhs))
    }
}

impl Neg for Expr<f64> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        neg(self)
    }
}

impl Not for Expr<bool> {
    type Output = Self;

    fn not(self) -> Self::Output {
        not(self)
    }
}

fn call1<A, R>(function: FunctionId, arg: Expr<A>) -> Expr<R>
where
    A: Clone + 'static,
    R: Clone + 'static,
{
    Expr::call_with_signature(
        function,
        builtins::signature(function),
        alloc::vec![arg.into_erased()],
    )
}

fn call2<A, B, R>(function: FunctionId, first: Expr<A>, second: Expr<B>) -> Expr<R>
where
    A: Clone + 'static,
    B: Clone + 'static,
    R: Clone + 'static,
{
    Expr::call_with_signature(
        function,
        builtins::signature(function),
        alloc::vec![first.into_erased(), second.into_erased()],
    )
}

fn call3<A, B, C, R>(
    function: FunctionId,
    first: Expr<A>,
    second: Expr<B>,
    third: Expr<C>,
) -> Expr<R>
where
    A: Clone + 'static,
    B: Clone + 'static,
    C: Clone + 'static,
    R: Clone + 'static,
{
    Expr::call_with_signature(
        function,
        builtins::signature(function),
        alloc::vec![
            first.into_erased(),
            second.into_erased(),
            third.into_erased()
        ],
    )
}

#[cfg(test)]
mod tests {
    use core::any::TypeId;

    use understory_property::{ErasedValue, PropertyId};

    use crate::{
        Color, Expr, ExprError, ExprEvalCx, ExprResourceKey, FunctionRegistry, FunctionSignature,
        builtins, expr,
    };

    struct TestCx {
        functions: FunctionRegistry,
    }

    impl TestCx {
        fn new() -> Self {
            Self {
                functions: FunctionRegistry::with_builtins(),
            }
        }
    }

    impl ExprEvalCx for TestCx {
        fn get_property(&mut self, property: PropertyId) -> Result<ErasedValue, ExprError> {
            Err(ExprError::MissingProperty { property })
        }

        fn get_resource(&mut self, resource: ExprResourceKey) -> Result<ErasedValue, ExprError> {
            Err(ExprError::MissingResource { key: resource })
        }

        fn functions(&self) -> &FunctionRegistry {
            &self.functions
        }
    }

    #[test]
    fn numeric_operators_evaluate_against_registered_builtins() {
        assert_eq!(eval(-expr::lit(3.0)), Ok(-3.0));
        assert_eq!(eval(expr::lit(1.0) + expr::lit(2.0)), Ok(3.0));
        assert_eq!(eval(expr::lit(5.0) - expr::lit(2.0)), Ok(3.0));
        assert_eq!(eval(expr::lit(4.0) * expr::lit(2.0)), Ok(8.0));
        assert_eq!(eval(expr::lit(8.0) / expr::lit(2.0)), Ok(4.0));
        assert_eq!(eval(expr::min(expr::lit(1.0), expr::lit(2.0))), Ok(1.0));
        assert_eq!(eval(expr::max(expr::lit(1.0), expr::lit(2.0))), Ok(2.0));
        assert_eq!(
            eval(expr::clamp(expr::lit(5.0), expr::lit(0.0), expr::lit(3.0))),
            Ok(3.0)
        );
        assert_eq!(eval(expr::abs(expr::lit(-3.0))), Ok(3.0));
        assert_eq!(eval(expr::floor(expr::lit(1.9))), Ok(1.0));
        assert_eq!(eval(expr::ceil(expr::lit(1.1))), Ok(2.0));
        assert_eq!(eval(expr::round(expr::lit(1.6))), Ok(2.0));
    }

    #[test]
    fn numeric_helpers_preserve_invalid_float_results() {
        assert!(
            eval(expr::min(expr::lit(f64::NAN), expr::lit(2.0)))
                .unwrap()
                .is_nan()
        );
        assert!(
            eval(expr::max(expr::lit(2.0), expr::lit(f64::NAN)))
                .unwrap()
                .is_nan()
        );
        assert!(
            eval(expr::clamp(
                expr::lit(f64::NAN),
                expr::lit(0.0),
                expr::lit(1.0)
            ))
            .unwrap()
            .is_nan()
        );
        assert!(
            eval(expr::clamp(expr::lit(0.5), expr::lit(1.0), expr::lit(0.0)))
                .unwrap()
                .is_nan()
        );
    }

    #[test]
    fn comparison_and_logic_helpers_evaluate_against_registered_builtins() {
        assert_eq!(eval(expr::lt(expr::lit(1.0), expr::lit(2.0))), Ok(true));
        assert_eq!(eval(expr::le(expr::lit(2.0), expr::lit(2.0))), Ok(true));
        assert_eq!(eval(expr::gt(expr::lit(3.0), expr::lit(2.0))), Ok(true));
        assert_eq!(eval(expr::ge(expr::lit(3.0), expr::lit(3.0))), Ok(true));
        assert_eq!(eval(expr::eq(expr::lit(3.0), expr::lit(3.0))), Ok(true));
        assert_eq!(eval(expr::ne(expr::lit(3.0), expr::lit(4.0))), Ok(true));
        assert_eq!(
            eval(expr::and(expr::lit(true), expr::lit(false))),
            Ok(false)
        );
        assert_eq!(eval(expr::or(expr::lit(true), expr::lit(false))), Ok(true));
        assert_eq!(eval(!expr::lit(true)), Ok(false));
    }

    #[test]
    fn color_helpers_evaluate_against_registered_builtins() {
        let first = Color::new([0.2, 0.4, 0.6, 0.8]);
        let second = Color::new([0.8, 0.6, 0.4, 0.2]);

        assert_components_close(
            eval(expr::mix(
                expr::lit(first),
                expr::lit(second),
                expr::lit(0.0),
            ))
            .unwrap()
            .components,
            first.components,
        );
        assert_components_close(
            eval(expr::with_alpha(expr::lit(first), expr::lit(0.25)))
                .unwrap()
                .components,
            [0.2, 0.4, 0.6, 0.25],
        );
        assert_components_close(
            eval(expr::multiply_alpha(expr::lit(first), expr::lit(0.5)))
                .unwrap()
                .components,
            [0.2, 0.4, 0.6, 0.4],
        );
        assert_alpha_unchanged(eval(expr::lighten(expr::lit(first), expr::lit(0.1))).unwrap());
        assert_alpha_unchanged(eval(expr::darken(expr::lit(first), expr::lit(0.1))).unwrap());
    }

    #[test]
    fn operators_lift_to_builtin_calls() {
        let expr = -(expr::lit(3.0) * 2.0 + expr::lit(4.0) / 2.0 - 1.0);
        let logic = !expr::lt(expr::lit(1.0), expr::lit(0.0));

        assert_eq!(eval(expr), Ok(-7.0));
        assert_eq!(eval(logic), Ok(true));
    }

    #[test]
    fn helper_signature_skew_is_reported_by_evaluation() {
        let expr = expr::lit(1.0) + expr::lit(2.0);
        let mut functions = FunctionRegistry::new();
        functions
            .register_erased(
                builtins::ADD,
                FunctionSignature::of2::<f64, f64, bool>(),
                |_| Ok(ErasedValue::new(true)),
            )
            .unwrap();
        let mut cx = TestCx { functions };

        assert_eq!(
            expr.eval(&mut cx),
            Err(ExprError::FunctionSignatureMismatch {
                function: builtins::ADD,
                expected: builtins::signature(builtins::ADD),
                actual: FunctionSignature::of2::<f64, f64, bool>(),
            })
        );
    }

    #[test]
    fn helper_result_type_is_static() {
        let expr = expr::lit(1.0) + expr::lit(2.0);

        assert_eq!(expr.as_erased().type_id(), TypeId::of::<f64>());
    }

    #[test]
    fn erased_expression_exposes_nodes_for_inspection() {
        let expr = expr::lit(1.0) + expr::lit(2.0);
        let erased = expr.as_erased();

        assert!(matches!(
            erased.nodes()[erased.root().index()],
            crate::ExprNode::Call {
                function: builtins::ADD,
                ..
            }
        ));
    }

    fn eval<T: Clone + 'static>(expr: Expr<T>) -> Result<T, ExprError> {
        let mut cx = TestCx::new();
        expr.eval(&mut cx)
    }

    fn assert_alpha_unchanged(color: Color) {
        assert!(
            (color.components[3] - 0.8).abs() < 1e-6,
            "expected alpha to remain 0.8, got {}",
            color.components[3]
        );
    }

    fn assert_components_close(actual: [f32; 4], expected: [f32; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() < 1e-6,
                "expected component {expected}, got {actual}"
            );
        }
    }
}
