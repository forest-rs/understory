// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{Point, Rect};

use crate::util::{point_to_rect_distance_squared, rect_from_points};
use crate::{OverlayId, OverlayStack};

const HIT_EPSILON: f64 = 1e-9;

/// Hover grace behavior used while deriving submenu-safe regions.
///
/// Hosts usually keep the default. The current frame builder uses this policy
/// internally when a pointer, parent menu, and child submenu geometry are all
/// available. [`GracePolicy::max_distance`] must be finite and non-negative;
/// debug builds assert this at frame-building ingress.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GracePolicy {
    /// Whether safe-travel regions should be produced.
    pub enabled: bool,
    /// Maximum pointer distance from either endpoint rectangle before no grace
    /// region is generated.
    pub max_distance: f64,
}

impl Default for GracePolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            max_distance: 320.0,
        }
    }
}

/// A triangular hover grace region.
///
/// You get these through [`GraceShape::Triangle`] in
/// [`GraceRegion::shape`](crate::GraceRegion::shape). Coordinates are in the
/// same scene space as overlay frame rectangles.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Triangle {
    /// First triangle point.
    pub a: Point,
    /// Second triangle point.
    pub b: Point,
    /// Third triangle point.
    pub c: Point,
}

impl Triangle {
    /// Creates a triangle from scene-space points.
    #[must_use]
    pub fn new(a: Point, b: Point, c: Point) -> Self {
        debug_assert!(a.is_finite(), "triangle point must be finite");
        debug_assert!(b.is_finite(), "triangle point must be finite");
        debug_assert!(c.is_finite(), "triangle point must be finite");
        Self { a, b, c }
    }

    pub(crate) fn bounds(self) -> Rect {
        rect_from_points(&[self.a, self.b, self.c])
    }
}

/// Shape for pointer-travel grace between related overlays.
///
/// You get this from [`GraceRegion::shape`](crate::GraceRegion::shape) or from
/// [`DismissRegion::grace`](crate::DismissRegion::grace). A renderer can also
/// visualize these shapes for diagnostics.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GraceShape {
    /// A triangular region, usually from the current pointer to a submenu edge.
    Triangle(Triangle),
    /// A quadrilateral region in scene coordinates.
    Quad([Point; 4]),
}

impl GraceShape {
    pub(crate) fn bounds(self) -> Rect {
        match self {
            Self::Triangle(triangle) => triangle.bounds(),
            Self::Quad(points) => rect_from_points(&points),
        }
    }
}

/// Derived pointer-travel region between a parent overlay and a child overlay.
///
/// You get these from [`OverlayFrame::grace_regions`](crate::OverlayFrame::grace_regions).
/// They let menu renderers keep a submenu open while the pointer travels from
/// the parent item into the child surface.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraceRegion {
    /// Parent overlay that owns the originating menu or hover surface.
    pub parent: OverlayId,
    /// Child overlay being protected while the pointer travels.
    pub child: OverlayId,
    /// Shape that counts as inside both overlays for dismissal purposes.
    pub shape: GraceShape,
}

/// Returns whether a point is inside a triangle.
///
/// Callers use this for diagnostics or custom event handling. The same test is
/// used by [`point_in_grace_shape`].
#[must_use]
pub fn point_in_triangle(point: Point, triangle: Triangle) -> bool {
    debug_assert!(point.is_finite(), "point must be finite");
    debug_assert!(triangle.a.is_finite(), "triangle point must be finite");
    debug_assert!(triangle.b.is_finite(), "triangle point must be finite");
    debug_assert!(triangle.c.is_finite(), "triangle point must be finite");

    let d1 = edge_sign(point, triangle.a, triangle.b);
    let d2 = edge_sign(point, triangle.b, triangle.c);
    let d3 = edge_sign(point, triangle.c, triangle.a);
    let has_negative = d1 < -HIT_EPSILON || d2 < -HIT_EPSILON || d3 < -HIT_EPSILON;
    let has_positive = d1 > HIT_EPSILON || d2 > HIT_EPSILON || d3 > HIT_EPSILON;
    !(has_negative && has_positive)
}

