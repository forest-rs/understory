// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::f64::consts::PI;

use kurbo::{Affine, Point, Vec2};

use crate::math::rem_euclid;
use crate::{AnimatableValue, Interpolate};

const TAU: f64 = PI * 2.0;

/// Decomposed two-dimensional transform channels.
///
/// This is an animation-friendly representation. Runtime composition can
/// animate translation, scale, rotation, skew, and origin independently before
/// lowering to a matrix for rendering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2d {
    /// Translation applied after transform-origin adjustment.
    pub translation: Vec2,
    /// Non-uniform scale factors.
    pub scale: Vec2,
    /// Rotation in radians.
    pub rotation: f64,
    /// X and Y skew factors.
    pub skew: Vec2,
    /// Local transform origin.
    pub origin: Point,
}

impl Transform2d {
    /// Identity transform.
    pub const IDENTITY: Self = Self {
        translation: Vec2::ZERO,
        scale: Vec2::new(1.0, 1.0),
        rotation: 0.0,
        skew: Vec2::ZERO,
        origin: Point::ZERO,
    };

    /// Creates an identity transform.
    #[must_use]
    pub const fn identity() -> Self {
        Self::IDENTITY
    }

    /// Creates a transform containing only `translation`.
    #[must_use]
    pub const fn translated(translation: Vec2) -> Self {
        Self {
            translation,
            ..Self::IDENTITY
        }
    }

    /// Creates a transform containing only non-uniform `scale`.
    #[must_use]
    pub const fn scaled(scale: Vec2) -> Self {
        Self {
            scale,
            ..Self::IDENTITY
        }
    }

    /// Creates a transform containing only `rotation` in radians.
    #[must_use]
    pub const fn rotated(rotation: f64) -> Self {
        Self {
            rotation,
            ..Self::IDENTITY
        }
    }

    /// Lowers this decomposed transform to a Kurbo affine matrix.
    #[must_use]
    pub fn to_affine(self) -> Affine {
        let origin = Vec2::new(self.origin.x, self.origin.y);
        Affine::translate(self.translation)
            * Affine::translate(origin)
            * Affine::rotate(self.rotation)
            * Affine::skew(self.skew.x, self.skew.y)
            * Affine::scale_non_uniform(self.scale.x, self.scale.y)
            * Affine::translate(-origin)
    }
}

impl Default for Transform2d {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Interpolate for Transform2d {
    fn interpolate(&self, to: &Self, progress: f64) -> Self {
        Self {
            translation: self.translation.interpolate(&to.translation, progress),
            scale: self.scale.interpolate(&to.scale, progress),
            rotation: self.rotation + (shortest_angle_delta(self.rotation, to.rotation) * progress),
            skew: self.skew.interpolate(&to.skew, progress),
            origin: Point::new(
                self.origin.x.interpolate(&to.origin.x, progress),
                self.origin.y.interpolate(&to.origin.y, progress),
            ),
        }
    }
}

impl AnimatableValue for Transform2d {
    type Velocity = Self;

    fn is_close(&self, other: &Self) -> bool {
        self.translation.is_close(&other.translation)
            && self.scale.is_close(&other.scale)
            && self.rotation.is_close(&other.rotation)
            && self.skew.is_close(&other.skew)
            && self.origin.x.is_close(&other.origin.x)
            && self.origin.y.is_close(&other.origin.y)
    }
}

fn shortest_angle_delta(from: f64, to: f64) -> f64 {
    rem_euclid(to - from + PI, TAU) - PI
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_interpolates_independent_channels() {
        let from = Transform2d::identity();
        let to = Transform2d {
            translation: Vec2::new(20.0, 40.0),
            scale: Vec2::new(2.0, 3.0),
            rotation: PI,
            skew: Vec2::new(0.2, 0.4),
            origin: Point::new(10.0, 12.0),
        };

        let mid = from.interpolate(&to, 0.5);

        assert_eq!(mid.translation, Vec2::new(10.0, 20.0));
        assert_eq!(mid.scale, Vec2::new(1.5, 2.0));
        assert_eq!(mid.skew, Vec2::new(0.1, 0.2));
        assert_eq!(mid.origin, Point::new(5.0, 6.0));
    }

    #[test]
    fn transform_rotation_uses_shortest_path() {
        let from = Transform2d::rotated(PI - 0.1);
        let to = Transform2d::rotated(-PI + 0.1);

        assert!(from.interpolate(&to, 0.5).rotation > PI - 0.1);
    }
}
