// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{BezPath, Point, Rect, Size};

use crate::util::{finite_non_negative, normalize_rect};
use crate::{
    BorderStyle, BoxArea, BoxContour, CornerRadii, CornerShape, CornerShapes, Edges,
    ResolvedCorner, Side,
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
    /// copied into [`crate::BorderSidePaintGeometry`]; renderer/backend code
    /// remains responsible for lowering each style into drawing commands.
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
    /// [`BoxDecorationGeometry::border_paint`] when a lowerer needs resolved
    /// border paint fragments.
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

    /// Returns an intermediate contour through the border ring.
    ///
    /// `0.0` is the outer border edge and `1.0` is the inner padding edge.
    /// Values outside that interval are clamped; non-finite values are treated
    /// as `0.0`.
    ///
    /// This is intended for renderers that lower border styles such as
    /// `double`, `groove`, `ridge`, dashed, or dotted by painting bands or
    /// strokes inside the already-resolved border ring. It preserves resolved
    /// corner shapes while interpolating the box and corner radii between the
    /// border and padding contours.
    #[must_use]
    pub(crate) fn border_contour_at(self, t: f64) -> BoxContour {
        interpolate_contour(self.border_edge, self.padding_edge, unit_interval(t))
    }

    /// Appends a compound even-odd path for a band inside the border ring.
    ///
    /// `outer_t` and `inner_t` use the same coordinate as
    /// [`Self::border_contour_at`], where `0.0` is the outer border edge and
    /// `1.0` is the inner padding edge. The values are clamped into `[0, 1]`,
    /// and `inner_t` is clamped to be no smaller than `outer_t`.
    ///
    /// This writes two closed subpaths with matching winding. Renderers
    /// **must** fill the result with an even-odd rule to paint only the band.
    pub(crate) fn write_border_band_path(self, outer_t: f64, inner_t: f64, out: &mut BezPath) {
        let outer_t = unit_interval(outer_t);
        let inner_t = unit_interval(inner_t).max(outer_t);
        self.border_contour_at(outer_t).write_path(out);
        self.border_contour_at(inner_t).write_path(out);
    }

    /// Builds a compound even-odd path for a band inside the border ring.
    ///
    /// Prefer [`Self::write_border_band_path`] in hot paths, and remember that
    /// the returned path must be filled with an even-odd rule to remain hollow.
    #[must_use]
    pub(crate) fn to_border_band_path(self, outer_t: f64, inner_t: f64) -> BezPath {
        let mut path = BezPath::new();
        self.write_border_band_path(outer_t, inner_t, &mut path);
        path
    }

    /// Appends the miter clip polygon for one physical border side.
    ///
    /// The polygon covers the requested side's share of the border ring,
    /// including its miter share of the adjacent corner areas. It is intended
    /// to be intersected with [`Self::write_border_ring_path`] or
    /// [`Self::write_border_band_path`]; it is not a standalone border paint
    /// shape. The outer and inner contours still define the visible curved or
    /// shaped edge.
    ///
    /// This is useful when per-side brushes or styles need deterministic
    /// corner ownership without asking a renderer to reconstruct box geometry.
    /// The clip uses straight miter boundaries from the border-box corner to
    /// the corresponding padding-box corner.
    pub(crate) fn write_border_side_clip_path(self, side: Side, out: &mut BezPath) {
        let points = border_side_clip_points(self.border_box, self.padding_box, side);
        out.move_to(points[0]);
        out.line_to(points[1]);
        out.line_to(points[2]);
        out.line_to(points[3]);
        out.close_path();
    }

    /// Builds the miter clip polygon for one physical border side.
    ///
    /// Prefer [`Self::write_border_side_clip_path`] in hot paths.
    #[must_use]
    pub(crate) fn to_border_side_clip_path(self, side: Side) -> BezPath {
        let mut path = BezPath::new();
        self.write_border_side_clip_path(side, &mut path);
        path
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

fn interpolate_contour(outer: BoxContour, inner: BoxContour, t: f64) -> BoxContour {
    BoxContour::new(
        Rect::new(
            lerp_f64(outer.rect.x0, inner.rect.x0, t),
            lerp_f64(outer.rect.y0, inner.rect.y0, t),
            lerp_f64(outer.rect.x1, inner.rect.x1, t),
            lerp_f64(outer.rect.y1, inner.rect.y1, t),
        ),
        crate::Corners::new(
            interpolate_corner(outer.corners.top_left, inner.corners.top_left, t),
            interpolate_corner(outer.corners.top_right, inner.corners.top_right, t),
            interpolate_corner(outer.corners.bottom_right, inner.corners.bottom_right, t),
            interpolate_corner(outer.corners.bottom_left, inner.corners.bottom_left, t),
        ),
    )
}

fn interpolate_corner(outer: ResolvedCorner, inner: ResolvedCorner, t: f64) -> ResolvedCorner {
    ResolvedCorner::new(
        Size::new(
            lerp_f64(outer.radii.width, inner.radii.width, t),
            lerp_f64(outer.radii.height, inner.radii.height, t),
        ),
        outer.shape,
    )
}

fn border_side_clip_points(border_box: Rect, padding_box: Rect, side: Side) -> [Point; 4] {
    let outer_top_left = Point::new(border_box.x0, border_box.y0);
    let outer_top_right = Point::new(border_box.x1, border_box.y0);
    let outer_bottom_right = Point::new(border_box.x1, border_box.y1);
    let outer_bottom_left = Point::new(border_box.x0, border_box.y1);

    let inner_top_left = Point::new(padding_box.x0, padding_box.y0);
    let inner_top_right = Point::new(padding_box.x1, padding_box.y0);
    let inner_bottom_right = Point::new(padding_box.x1, padding_box.y1);
    let inner_bottom_left = Point::new(padding_box.x0, padding_box.y1);

    match side {
        Side::Top => [
            outer_top_left,
            outer_top_right,
            inner_top_right,
            inner_top_left,
        ],
        Side::Right => [
            outer_top_right,
            outer_bottom_right,
            inner_bottom_right,
            inner_top_right,
        ],
        Side::Bottom => [
            outer_bottom_right,
            outer_bottom_left,
            inner_bottom_left,
            inner_bottom_right,
        ],
        Side::Left => [
            outer_bottom_left,
            outer_top_left,
            inner_top_left,
            inner_bottom_left,
        ],
    }
}

fn lerp_f64(start: f64, end: f64, t: f64) -> f64 {
    start + (end - start) * t
}

fn unit_interval(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
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
    fn border_contour_at_interpolates_between_border_and_padding_edges() {
        let geometry = BoxDecorationGeometry::from_round_border_box(
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Edges::new(6.0, 12.0, 18.0, 24.0),
            Edges::ZERO,
            CornerRadii::uniform(20.0),
        );

        let middle = geometry.border_contour_at(0.5);

        assert_eq!(middle.rect, Rect::new(12.0, 3.0, 94.0, 41.0));
        assert_eq!(middle.corners.top_left.radii, Size::new(10.0, 17.0));
        assert_eq!(middle.corners.bottom_right.radii, Size::new(14.0, 11.0));
        assert_eq!(middle.corners.top_left.shape, CornerShape::Round);
        assert_eq!(geometry.border_contour_at(-1.0), geometry.border_edge);
        assert_eq!(geometry.border_contour_at(f64::NAN), geometry.border_edge);
        assert_eq!(geometry.border_contour_at(2.0), geometry.padding_edge);
    }

    #[test]
    fn border_band_path_appends_shaped_outer_and_inner_subpaths() {
        let geometry = BoxDecorationGeometry::from_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::all(12.0),
            Edges::ZERO,
            CornerRadii::all(Size::new(24.0, 16.0)),
            CornerShapes::new(
                CornerShape::Square,
                CornerShape::Bevel,
                CornerShape::squircle(),
                CornerShape::scoop(),
            ),
        );

        let path = geometry.to_border_band_path(0.0, 1.0 / 3.0);
        let close_count = path
            .iter()
            .filter(|el| matches!(el, PathEl::ClosePath))
            .count();

        assert_eq!(close_count, 2);
        assert_eq!(
            geometry
                .border_contour_at(1.0 / 3.0)
                .corners
                .bottom_right
                .shape,
            CornerShape::squircle()
        );
        assert_eq!(
            geometry
                .border_contour_at(1.0 / 3.0)
                .corners
                .bottom_left
                .shape,
            CornerShape::scoop()
        );
        assert!(path_is_finite(&path));
    }

    #[test]
    fn border_side_clip_path_uses_mitered_corner_ownership() {
        let geometry = BoxDecorationGeometry::from_round_border_box(
            Rect::new(0.0, 0.0, 100.0, 60.0),
            Edges::new(6.0, 12.0, 18.0, 24.0),
            Edges::ZERO,
            CornerRadii::uniform(20.0),
        );

        let top = geometry.to_border_side_clip_path(Side::Top);
        let mut elements = top.iter();
        assert_eq!(elements.next(), Some(PathEl::MoveTo(Point::new(0.0, 0.0))));
        assert_eq!(
            elements.next(),
            Some(PathEl::LineTo(Point::new(100.0, 0.0)))
        );
        assert_eq!(elements.next(), Some(PathEl::LineTo(Point::new(88.0, 6.0))));
        assert_eq!(elements.next(), Some(PathEl::LineTo(Point::new(24.0, 6.0))));
        assert_eq!(elements.next(), Some(PathEl::ClosePath));
        assert_eq!(elements.next(), None);
        assert!(path_is_finite(&top));
    }

    #[test]
    fn border_side_clip_path_can_clip_shaped_border_rings() {
        let geometry = BoxDecorationGeometry::from_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::all(12.0),
            Edges::ZERO,
            CornerRadii::all(Size::new(24.0, 16.0)),
            CornerShapes::new(
                CornerShape::Square,
                CornerShape::Bevel,
                CornerShape::squircle(),
                CornerShape::scoop(),
            ),
        );

        let mut side_clip = BezPath::new();
        geometry.write_border_side_clip_path(Side::Right, &mut side_clip);
        let mut ring = BezPath::new();
        geometry.write_border_ring_path(&mut ring);

        assert_eq!(
            side_clip
                .iter()
                .filter(|el| matches!(el, PathEl::ClosePath))
                .count(),
            1
        );
        assert_eq!(
            ring.iter()
                .filter(|el| matches!(el, PathEl::ClosePath))
                .count(),
            2
        );
        assert_eq!(
            geometry.border_edge.corners.bottom_right.shape,
            CornerShape::squircle()
        );
        assert!(path_is_finite(&side_clip));
        assert!(path_is_finite(&ring));
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
    fn side_paint_geometry_reports_resolved_style_and_width() {
        let geometry = BoxDecorationGeometry::from_round_border_box(
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Edges::new(4.0, 6.0, 8.0, 10.0),
            Edges::ZERO,
            CornerRadii::uniform(12.0),
        );

        let top = geometry.border_paint().side(Side::Top);

        assert_eq!(top.width(), 4.0);
        assert_eq!(top.style(), BorderStyle::Solid);
        assert!(!top.is_empty());
        assert_eq!(
            geometry.border_edge.side_span(Side::Top).start,
            Point::new(12.0, 0.0)
        );
        assert!(path_is_finite(&top.ring().path));
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
    fn side_paint_geometry_with_non_finite_width_is_empty() {
        let geometry = BoxDecorationGeometry::from_round_border_box(
            Rect::new(0.0, 0.0, 100.0, 50.0),
            Edges::new(f64::NAN, 4.0, 4.0, 4.0),
            Edges::ZERO,
            CornerRadii::ZERO,
        );

        assert!(geometry.border_paint().side(Side::Top).is_empty());
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
            geometry.border_paint().side(Side::Right).style(),
            BorderStyle::Dashed
        );
        assert!(geometry.border_paint().side(Side::Bottom).is_empty());
        assert!(geometry.border_paint().side(Side::Left).is_empty());
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
