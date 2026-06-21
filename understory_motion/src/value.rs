// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::Vec2;
use peniko::Color;

/// Value that can be interpolated between two endpoints.
pub trait Interpolate: Clone {
    /// Returns the value at normalized progress `progress` between `self` and
    /// `to`.
    ///
    /// Most callers pass values in the inclusive `0.0..=1.0` range, but easing
    /// and spring effects may intentionally overshoot.
    fn interpolate(&self, to: &Self, progress: f64) -> Self;
}

/// Value suitable for typed animation and effect-stack sampling.
pub trait AnimatableValue: Interpolate + PartialEq + 'static {
    /// Velocity representation used by physics and retargeting.
    type Velocity: Clone + PartialEq + 'static;

    /// Returns whether two values are close enough to snap instead of animate.
    fn is_close(&self, other: &Self) -> bool;

    /// Returns additive composition of `self` and `other`, when supported.
    #[must_use]
    fn add(&self, _other: &Self) -> Option<Self> {
        None
    }

    /// Returns accumulated composition of `self` with `other` repeated `count`
    /// times, when supported.
    #[must_use]
    fn accumulate(&self, other: &Self, count: u64) -> Option<Self> {
        let mut value = self.clone();
        for _ in 0..count {
            value = value.add(other)?;
        }
        Some(value)
    }
}

impl Interpolate for f32 {
    fn interpolate(&self, to: &Self, progress: f64) -> Self {
        self + ((to - self) * f32_progress(progress))
    }
}

impl AnimatableValue for f32 {
    type Velocity = Self;

    fn is_close(&self, other: &Self) -> bool {
        (self - other).abs() <= Self::EPSILON
    }

    fn add(&self, other: &Self) -> Option<Self> {
        Some(self + other)
    }

    fn accumulate(&self, other: &Self, count: u64) -> Option<Self> {
        Some(self + (other * count as Self))
    }
}

impl Interpolate for f64 {
    fn interpolate(&self, to: &Self, progress: f64) -> Self {
        self + ((to - self) * progress)
    }
}

impl AnimatableValue for f64 {
    type Velocity = Self;

    fn is_close(&self, other: &Self) -> bool {
        (self - other).abs() <= Self::EPSILON
    }

    fn add(&self, other: &Self) -> Option<Self> {
        Some(self + other)
    }

    fn accumulate(&self, other: &Self, count: u64) -> Option<Self> {
        Some(self + (other * count as Self))
    }
}

impl Interpolate for Vec2 {
    fn interpolate(&self, to: &Self, progress: f64) -> Self {
        *self + ((*to - *self) * progress)
    }
}

impl AnimatableValue for Vec2 {
    type Velocity = Self;

    fn is_close(&self, other: &Self) -> bool {
        (self.x - other.x).abs() <= f64::EPSILON && (self.y - other.y).abs() <= f64::EPSILON
    }

    fn add(&self, other: &Self) -> Option<Self> {
        Some(*self + *other)
    }

    fn accumulate(&self, other: &Self, count: u64) -> Option<Self> {
        Some(*self + (*other * count as f64))
    }
}

impl Interpolate for Color {
    fn interpolate(&self, to: &Self, progress: f64) -> Self {
        crate::ColorInterpolation::default().interpolate(*self, *to, progress)
    }
}

impl AnimatableValue for Color {
    type Velocity = ();

    fn is_close(&self, other: &Self) -> bool {
        self == other
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "f32-valued properties require f32 progress"
)]
pub(crate) fn f32_progress(progress: f64) -> f32 {
    progress as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_values_interpolate_and_add() {
        assert_eq!(0.0_f64.interpolate(&10.0, 0.25), 2.5);
        assert_eq!(2.0_f64.add(&3.0), Some(5.0));
        assert_eq!(2.0_f64.accumulate(&3.0, 3), Some(11.0));
    }

    #[test]
    fn vec2_values_interpolate_and_add() {
        let from = Vec2::new(2.0, 4.0);
        let to = Vec2::new(6.0, 12.0);

        assert_eq!(from.interpolate(&to, 0.5), Vec2::new(4.0, 8.0));
        assert_eq!(from.add(&Vec2::new(1.0, -1.0)), Some(Vec2::new(3.0, 3.0)));
    }
}
