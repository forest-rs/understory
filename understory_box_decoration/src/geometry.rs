// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{BezPath, Point, Rect, Size};

use crate::util::{finite_non_negative, normalize_rect};
use crate::{
    BorderStyle, BoxArea, BoxContour, CornerRadii, CornerShape, CornerShapes, Edges, Side,
};

/// Fully resolved geometry for a single box decoration.
///
/// The geometry stores contours and scalar parameters, not materialized paths.
/// Use writer methods such as
/// [`BoxDecorationGeometry::write_border_ring_path`] and
/// [`BoxDecorationGeometry::write_background_clip`] when a renderer or hit
/// tester needs a concrete [`BezPath`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxDecorationGeometry {
    /// Outer border edge in local coordinates.
    pub border_box: Rect,
    /// Padding edge in local coordinates.
    pub padding_box: Rect,
    /// Content edge in local coordinates.
    pub content_box: Rect,
    /// Non-negative border widths for each side.
    pub border_widths: Edges<f64>,
    /// Border style for each side.
    ///
    /// The style is stored for renderer lowering. This geometry crate treats
    /// [`BorderStyle::None`] and [`BorderStyle::Hidden`] as non-painting but
    /// does not implement the paint algorithms for the visible styles.
    pub border_styles: Edges<BorderStyle>,
    /// Non-negative padding widths for each side.
    pub padding_widths: Edges<f64>,
    /// Resolved contour for the border edge.
    pub border_edge: BoxContour,
    /// Resolved contour for the padding edge.
    pub padding_edge: BoxContour,
    /// Resolved contour for the content edge.
    pub content_edge: BoxContour,
}

impl BoxDecorationGeometry {
    /// Resolve decoration geometry from a border box, edge widths, corner
    /// radii, and corner shapes.
    ///
    /// This convenience constructor records every border side as
    /// [`BorderStyle::Solid`]. UI presentation layers with resolved
    /// `border-style` values should use
    /// [`BoxDecorationGeometry::from_styled_border_box`] instead.
    ///
    /// This is the common entry point for geometry-only callers: layout
    /// supplies a border box, callers supply resolved physical border/padding
    /// widths plus corner parameters, and this crate derives the contours a
    /// renderer needs to paint fills, clips, shadows, and borders.
    pub fn from_border_box(
        border_box: Rect,
        border_widths: Edges<f64>,
        padding_widths: Edges<f64>,
        requested_radii: CornerRadii,
        corner_shapes: CornerShapes,
    ) -> Self {
        Self::from_styled_border_box(
            border_box,
            border_widths,
            Edges::all(BorderStyle::Solid),
            padding_widths,
            requested_radii,
            corner_shapes,
        )
    }

    /// Resolve decoration geometry with per-side border styles.
    ///
    /// This is the entry point for callers that have resolved CSS
    /// `border-style` values. The styles are stored with the geometry and
    /// copied into [`BorderSideGeometry`]; renderer/backend code remains
    /// responsible for lowering each style into drawing commands.
    pub fn from_styled_border_box(
        border_box: Rect,
        border_widths: Edges<f64>,
        border_styles: Edges<BorderStyle>,
        padding_widths: Edges<f64>,
        requested_radii: CornerRadii,
        corner_shapes: CornerShapes,
    ) -> Self {
        let border_box = normalize_rect(border_box);
        let border_widths =
            visible_border_widths(border_widths.clamped_non_negative(), border_styles);
        let padding_widths = padding_widths.clamped_non_negative();
        let border_radii = requested_radii.scale_to_fit(border_box);
        let padding_box = inset_rect(border_box, border_widths);
        let padding_radii = inset_radii_for_shapes(border_radii, border_widths, corner_shapes)
            .scale_to_fit(padding_box);
        let content_box = inset_rect(padding_box, padding_widths);
        let content_radii = inset_radii_for_shapes(padding_radii, padding_widths, corner_shapes)
            .scale_to_fit(content_box);

        let border_edge = BoxContour::from_radii(border_box, border_radii, corner_shapes);
        let padding_edge = BoxContour::from_radii(padding_box, padding_radii, corner_shapes);
        let content_edge = BoxContour::from_radii(content_box, content_radii, corner_shapes);

        Self {
            border_box,
            padding_box,
            content_box,
            border_widths,
            border_styles,
            padding_widths,
            border_edge,
            padding_edge,
            content_edge,
        }
    }