/// Returns whether a point is inside a grace shape.
///
/// This is the same containment test used by event resolution for
/// [`DismissRegion::grace`](crate::DismissRegion::grace).
#[must_use]
pub fn point_in_grace_shape(point: Point, shape: GraceShape) -> bool {
    debug_assert!(point.is_finite(), "point must be finite");
    match shape {
        GraceShape::Triangle(triangle) => point_in_triangle(point, triangle),
        GraceShape::Quad(points) => {
            let first = Triangle::new(points[0], points[1], points[2]);
            let second = Triangle::new(points[0], points[2], points[3]);
            point_in_triangle(point, first) || point_in_triangle(point, second)
        }
    }
}

pub(crate) fn grace_shape_between(pointer: Point, parent: Rect, child: Rect) -> GraceShape {
    let parent = parent.abs();
    let child = child.abs();
    let parent_center = parent.center();
    let child_center = child.center();
    let dx = child_center.x - parent_center.x;
    let dy = child_center.y - parent_center.y;
    let (b, c) = if dx.abs() >= dy.abs() {
        if dx >= 0.0 {
            (
                Point::new(child.x0, child.y0),
                Point::new(child.x0, child.y1),
            )
        } else {
            (
                Point::new(child.x1, child.y0),
                Point::new(child.x1, child.y1),
            )
        }
    } else if dy >= 0.0 {
        (
            Point::new(child.x0, child.y0),
            Point::new(child.x1, child.y0),
        )
    } else {
        (
            Point::new(child.x0, child.y1),
            Point::new(child.x1, child.y1),
        )
    };
    GraceShape::Triangle(Triangle::new(pointer, b, c))
}

pub(crate) fn should_generate_grace(
    stack: &OverlayStack,
    parent: OverlayId,
    child: OverlayId,
    pointer: Point,
    parent_rect: Rect,
    child_rect: Rect,
    policy: GracePolicy,
) -> bool {
    if !policy.enabled {
        return false;
    }
    let Some(parent_entry) = stack.entry(parent) else {
        return false;
    };
    let Some(child_entry) = stack.entry(child) else {
        return false;
    };
    if !parent_entry.behavior.supports_grace_to_child()
        && !child_entry.behavior.supports_grace_from_parent()
    {
        return false;
    }
    let max_distance_squared = policy.max_distance * policy.max_distance;
    point_to_rect_distance_squared(pointer, parent_rect) <= max_distance_squared
        || point_to_rect_distance_squared(pointer, child_rect) <= max_distance_squared
}

fn edge_sign(point: Point, a: Point, b: Point) -> f64 {
    (point.x - b.x) * (a.y - b.y) - (a.x - b.x) * (point.y - b.y)
}

#[cfg(test)]
mod tests {
    use super::{GraceShape, Triangle, point_in_grace_shape, point_in_triangle};
    use kurbo::Point;

    #[test]
    fn triangle_contains_points_on_edges() {
        let triangle = Triangle::new(
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(0.0, 10.0),
        );

        assert!(
            point_in_triangle(Point::new(1.0, 1.0), triangle),
            "interior point should be contained",
        );
        assert!(
            point_in_triangle(Point::new(5.0, 0.0), triangle),
            "edge point should be contained",
        );
        assert!(
            !point_in_triangle(Point::new(8.0, 8.0), triangle),
            "exterior point should not be contained",
        );
    }

    #[test]
    fn quad_is_split_into_two_triangles() {
        let shape = GraceShape::Quad([
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ]);

        assert!(
            point_in_grace_shape(Point::new(7.0, 7.0), shape),
            "quad interior should be contained",
        );
        assert!(
            !point_in_grace_shape(Point::new(11.0, 7.0), shape),
            "quad exterior should not be contained",
        );
    }
}
