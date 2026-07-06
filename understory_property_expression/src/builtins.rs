// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Built-in expression functions.

use core::any::TypeId;

use color::{AlphaColor, ColorSpaceTag, DynamicColor, HueDirection, Srgb};
use understory_property::ErasedValue;

use crate::{ExprError, FunctionId, FunctionRegistry, FunctionSignature};

#[cfg(all(not(feature = "std"), not(feature = "libm")))]
compile_error!(
    "understory_property_expression requires either the `std` or `libm` feature for float math"
);

/// The color type used by built-in color functions.
pub type Color = AlphaColor<Srgb>;

/// Built-in numeric negation.
pub const NEG: FunctionId = FunctionId::new(0);
/// Built-in boolean negation.
pub const NOT: FunctionId = FunctionId::new(1);
/// Built-in numeric addition.
pub const ADD: FunctionId = FunctionId::new(2);
/// Built-in numeric subtraction.
pub const SUB: FunctionId = FunctionId::new(3);
/// Built-in numeric multiplication.
pub const MUL: FunctionId = FunctionId::new(4);
/// Built-in numeric division.
pub const DIV: FunctionId = FunctionId::new(5);
/// Built-in numeric less-than comparison.
pub const LT: FunctionId = FunctionId::new(6);
/// Built-in numeric less-than-or-equal comparison.
pub const LE: FunctionId = FunctionId::new(7);
/// Built-in numeric greater-than comparison.
pub const GT: FunctionId = FunctionId::new(8);
/// Built-in numeric greater-than-or-equal comparison.
pub const GE: FunctionId = FunctionId::new(9);
/// Built-in numeric equality comparison.
pub const EQ: FunctionId = FunctionId::new(10);
/// Built-in numeric inequality comparison.
pub const NE: FunctionId = FunctionId::new(11);
/// Built-in boolean conjunction.
pub const AND: FunctionId = FunctionId::new(12);
/// Built-in boolean disjunction.
pub const OR: FunctionId = FunctionId::new(13);
/// Built-in numeric minimum.
pub const MIN: FunctionId = FunctionId::new(14);
/// Built-in numeric maximum.
pub const MAX: FunctionId = FunctionId::new(15);
/// Built-in numeric clamp.
pub const CLAMP: FunctionId = FunctionId::new(16);
/// Built-in numeric absolute value.
pub const ABS: FunctionId = FunctionId::new(17);
/// Built-in numeric floor.
pub const FLOOR: FunctionId = FunctionId::new(18);
/// Built-in numeric ceiling.
pub const CEIL: FunctionId = FunctionId::new(19);
/// Built-in numeric round.
pub const ROUND: FunctionId = FunctionId::new(20);
/// Built-in color interpolation.
pub const MIX: FunctionId = FunctionId::new(21);
/// Built-in color alpha replacement.
pub const WITH_ALPHA: FunctionId = FunctionId::new(22);
/// Built-in color alpha multiplication.
pub const MULTIPLY_ALPHA: FunctionId = FunctionId::new(23);
/// Built-in color lightening.
pub const LIGHTEN: FunctionId = FunctionId::new(24);
/// Built-in color darkening.
pub const DARKEN: FunctionId = FunctionId::new(25);

pub(crate) fn signature(function: FunctionId) -> FunctionSignature {
    match function {
        NEG => FunctionSignature::of1::<f64, f64>(),
        NOT => FunctionSignature::of1::<bool, bool>(),
        ADD | SUB | MUL | DIV | MIN | MAX => FunctionSignature::of2::<f64, f64, f64>(),
        LT | LE | GT | GE | EQ | NE => FunctionSignature::of2::<f64, f64, bool>(),
        AND | OR => FunctionSignature::of2::<bool, bool, bool>(),
        CLAMP => FunctionSignature::of3::<f64, f64, f64, f64>(),
        ABS | FLOOR | CEIL | ROUND => FunctionSignature::of1::<f64, f64>(),
        MIX => FunctionSignature::of3::<Color, Color, f64, Color>(),
        WITH_ALPHA | MULTIPLY_ALPHA | LIGHTEN | DARKEN => {
            FunctionSignature::of2::<Color, f64, Color>()
        }
        _ => panic!("unknown built-in function id {function:?}"),
    }
}