    /// Resolve decoration geometry with round corners.
    #[must_use]
    pub fn from_round_border_box(
        border_box: Rect,
        border_widths: Edges<f64>,
        padding_widths: Edges<f64>,
        requested_radii: CornerRadii,
    ) -> Self {
        Self::from_styled_border_box(
            border_box,
            border_widths,
            Edges::all(BorderStyle::Solid),
            padding_widths,
            requested_radii,
            CornerShapes::ROUND,
        )
    }

    /// Return true when the border has any positive width.
    pub const fn has_border_width(self) -> bool {
        self.border_widths.any_positive()
    }

    /// Return true when the border has any positive-width side with a painting
    /// style.
    pub const fn has_visible_border(self) -> bool {
        (self.border_styles.top.paints_border() && self.border_widths.top > 0.0)
            || (self.border_styles.right.paints_border() && self.border_widths.right > 0.0)
            || (self.border_styles.bottom.paints_border() && self.border_widths.bottom > 0.0)
            || (self.border_styles.left.paints_border() && self.border_widths.left > 0.0)
    }

    /// Returns the resolved contour for a CSS box area.
    #[must_use]
    pub const fn contour(self, area: BoxArea) -> BoxContour {
        match area {
            BoxArea::Border => self.border_edge,
            BoxArea::Padding => self.padding_edge,
            BoxArea::Content => self.content_edge,
        }
    }

    /// Appends a compound path for the border ring.
    ///
    /// This writes the outer border contour and inner padding contour as two
    /// closed subpaths with the same winding direction. Renderers **must** use
    /// an even-odd fill rule, or an equivalent path-difference operation, to
    /// paint this as a hollow border ring. A default nonzero fill will also
    /// fill the inner padding contour.
    ///
    /// The method appends to `out` so hot paths can reuse path storage instead
    /// of allocating a new path for every box.
    ///
    /// This method does not apply [`BorderStyle`]. Use
    /// [`BoxDecorationGeometry::border_side_region`] when a lowerer needs
    /// side-specific style data.
    pub fn write_border_ring_path(self, out: &mut BezPath) {
        self.border_edge.write_path(out);
        self.padding_edge.write_path(out);
    }

    /// Appends a closed path for the requested background clip area.
    pub fn write_background_clip(self, area: BoxArea, out: &mut BezPath) {
        self.contour(area).write_path(out);
    }

    /// Builds a new even-odd compound path for the border ring.
    ///
    /// Prefer [`BoxDecorationGeometry::write_border_ring_path`] in hot paths,
    /// and remember that the returned path must be filled with an even-odd fill
    /// rule to remain hollow.
    #[must_use]
    pub fn to_border_ring_path(self) -> BezPath {
        let mut path = BezPath::new();
        self.write_border_ring_path(&mut path);
        path
    }

    /// Returns the central side region for one physical border side.
    ///
    /// The returned region spans between the matching straight side spans of
    /// the border and padding contours. It intentionally does not include
    /// corner transition regions or border-style paint lowering.
    #[must_use]
    pub fn border_side_region(self, side: Side) -> BorderSideGeometry {
        let outer = self.border_edge.side_span(side);
        let inner = self.padding_edge.side_span(side);
        let width = match side {
            Side::Top => self.border_widths.top,
            Side::Right => self.border_widths.right,
            Side::Bottom => self.border_widths.bottom,
            Side::Left => self.border_widths.left,
        };
        let style = match side {
            Side::Top => self.border_styles.top,
            Side::Right => self.border_styles.right,
            Side::Bottom => self.border_styles.bottom,
            Side::Left => self.border_styles.left,
        };

        BorderSideGeometry {
            side,
            style,
            width,
            outer_start: outer.start,
            outer_end: outer.end,
            inner_start: inner.start,
            inner_end: inner.end,
            bounds: bounds_for_points([outer.start, outer.end, inner.start, inner.end]),
        }
    }
}

