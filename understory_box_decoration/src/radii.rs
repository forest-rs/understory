// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{BezPath, Rect, RoundedRect, RoundedRectRadii, Size};

use crate::Edges;
use crate::path::rounded_rect_path;
use crate::util::{
    clean_size, distance, finite_non_negative, fit_scale, normalize_rect, scale_size,
};

const CIRCULAR_EPSILON: f64 = 1e-9;

/// Values for the four physical corners of a rectangular box.
///
/// Corner order follows CSS `border-radius`: top-left, top-right,
/// bottom-right, bottom-left. The type is generic so the same physical
/// ordering can carry radii, corner shapes, resolved corner geometry, or
/// property handles.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Corners<T> {
    /// Value associated with the top-left corner.
    pub top_left: T,
    /// Value associated with the top-right corner.
    pub top_right: T,
    /// Value associated with the bottom-right corner.
    pub bottom_right: T,
    /// Value associated with the bottom-left corner.
    pub bottom_left: T,
}

impl<T> Corners<T> {
    /// Create corner values in top-left, top-right, bottom-right,
    /// bottom-left order.
    pub const fn new(top_left: T, top_right: T, bottom_right: T, bottom_left: T) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }
}

impl<T: Copy> Corners<T> {
    /// Use one value for every corner.
    pub const fn all(value: T) -> Self {
        Self::new(value, value, value, value)
    }
}

/// Elliptical corner radii for a box contour.
///
/// Each corner is a [`Size`] where `width` is the horizontal radius and
/// `height` is the vertical radius. This is more general than Kurbo's
/// [`RoundedRectRadii`], which stores one circular radius per corner. Use
/// [`CornerRadii::as_kurbo_radii`] or [`CornerRadii::to_kurbo_rounded_rect`]
/// when the radii are circular and a renderer can use Kurbo's compact rounded
/// rectangle type directly.
pub type CornerRadii = Corners<Size>;

impl CornerRadii {
    /// Radii with every corner set to zero.
    pub const ZERO: Self = Self::new(Size::ZERO, Size::ZERO, Size::ZERO, Size::ZERO);

    /// Use the same circular radius for every corner.
    pub const fn uniform(radius: f64) -> Self {
        Self::all(Size::new(radius, radius))
    }

    /// Create circular radii for each corner.
    ///
    /// Arguments are in top-left, top-right, bottom-right, bottom-left order.
    pub const fn circular(
        top_left: f64,
        top_right: f64,
        bottom_right: f64,
        bottom_left: f64,
    ) -> Self {
        Self::new(
            Size::new(top_left, top_left),
            Size::new(top_right, top_right),
            Size::new(bottom_right, bottom_right),
            Size::new(bottom_left, bottom_left),
        )
    }

    /// Clamp negative and non-finite radius components to zero.
    pub const fn clamped_non_negative(self) -> Self {
        Self::new(
            clean_size(self.top_left),
            clean_size(self.top_right),
            clean_size(self.bottom_right),
            clean_size(self.bottom_left),
        )
    }

    /// Scale adjacent corner radii so they fit within `rect`.
    ///
    /// This follows the CSS border-radius conflict resolution rule: compute
    /// the ratio for every side whose two adjacent radii exceed the side
    /// length, then multiply all radii by the smallest ratio. Negative and
    /// non-finite radius components are treated as zero before scaling.
    pub fn scale_to_fit(self, rect: Rect) -> Self {
        let radii = self.clamped_non_negative();
        let rect = normalize_rect(rect);
        let width = finite_non_negative(rect.width());
        let height = finite_non_negative(rect.height());

        let mut scale = 1.0;
        scale = fit_scale(scale, width, radii.top_left.width + radii.top_right.width);
        scale = fit_scale(
            scale,
            height,
            radii.top_right.height + radii.bottom_right.height,
        );
        scale = fit_scale(
            scale,
            width,
            radii.bottom_left.width + radii.bottom_right.width,
        );
        scale = fit_scale(
            scale,
            height,
            radii.top_left.height + radii.bottom_left.height,
        );

        if scale < 1.0 {
            radii.scaled(scale)
        } else {
            radii
        }
    }

    /// Convert to Kurbo's circular per-corner radii when possible.
    ///
    /// Returns `None` for elliptical radii because [`RoundedRectRadii`] cannot
    /// represent them without losing information.
    pub fn as_kurbo_radii(self) -> Option<RoundedRectRadii> {
        let radii = self.clamped_non_negative();
        if is_circular(radii.top_left)
            && is_circular(radii.top_right)
            && is_circular(radii.bottom_right)
            && is_circular(radii.bottom_left)
        {
            Some(RoundedRectRadii::new(
                radii.top_left.width,
                radii.top_right.width,
                radii.bottom_right.width,
                radii.bottom_left.width,
            ))
        } else {
            None
        }
    }

