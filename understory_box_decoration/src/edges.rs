// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::util::finite_non_negative;

/// Widths or offsets for the four edges of a rectangular box.
///
/// The field names follow CSS and Kurbo's usual y-down coordinate naming:
/// `top` is the smaller y edge and `bottom` is the larger y edge. `Edges<f64>`
/// is used for border widths, but the type is generic so callers can use the
/// same shape for edge-associated metadata such as per-side border styles.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Edges<T> {
    /// Value associated with the top edge.
    pub top: T,
    /// Value associated with the right edge.
    pub right: T,
    /// Value associated with the bottom edge.
    pub bottom: T,
    /// Value associated with the left edge.
    pub left: T,
}

impl<T: Copy> Edges<T> {
    /// Create edge values in top, right, bottom, left order.
    pub const fn new(top: T, right: T, bottom: T, left: T) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Use one value for every edge.
    pub const fn all(value: T) -> Self {
        Self::new(value, value, value, value)
    }

    /// Use one value for vertical edges and one for horizontal edges.
    ///
    /// This matches CSS two-value shorthand order: top/bottom first, then
    /// right/left.
    pub const fn vertical_horizontal(vertical: T, horizontal: T) -> Self {
        Self::new(vertical, horizontal, vertical, horizontal)
    }
}

impl Edges<f64> {
    /// Edge widths with every side set to zero.
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    /// Sum of left and right values.
    pub const fn horizontal(self) -> f64 {
        self.left + self.right
    }

    /// Sum of top and bottom values.
    pub const fn vertical(self) -> f64 {
        self.top + self.bottom
    }

    /// Return true when any edge has a positive finite value.
    pub const fn any_positive(self) -> bool {
        finite_non_negative(self.top) > 0.0
            || finite_non_negative(self.right) > 0.0
            || finite_non_negative(self.bottom) > 0.0
            || finite_non_negative(self.left) > 0.0
    }

    /// Clamp negative and non-finite values to zero.
    pub const fn clamped_non_negative(self) -> Self {
        Self::new(
            finite_non_negative(self.top),
            finite_non_negative(self.right),
            finite_non_negative(self.bottom),
            finite_non_negative(self.left),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_positive_ignores_non_finite_edges() {
        assert!(!Edges::new(f64::INFINITY, f64::NAN, -1.0, 0.0).any_positive());
        assert!(Edges::new(f64::INFINITY, f64::NAN, -1.0, 0.5).any_positive());
    }
}
