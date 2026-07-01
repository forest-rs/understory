// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{BezPath, Point, Rect, RoundedRect, Size};

use crate::path::write_contour_path;
use crate::util::{clean_size, normalize_rect};
use crate::{CornerRadii, CornerShape, CornerShapes, Corners, Side};

/// One resolved corner of a box contour.
///
/// A corner combines the fitted radius area with the shape used inside that
/// area. `radii.width` is the horizontal radius and `radii.height` is the
/// vertical radius in the contour's local coordinate space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedCorner {
    /// Horizontal and vertical corner radii.
    pub radii: Size,
    /// Shape used inside the radius area.
    pub shape: CornerShape,
}

impl ResolvedCorner {
    /// A square, zero-radius corner.
    pub const ZERO: Self = Self::new(Size::ZERO, CornerShape::Round);

    /// Creates a resolved corner from radii and a shape.
    #[must_use]
    pub const fn new(radii: Size, shape: CornerShape) -> Self {
        Self {
            radii: clean_size(radii),
            shape,
        }
    }
}

impl Default for ResolvedCorner {
    fn default() -> Self {
        Self::ZERO
    }
}

/// A resolved physical contour for one box edge.
///
/// A contour is not a materialized path. It stores the rectangle plus the
/// resolved corner parameters needed to write paths for fills, clips, borders,
/// shadows, or hit regions on demand.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoxContour {
    /// Rectangular edge before corner shaping.
    pub rect: Rect,
    /// Resolved corner parameters in physical corner order.
    pub corners: Corners<ResolvedCorner>,
}

impl BoxContour {
    /// Creates a contour from already-fitted corner geometry.
    #[must_use]
    pub fn new(rect: Rect, corners: Corners<ResolvedCorner>) -> Self {
        Self {
            rect: normalize_rect(rect),
            corners,
        }
    }

    /// Creates a contour from fitted radii and physical corner shapes.
    #[must_use]
    pub fn from_radii(rect: Rect, radii: CornerRadii, shapes: CornerShapes) -> Self {
        Self::new(
            rect,
            Corners::new(
                ResolvedCorner::new(radii.top_left, shapes.top_left),
                ResolvedCorner::new(radii.top_right, shapes.top_right),
                ResolvedCorner::new(radii.bottom_right, shapes.bottom_right),
                ResolvedCorner::new(radii.bottom_left, shapes.bottom_left),
            ),
        )
    }

    /// Returns this contour's radii without shape information.
    #[must_use]
    pub const fn radii(self) -> CornerRadii {
        CornerRadii::new(
            self.corners.top_left.radii,
            self.corners.top_right.radii,
            self.corners.bottom_right.radii,
            self.corners.bottom_left.radii,
        )
    }

    /// Appends this contour as a closed path.
    ///
    /// The path is appended to `out`; callers that want only this contour
    /// should clear or create their [`BezPath`] before calling.
    pub fn write_path(self, out: &mut BezPath) {
        write_contour_path(self, out);
    }

    /// Builds a new closed path for this contour.
    ///
    /// Prefer [`BoxContour::write_path`] in hot paths so the caller can reuse
    /// path storage.
    #[must_use]
    pub fn to_path(self) -> BezPath {
        let mut path = BezPath::new();
        self.write_path(&mut path);
        path
    }

    /// Return a Kurbo rounded rectangle when every corner is round and
    /// circular.
    #[must_use]
    pub fn rounded_rect(self) -> Option<RoundedRect> {
        if self.corners.top_left.shape != CornerShape::Round
            || self.corners.top_right.shape != CornerShape::Round
            || self.corners.bottom_right.shape != CornerShape::Round
            || self.corners.bottom_left.shape != CornerShape::Round
        {
            return None;
        }

        self.radii().to_kurbo_rounded_rect(self.rect)
    }

    /// Returns the straight span on one side between adjacent corner areas.
    ///
    /// This is useful for side-oriented border lowering. It deliberately names
    /// only the central side span; corner transition regions and border-style
    /// paint lowering are separate future geometry.
    #[must_use]
    pub fn side_span(self, side: Side) -> ContourSideSpan {
        let rect = self.rect;
        let radii = self.radii();
        match side {
            Side::Top => ContourSideSpan::new(
                Point::new(rect.x0 + radii.top_left.width, rect.y0),
                Point::new(rect.x1 - radii.top_right.width, rect.y0),
            ),
            Side::Right => ContourSideSpan::new(
                Point::new(rect.x1, rect.y0 + radii.top_right.height),
                Point::new(rect.x1, rect.y1 - radii.bottom_right.height),
            ),
            Side::Bottom => ContourSideSpan::new(
                Point::new(rect.x1 - radii.bottom_right.width, rect.y1),
                Point::new(rect.x0 + radii.bottom_left.width, rect.y1),
            ),
            Side::Left => ContourSideSpan::new(
                Point::new(rect.x0, rect.y1 - radii.bottom_left.height),
                Point::new(rect.x0, rect.y0 + radii.top_left.height),
            ),
        }
    }
}

/// The straight portion of one side of a contour.
///
/// Spans follow clockwise path order: top left-to-right, right top-to-bottom,
/// bottom right-to-left, and left bottom-to-top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContourSideSpan {
    /// First point of the side span in clockwise contour order.
    pub start: Point,
    /// Last point of the side span in clockwise contour order.
    pub end: Point,
}

impl ContourSideSpan {
    /// Creates a side span from two points.
    #[must_use]
    pub const fn new(start: Point, end: Point) -> Self {
        Self { start, end }
    }
}