/// Central geometry for one physical border side.
///
/// This describes the side band between the straight spans of the outer border
/// contour and inner padding contour. It is suitable for simple side-aware
/// lowerers and leaves corner transition regions for a richer future API.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderSideGeometry {
    /// Physical side represented by this region.
    pub side: Side,
    /// Resolved style for this border side.
    pub style: BorderStyle,
    /// Visible border width for this side.
    pub width: f64,
    /// First point on the outer contour side span.
    pub outer_start: Point,
    /// Last point on the outer contour side span.
    pub outer_end: Point,
    /// First point on the inner contour side span.
    pub inner_start: Point,
    /// Last point on the inner contour side span.
    pub inner_end: Point,
    /// Axis-aligned bounds of the four side-region points.
    pub bounds: Rect,
}

impl BorderSideGeometry {
    /// Returns true when this side has no positive border width.
    #[must_use]
    pub fn is_empty(self) -> bool {
        !self.style.paints_border() || !self.width.is_finite() || self.width <= 0.0
    }

    /// Appends this side region as a closed quadrilateral.
    ///
    /// The path is appended to `out`; callers that want only this region
    /// should clear or create their [`BezPath`] before calling.
    pub fn write_path(self, out: &mut BezPath) {
        out.move_to(self.outer_start);
        out.line_to(self.outer_end);
        out.line_to(self.inner_end);
        out.line_to(self.inner_start);
        out.close_path();
    }
}

/// Return `rect` inset by `edges`, clamping overlarge insets to a zero-size
/// rectangle on each over-constrained axis.
///
/// When the left and right insets exceed the rectangle width, the resulting
/// x coordinates collapse proportionally between the two inset requests.
/// The same rule is applied independently to the y axis.
pub fn inset_rect(rect: Rect, edges: Edges<f64>) -> Rect {
    let rect = normalize_rect(rect);
    let edges = edges.clamped_non_negative();
    let (x0, x1) = inset_axis(rect.x0, rect.x1, edges.left, edges.right);
    let (y0, y1) = inset_axis(rect.y0, rect.y1, edges.top, edges.bottom);
    Rect::new(x0, y0, x1, y1)
}

fn inset_axis(min: f64, max: f64, before: f64, after: f64) -> (f64, f64) {
    let extent = finite_non_negative(max - min);
    let sum = before + after;

    if extent <= 0.0 {
        let point = (min + max) * 0.5;
        (point, point)
    } else if sum >= extent && sum > 0.0 {
        let point = min + extent * (before / sum);
        (point, point)
    } else {
        (min + before, max - after)
    }
}

fn bounds_for_points(points: [Point; 4]) -> Rect {
    let mut x0 = points[0].x;
    let mut y0 = points[0].y;
    let mut x1 = points[0].x;
    let mut y1 = points[0].y;

    for point in points.into_iter().skip(1) {
        x0 = x0.min(point.x);
        y0 = y0.min(point.y);
        x1 = x1.max(point.x);
        y1 = y1.max(point.y);
    }

    Rect::new(x0, y0, x1, y1)
}

fn inset_radii_for_shapes(
    radii: CornerRadii,
    edges: Edges<f64>,
    shapes: CornerShapes,
) -> CornerRadii {
    let convex = radii.inset_by_edges(edges);
    CornerRadii::new(
        inset_corner_radius(radii.top_left, convex.top_left, shapes.top_left),
        inset_corner_radius(radii.top_right, convex.top_right, shapes.top_right),
        inset_corner_radius(radii.bottom_right, convex.bottom_right, shapes.bottom_right),
        inset_corner_radius(radii.bottom_left, convex.bottom_left, shapes.bottom_left),
    )
}