/// Registers all built-in functions into `registry`.
///
/// # Panics
///
/// Panics if any built-in function id is already registered.
pub fn register_builtins(registry: &mut FunctionRegistry) {
    register_unary(registry, NEG, |value: f64| -value);
    register_unary(registry, NOT, |value: bool| !value);
    register_binary(registry, ADD, |a: f64, b: f64| a + b);
    register_binary(registry, SUB, |a: f64, b: f64| a - b);
    register_binary(registry, MUL, |a: f64, b: f64| a * b);
    register_binary(registry, DIV, |a: f64, b: f64| a / b);
    register_binary(registry, LT, |a: f64, b: f64| a < b);
    register_binary(registry, LE, |a: f64, b: f64| a <= b);
    register_binary(registry, GT, |a: f64, b: f64| a > b);
    register_binary(registry, GE, |a: f64, b: f64| a >= b);
    register_binary(registry, EQ, |a: f64, b: f64| a == b);
    register_binary(registry, NE, |a: f64, b: f64| a != b);
    register_binary(registry, AND, |a: bool, b: bool| a && b);
    register_binary(registry, OR, |a: bool, b: bool| a || b);
    register_binary(registry, MIN, numeric_min);
    register_binary(registry, MAX, numeric_max);
    register_ternary(registry, CLAMP, numeric_clamp);
    register_unary(registry, ABS, f64::abs);
    register_unary(registry, FLOOR, math::floor);
    register_unary(registry, CEIL, math::ceil);
    register_unary(registry, ROUND, math::round);
    register_ternary(registry, MIX, mix);
    register_binary(registry, WITH_ALPHA, |color: Color, alpha: f64| {
        color.with_alpha(f64_to_f32(alpha))
    });
    register_binary(registry, MULTIPLY_ALPHA, |color: Color, alpha: f64| {
        color.multiply_alpha(f64_to_f32(alpha))
    });
    register_binary(registry, LIGHTEN, |color: Color, amount: f64| {
        map_lightness(color, f64_to_f32(amount))
    });
    register_binary(registry, DARKEN, |color: Color, amount: f64| {
        map_lightness(color, -f64_to_f32(amount))
    });
}

fn register_unary<A, R, F>(registry: &mut FunctionRegistry, id: FunctionId, f: F)
where
    A: Clone + 'static,
    R: Clone + 'static,
    F: Fn(A) -> R + 'static,
{
    registry
        .register_erased(id, signature(id), move |args| {
            Ok(ErasedValue::new(f(arg(args, 0)?)))
        })
        .expect("built-in function ids must be unique");
}

fn register_binary<A, B, R, F>(registry: &mut FunctionRegistry, id: FunctionId, f: F)
where
    A: Clone + 'static,
    B: Clone + 'static,
    R: Clone + 'static,
    F: Fn(A, B) -> R + 'static,
{
    registry
        .register_erased(id, signature(id), move |args| {
            Ok(ErasedValue::new(f(arg(args, 0)?, arg(args, 1)?)))
        })
        .expect("built-in function ids must be unique");
}

fn register_ternary<A, B, C, R, F>(registry: &mut FunctionRegistry, id: FunctionId, f: F)
where
    A: Clone + 'static,
    B: Clone + 'static,
    C: Clone + 'static,
    R: Clone + 'static,
    F: Fn(A, B, C) -> R + 'static,
{
    registry
        .register_erased(id, signature(id), move |args| {
            Ok(ErasedValue::new(f(
                arg(args, 0)?,
                arg(args, 1)?,
                arg(args, 2)?,
            )))
        })
        .expect("built-in function ids must be unique");
}

fn arg<T: Clone + 'static>(args: &[ErasedValue], index: usize) -> Result<T, ExprError> {
    let value = &args[index];
    value
        .downcast_ref::<T>()
        .cloned()
        .ok_or_else(|| ExprError::TypeMismatch {
            expected: TypeId::of::<T>(),
            actual: value.type_id(),
        })
}

fn mix(first: Color, second: Color, t: f64) -> Color {
    DynamicColor::from_alpha_color(first)
        .interpolate(
            DynamicColor::from_alpha_color(second),
            ColorSpaceTag::Srgb,
            HueDirection::Shorter,
        )
        .eval(f64_to_f32(t))
        .to_alpha_color::<Srgb>()
}

fn numeric_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

fn numeric_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

fn numeric_clamp(value: f64, min: f64, max: f64) -> f64 {
    if value.is_nan() || min.is_nan() || max.is_nan() || min > max {
        f64::NAN
    } else {
        value.clamp(min, max)
    }
}

fn map_lightness(color: Color, amount: f32) -> Color {
    DynamicColor::from_alpha_color(color)
        .map_lightness(|lightness| (lightness + amount).clamp(0.0, 1.0))
        .to_alpha_color::<Srgb>()
}

fn f64_to_f32(value: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "expression color built-ins accept f64 scalars but color uses f32 components"
    )]
    {
        value as f32
    }
}

#[cfg(feature = "std")]
mod math {
    #[inline]
    pub(crate) fn floor(value: f64) -> f64 {
        value.floor()
    }

    #[inline]
    pub(crate) fn ceil(value: f64) -> f64 {
        value.ceil()
    }

    #[inline]
    pub(crate) fn round(value: f64) -> f64 {
        value.round()
    }
}

#[cfg(all(not(feature = "std"), feature = "libm"))]
mod math {
    #[inline]
    pub(crate) fn floor(value: f64) -> f64 {
        libm::floor(value)
    }

    #[inline]
    pub(crate) fn ceil(value: f64) -> f64 {
        libm::ceil(value)
    }

    #[inline]
    pub(crate) fn round(value: f64) -> f64 {
        libm::round(value)
    }
}