    /// Convert to Kurbo's rounded rectangle type when the fitted radii are
    /// circular.
    ///
    /// This is useful for renderers that have a fast path for Kurbo
    /// [`RoundedRect`]. For elliptical radii, use [`CornerRadii::to_path`].
    pub fn to_kurbo_rounded_rect(self, rect: Rect) -> Option<RoundedRect> {
        let rect = normalize_rect(rect);
        self.scale_to_fit(rect)
            .as_kurbo_radii()
            .map(|radii| RoundedRect::from_rect(rect, radii))
    }

    /// Build a closed path for `rect` using the fitted elliptical radii.
    ///
    /// The path uses cubic Bezier quarter-ellipse approximations for rounded
    /// corners. Renderers that need exact circular arcs can use
    /// [`CornerRadii::to_kurbo_rounded_rect`] when it returns `Some`.
    pub fn to_path(self, rect: Rect) -> BezPath {
        let rect = normalize_rect(rect);
        let radii = self.scale_to_fit(rect);
        rounded_rect_path(rect, radii)
    }

    pub(crate) fn inset_by_edges(self, edges: Edges<f64>) -> Self {
        Self::new(
            Size::new(
                finite_non_negative(self.top_left.width - edges.left),
                finite_non_negative(self.top_left.height - edges.top),
            ),
            Size::new(
                finite_non_negative(self.top_right.width - edges.right),
                finite_non_negative(self.top_right.height - edges.top),
            ),
            Size::new(
                finite_non_negative(self.bottom_right.width - edges.right),
                finite_non_negative(self.bottom_right.height - edges.bottom),
            ),
            Size::new(
                finite_non_negative(self.bottom_left.width - edges.left),
                finite_non_negative(self.bottom_left.height - edges.bottom),
            ),
        )
    }

    fn scaled(self, scale: f64) -> Self {
        Self::new(
            scale_size(self.top_left, scale),
            scale_size(self.top_right, scale),
            scale_size(self.bottom_right, scale),
            scale_size(self.bottom_left, scale),
        )
    }
}

impl From<f64> for CornerRadii {
    fn from(radius: f64) -> Self {
        Self::uniform(radius)
    }
}

impl From<Size> for CornerRadii {
    fn from(radius: Size) -> Self {
        Self::all(radius)
    }
}

impl From<RoundedRectRadii> for CornerRadii {
    fn from(radii: RoundedRectRadii) -> Self {
        Self::circular(
            radii.top_left,
            radii.top_right,
            radii.bottom_right,
            radii.bottom_left,
        )
    }
}

fn is_circular(size: Size) -> bool {
    distance(size.width, size.height) <= CIRCULAR_EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{PathEl, Point};

    #[test]
    fn radii_scale_to_css_smallest_factor() {
        let radii = CornerRadii::new(
            Size::new(80.0, 10.0),
            Size::new(80.0, 20.0),
            Size::new(10.0, 20.0),
            Size::new(10.0, 10.0),
        );

        let scaled = radii.scale_to_fit(Rect::new(0.0, 0.0, 100.0, 50.0));

        assert_eq!(scaled.top_left, Size::new(50.0, 6.25));
        assert_eq!(scaled.top_right, Size::new(50.0, 12.5));
        assert_eq!(scaled.bottom_right, Size::new(6.25, 12.5));
        assert_eq!(scaled.bottom_left, Size::new(6.25, 6.25));
    }

    #[test]
    fn circular_radii_can_use_kurbo_rounded_rect() {
        let radii = CornerRadii::circular(4.0, 8.0, 12.0, 16.0);

        assert_eq!(
            radii.as_kurbo_radii(),
            Some(RoundedRectRadii::new(4.0, 8.0, 12.0, 16.0)),
        );
        assert!(
            radii
                .to_kurbo_rounded_rect(Rect::new(0.0, 0.0, 100.0, 50.0))
                .is_some()
        );
    }

    #[test]
    fn elliptical_radii_use_path() {
        let radii = CornerRadii::all(Size::new(12.0, 8.0));

        assert_eq!(radii.as_kurbo_radii(), None);

        let path = radii.to_path(Rect::new(0.0, 0.0, 100.0, 50.0));
        assert_eq!(
            path.iter().next(),
            Some(PathEl::MoveTo(Point::new(12.0, 0.0))),
        );
        assert_eq!(path.iter().last(), Some(PathEl::ClosePath));
    }
}