fn inset_corner_radius(original: Size, convex: Size, shape: CornerShape) -> Size {
    // Convex corners shrink their radii with the inset edge. Concave corners
    // already carve into the side spans; shrinking both the rect and radius
    // makes inner and outer scoop contours nearly coincide through the curve.
    if is_concave_corner(shape) {
        original
    } else {
        convex
    }
}

fn is_concave_corner(shape: CornerShape) -> bool {
    match shape {
        CornerShape::Superellipse(superellipse) => superellipse.parameter() < 0.0,
        CornerShape::Round | CornerShape::Square | CornerShape::Bevel => false,
    }
}

const fn visible_border_widths(widths: Edges<f64>, styles: Edges<BorderStyle>) -> Edges<f64> {
    Edges::new(
        visible_border_width(widths.top, styles.top),
        visible_border_width(widths.right, styles.right),
        visible_border_width(widths.bottom, styles.bottom),
        visible_border_width(widths.left, styles.left),
    )
}

const fn visible_border_width(width: f64, style: BorderStyle) -> f64 {
    if style.paints_border() { width } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Superellipse;
    use kurbo::{PathEl, Size};

    #[test]
    fn geometry_derives_border_padding_and_content_contours() {
        let geometry = BoxDecorationGeometry::from_round_border_box(
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Edges::new(4.0, 6.0, 8.0, 10.0),
            Edges::new(2.0, 3.0, 4.0, 5.0),
            CornerRadii::uniform(20.0),
        );

        assert_eq!(geometry.padding_box, Rect::new(10.0, 4.0, 94.0, 42.0));
        assert_eq!(geometry.content_box, Rect::new(15.0, 6.0, 91.0, 38.0));
        assert_eq!(
            geometry.padding_edge.radii(),
            CornerRadii::new(
                Size::new(10.0, 16.0),
                Size::new(14.0, 16.0),
                Size::new(14.0, 12.0),
                Size::new(10.0, 12.0),
            ),
        );
        assert_eq!(
            geometry.content_edge.radii(),
            CornerRadii::new(
                Size::new(5.0, 14.0),
                Size::new(11.0, 14.0),
                Size::new(11.0, 8.0),
                Size::new(5.0, 8.0),
            ),
        );
    }

    #[test]
    fn write_border_ring_path_appends_outer_and_inner_subpaths() {
        let geometry = BoxDecorationGeometry::from_round_border_box(
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Edges::all(4.0),
            Edges::ZERO,
            CornerRadii::uniform(12.0),
        );

        let mut path = BezPath::new();
        geometry.write_border_ring_path(&mut path);
        let close_count = path
            .iter()
            .filter(|el| matches!(el, PathEl::ClosePath))
            .count();
        assert_eq!(close_count, 2);
    }

    #[test]
    fn overlarge_insets_collapse_proportionally() {
        let inset = inset_rect(
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Edges::new(40.0, 80.0, 20.0, 120.0),
        );

        assert_eq!(
            inset,
            Rect::new(60.0, 33.333_333_333_333_33, 60.0, 33.333_333_333_333_33,),
        );
    }

    #[test]
    fn negative_and_non_finite_values_are_hardened() {
        let geometry = BoxDecorationGeometry::from_round_border_box(
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Edges::new(-1.0, f64::INFINITY, f64::NAN, 2.0),
            Edges::new(1.0, f64::NAN, f64::INFINITY, -1.0),
            CornerRadii::new(
                Size::new(-1.0, 10.0),
                Size::new(f64::INFINITY, 10.0),
                Size::new(10.0, f64::NAN),
                Size::new(10.0, 10.0),
            ),
        );

        assert_eq!(geometry.border_widths, Edges::new(0.0, 0.0, 0.0, 2.0));
        assert_eq!(geometry.padding_widths, Edges::new(1.0, 0.0, 0.0, 0.0));
        assert_eq!(
            geometry.border_edge.corners.top_left.radii,
            Size::new(0.0, 10.0)
        );
        assert_eq!(
            geometry.border_edge.corners.top_right.radii,
            Size::new(0.0, 10.0)
        );
        assert_eq!(
            geometry.border_edge.corners.bottom_right.radii,
            Size::new(10.0, 0.0)
        );
    }

    #[test]
    fn zero_size_boxes_collapse_radii_and_keep_paths_finite() {
        let geometry = BoxDecorationGeometry::from_round_border_box(
            Rect::new(5.0, 5.0, 5.0, 5.0),
            Edges::all(8.0),
            Edges::all(4.0),
            CornerRadii::all(Size::new(10.0, 20.0)),
        );

        assert_eq!(geometry.border_box, Rect::new(5.0, 5.0, 5.0, 5.0));
        assert_eq!(geometry.padding_box, Rect::new(5.0, 5.0, 5.0, 5.0));
        assert_eq!(geometry.content_box, Rect::new(5.0, 5.0, 5.0, 5.0));
        assert_eq!(geometry.border_edge.radii(), CornerRadii::ZERO);
        assert_eq!(geometry.padding_edge.radii(), CornerRadii::ZERO);
        assert_eq!(geometry.content_edge.radii(), CornerRadii::ZERO);
        assert!(path_is_finite(&geometry.border_edge.to_path()));
        assert!(path_is_finite(&geometry.padding_edge.to_path()));
        assert!(path_is_finite(&geometry.to_border_ring_path()));
    }

    #[test]
    fn background_clip_uses_named_box_area_contours() {
        let geometry = BoxDecorationGeometry::from_round_border_box(
            Rect::new(0.0, 0.0, 100.0, 80.0),
            Edges::all(10.0),
            Edges::all(5.0),
            CornerRadii::uniform(16.0),
        );

        let mut content_clip = BezPath::new();
        geometry.write_background_clip(BoxArea::Content, &mut content_clip);

        assert_eq!(
            geometry.contour(BoxArea::Content).rect,
            geometry.content_box
        );
        assert!(path_is_finite(&content_clip));
    }

    #[test]
    fn non_round_corner_shapes_write_finite_closed_paths() {
        let geometry = BoxDecorationGeometry::from_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::all(6.0),
            Edges::all(4.0),
            CornerRadii::all(Size::new(24.0, 16.0)),
            CornerShapes::new(
                CornerShape::Square,
                CornerShape::Bevel,
                CornerShape::squircle(),
                CornerShape::scoop(),
            ),
        );

        let path = geometry.border_edge.to_path();
        let close_count = path
            .iter()
            .filter(|el| matches!(el, PathEl::ClosePath))
            .count();

        assert_eq!(close_count, 1);
        assert_eq!(
            geometry.border_edge.corners.top_left.shape,
            CornerShape::Square
        );
        assert_eq!(
            geometry.border_edge.corners.bottom_left.shape,
            CornerShape::scoop()
        );
        assert!(path_is_finite(&path));
    }

    #[test]
    fn explicit_round_superellipse_writes_round_corners() {
        let geometry = BoxDecorationGeometry::from_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::ZERO,
            Edges::ZERO,
            CornerRadii::uniform(12.0),
            CornerShapes::all(CornerShape::Superellipse(Superellipse::ROUND)),
        );

        let path = geometry.border_edge.to_path();
        assert!(path.iter().any(|el| matches!(el, PathEl::CurveTo(..))));
        assert!(path_is_finite(&path));
    }

    #[test]
    fn side_regions_follow_central_contour_spans() {
        let geometry = BoxDecorationGeometry::from_round_border_box(
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Edges::new(4.0, 6.0, 8.0, 10.0),
            Edges::ZERO,
            CornerRadii::uniform(12.0),
        );

        let top = geometry.border_side_region(Side::Top);

        assert_eq!(top.width, 4.0);
        assert_eq!(top.style, BorderStyle::Solid);
        assert!(!top.is_empty());
        assert_eq!(top.outer_start, Point::new(12.0, 0.0));
        assert_eq!(top.outer_end, Point::new(88.0, 0.0));
        assert_eq!(top.inner_start, Point::new(12.0, 4.0));
        assert_eq!(top.inner_end, Point::new(88.0, 4.0));

        let mut path = BezPath::new();
        top.write_path(&mut path);
        assert!(path_is_finite(&path));
    }

    #[test]
    fn concave_corner_insets_move_side_spans_inward() {
        let geometry = BoxDecorationGeometry::from_border_box(
            Rect::new(0.0, 0.0, 150.0, 82.0),
            Edges::all(3.0),
            Edges::ZERO,
            CornerRadii::all(Size::new(28.0, 20.0)),
            CornerShapes::all(CornerShape::scoop()),
        );

        let outer_top = geometry.border_edge.side_span(Side::Top);
        let inner_top = geometry.padding_edge.side_span(Side::Top);
        assert!(
            inner_top.start.x > outer_top.start.x,
            "top-left concave padding span should move right instead of sharing the outer endpoint"
        );
        assert!(
            inner_top.end.x < outer_top.end.x,
            "top-right concave padding span should move left instead of sharing the outer endpoint"
        );

        let outer_left = geometry.border_edge.side_span(Side::Left);
        let inner_left = geometry.padding_edge.side_span(Side::Left);
        assert!(
            inner_left.end.y > outer_left.end.y,
            "top-left concave padding span should move down instead of sharing the outer endpoint"
        );
    }

    #[test]
    fn side_regions_with_non_finite_widths_are_empty() {
        let side = BorderSideGeometry {
            side: Side::Top,
            style: BorderStyle::Solid,
            width: f64::NAN,
            outer_start: Point::ZERO,
            outer_end: Point::ZERO,
            inner_start: Point::ZERO,
            inner_end: Point::ZERO,
            bounds: Rect::ZERO,
        };

        assert!(side.is_empty());
    }

    #[test]
    fn styled_geometry_preserves_per_side_styles() {
        let geometry = BoxDecorationGeometry::from_styled_border_box(
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Edges::all(4.0),
            Edges::new(
                BorderStyle::Solid,
                BorderStyle::Dashed,
                BorderStyle::None,
                BorderStyle::Hidden,
            ),
            Edges::ZERO,
            CornerRadii::ZERO,
            CornerShapes::ROUND,
        );

        assert!(geometry.has_border_width());
        assert!(geometry.has_visible_border());
        assert_eq!(geometry.border_widths, Edges::new(4.0, 4.0, 0.0, 0.0));
        assert_eq!(geometry.border_styles.right, BorderStyle::Dashed);
        assert_eq!(
            geometry.border_side_region(Side::Right).style,
            BorderStyle::Dashed
        );
        assert!(geometry.border_side_region(Side::Bottom).is_empty());
        assert!(geometry.border_side_region(Side::Left).is_empty());
    }

    fn path_is_finite(path: &BezPath) -> bool {
        path.iter().all(|element| match element {
            PathEl::MoveTo(point) | PathEl::LineTo(point) => {
                point.x.is_finite() && point.y.is_finite()
            }
            PathEl::QuadTo(control, point) => {
                control.x.is_finite()
                    && control.y.is_finite()
                    && point.x.is_finite()
                    && point.y.is_finite()
            }
            PathEl::CurveTo(control_0, control_1, point) => {
                control_0.x.is_finite()
                    && control_0.y.is_finite()
                    && control_1.x.is_finite()
                    && control_1.y.is_finite()
                    && point.x.is_finite()
                    && point.y.is_finite()
            }
            PathEl::ClosePath => true,
        })
    }
}
