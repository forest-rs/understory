// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::Corners;

/// Resolved physical corner shapes for a box contour.
pub type CornerShapes = Corners<CornerShape>;

impl CornerShapes {
    /// Round corner shapes for every corner.
    pub const ROUND: Self = Self::all(CornerShape::Round);
}

/// The shape used inside a corner's resolved radius area.
///
/// The enum is intentionally smaller than the authored CSS keyword set.
/// CSS `corner-shape` keywords such as `scoop`, `notch`, and `squircle` are
/// represented as superellipse parameters here; preserving the exact authored
/// keyword belongs in a property or parser layer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CornerShape {
    /// A quarter ellipse, equivalent to CSS `round` / `superellipse(1)`.
    #[default]
    Round,
    /// A convex square corner, equivalent to CSS `square` /
    /// `superellipse(infinity)`.
    Square,
    /// A diagonal corner, equivalent to CSS `bevel` / `superellipse(0)`.
    Bevel,
    /// A CSS-style superellipse corner parameter.
    Superellipse(Superellipse),
}

impl CornerShape {
    /// A concave quarter-ellipse scoop, equivalent to CSS `scoop` /
    /// `superellipse(-1)`.
    #[must_use]
    pub const fn scoop() -> Self {
        Self::Superellipse(Superellipse::new(-1.0))
    }

    /// A concave square notch, equivalent to CSS `notch` /
    /// `superellipse(-infinity)`.
    #[must_use]
    pub const fn notch() -> Self {
        Self::Superellipse(Superellipse::new(f64::NEG_INFINITY))
    }

    /// A convex curve between round and square, equivalent to CSS `squircle` /
    /// `superellipse(2)`.
    #[must_use]
    pub const fn squircle() -> Self {
        Self::Superellipse(Superellipse::new(2.0))
    }

    /// Creates a corner shape from a CSS-style superellipse parameter.
    ///
    /// `superellipse(1)` is represented as [`CornerShape::Round`],
    /// `superellipse(0)` as [`CornerShape::Bevel`], and positive infinity as
    /// [`CornerShape::Square`]. Negative infinity is retained as a
    /// superellipse because it represents a concave notch rather than a square
    /// outer corner.
    #[must_use]
    pub const fn superellipse(parameter: f64) -> Self {
        if parameter == 1.0 || parameter.is_nan() {
            Self::Round
        } else if parameter == 0.0 {
            Self::Bevel
        } else if parameter == f64::INFINITY {
            Self::Square
        } else {
            Self::Superellipse(Superellipse::new(parameter))
        }
    }
}

/// CSS-style superellipse parameter for a shaped corner.
///
/// CSS Borders and Box Decorations Level 4 defines `superellipse(K)` using an
/// exponent of `2^K`. This type stores the resolved `K` value. NaN is coerced
/// to `1.0`, matching the normal rounded-corner shape; infinities are kept so
/// callers can represent square and notch limits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Superellipse {
    parameter: f64,
}

impl Superellipse {
    /// The parameter for a round quarter ellipse.
    ///
    /// [`CornerShape::superellipse`] canonicalizes this value to
    /// [`CornerShape::Round`], but renderers also treat an explicitly-stored
    /// `CornerShape::Superellipse(Superellipse::ROUND)` as a round corner.
    pub const ROUND: Self = Self::new(1.0);

    /// The parameter for a diagonal bevel.
    pub const BEVEL: Self = Self::new(0.0);

    /// The parameter for a concave scoop.
    pub const SCOOP: Self = Self::new(-1.0);

    /// The parameter for a squircle.
    pub const SQUIRCLE: Self = Self::new(2.0);

    /// The parameter for a square limit.
    pub const SQUARE: Self = Self::new(f64::INFINITY);

    /// The parameter for a notch limit.
    pub const NOTCH: Self = Self::new(f64::NEG_INFINITY);

    /// Creates a CSS-style superellipse parameter.
    #[must_use]
    pub const fn new(parameter: f64) -> Self {
        if parameter.is_nan() {
            Self { parameter: 1.0 }
        } else {
            Self { parameter }
        }
    }

    /// Returns the stored CSS-style `K` parameter.
    #[must_use]
    pub const fn parameter(self) -> f64 {
        self.parameter
    }
}
