// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{BezPath, CubicBez, ParamCurve, Point, Rect, Size};

use crate::math::powf;
use crate::{BoxContour, CornerRadii, CornerShape, Corners, ResolvedCorner, Side, Superellipse};

const ARC_KAPPA: f64 = 0.552_284_749_830_793_6;
const SUPERELLIPSE_SEGMENTS: usize = 12;
const SUPERELLIPSE_LIMIT: f64 = 10.0;

pub(crate) fn rounded_rect_path(rect: Rect, radii: CornerRadii) -> BezPath {
    let mut path = BezPath::new();
    let contour = BoxContour::from_radii(rect, radii, crate::CornerShapes::ROUND);
    write_contour_path(contour, &mut path);
    path
}

pub(crate) fn write_contour_path(contour: BoxContour, path: &mut BezPath) {
    let rect = contour.rect;
    let corners = contour.corners;

    path.move_to(Point::new(rect.x0 + corners.top_left.radii.width, rect.y0));
    path.line_to(Point::new(rect.x1 - corners.top_right.radii.width, rect.y0));
    append_corner(
        path,
        CornerPlacement::TopRight,
        rect,
        corners.top_right,
        Point::new(rect.x1, rect.y0 + corners.top_right.radii.height),
    );

    path.line_to(Point::new(
        rect.x1,
        rect.y1 - corners.bottom_right.radii.height,
    ));
    append_corner(
        path,
        CornerPlacement::BottomRight,
        rect,
        corners.bottom_right,
        Point::new(rect.x1 - corners.bottom_right.radii.width, rect.y1),
    );

    path.line_to(Point::new(
        rect.x0 + corners.bottom_left.radii.width,
        rect.y1,
    ));
    append_corner(
        path,
        CornerPlacement::BottomLeft,
        rect,
        corners.bottom_left,
        Point::new(rect.x0, rect.y1 - corners.bottom_left.radii.height),
    );

    path.line_to(Point::new(rect.x0, rect.y0 + corners.top_left.radii.height));
    append_corner(
        path,
        CornerPlacement::TopLeft,
        rect,
        corners.top_left,
        Point::new(rect.x0 + corners.top_left.radii.width, rect.y0),
    );
    path.close_path();
}

pub(crate) fn write_contour_side_segment_path(
    contour: BoxContour,
    outer_rect: Rect,
    inner_rect: Rect,
    side: Side,
    path: &mut BezPath,
) {
    let splits = contour_corner_miter_splits(contour, outer_rect, inner_rect);
    path.move_to(side_segment_start_with_splits(contour, side, splits));
    append_contour_side_segment_with_splits(contour, side, splits, path);
}

pub(crate) fn write_contour_side_band_path(
    outer: BoxContour,
    inner: BoxContour,
    side: Side,
    path: &mut BezPath,
) {
    let (outer_splits, inner_splits) = side_band_corner_splits(outer, inner);

    path.move_to(side_segment_start_with_splits(outer, side, outer_splits));
    append_contour_side_segment_with_splits(outer, side, outer_splits, path);
    path.line_to(side_segment_end_with_splits(inner, side, inner_splits));
    append_contour_side_segment_reversed(inner, side, inner_splits, path);
    path.close_path();
}

fn append_contour_side_segment_with_splits(
    contour: BoxContour,
    side: Side,
    splits: Corners<f64>,
    path: &mut BezPath,
) {
    let rect = contour.rect;
    let corners = contour.corners;

    match side {
        Side::Top => {
            append_corner_range(
                path,
                CornerPlacement::TopLeft,
                rect,
                corners.top_left,
                splits.top_left,
                1.0,
            );
            path.line_to(corner_start(
                CornerPlacement::TopRight,
                rect,
                corners.top_right.radii,
            ));
            append_corner_range(
                path,
                CornerPlacement::TopRight,
                rect,
                corners.top_right,
                0.0,
                splits.top_right,
            );
        }
        Side::Right => {
            append_corner_range(
                path,
                CornerPlacement::TopRight,
                rect,
                corners.top_right,
                splits.top_right,
                1.0,
            );
            path.line_to(corner_start(
                CornerPlacement::BottomRight,
                rect,
                corners.bottom_right.radii,
            ));
            append_corner_range(
                path,
                CornerPlacement::BottomRight,
                rect,
                corners.bottom_right,
                0.0,
                splits.bottom_right,
            );
        }
        Side::Bottom => {
            append_corner_range(
                path,
                CornerPlacement::BottomRight,
                rect,
                corners.bottom_right,
                splits.bottom_right,
                1.0,
            );
            path.line_to(corner_start(
                CornerPlacement::BottomLeft,
                rect,
                corners.bottom_left.radii,
            ));
            append_corner_range(
                path,
                CornerPlacement::BottomLeft,
                rect,
                corners.bottom_left,
                0.0,
                splits.bottom_left,
            );
        }
        Side::Left => {
            append_corner_range(
                path,
                CornerPlacement::BottomLeft,
                rect,
                corners.bottom_left,
                splits.bottom_left,
                1.0,
            );
            path.line_to(corner_start(
                CornerPlacement::TopLeft,
                rect,
                corners.top_left.radii,
            ));
            append_corner_range(
                path,
                CornerPlacement::TopLeft,
                rect,
                corners.top_left,
                0.0,
                splits.top_left,
            );
        }
    }
}

fn append_contour_side_segment_reversed(
    contour: BoxContour,
    side: Side,
    splits: Corners<f64>,
    path: &mut BezPath,
) {
    let rect = contour.rect;
    let corners = contour.corners;

    match side {
        Side::Top => {
            append_corner_range(
                path,
                CornerPlacement::TopRight,
                rect,
                corners.top_right,
                splits.top_right,
                0.0,
            );
            path.line_to(corner_end(
                CornerPlacement::TopLeft,
                rect,
                corners.top_left.radii,
            ));
            append_corner_range(
                path,
                CornerPlacement::TopLeft,
                rect,
                corners.top_left,
                1.0,
                splits.top_left,
            );
        }
        Side::Right => {
            append_corner_range(
                path,
                CornerPlacement::BottomRight,
                rect,
                corners.bottom_right,
                splits.bottom_right,
                0.0,
            );
            path.line_to(corner_end(
                CornerPlacement::TopRight,
                rect,
                corners.top_right.radii,
            ));
            append_corner_range(
                path,
                CornerPlacement::TopRight,
                rect,
                corners.top_right,
                1.0,
                splits.top_right,
            );
        }
        Side::Bottom => {
            append_corner_range(
                path,
                CornerPlacement::BottomLeft,
                rect,
                corners.bottom_left,
                splits.bottom_left,
                0.0,
            );
            path.line_to(corner_end(
                CornerPlacement::BottomRight,
                rect,
                corners.bottom_right.radii,
            ));
            append_corner_range(
                path,
                CornerPlacement::BottomRight,
                rect,
                corners.bottom_right,
                1.0,
                splits.bottom_right,
            );
        }
        Side::Left => {
            append_corner_range(
                path,
                CornerPlacement::TopLeft,
                rect,
                corners.top_left,
                splits.top_left,
                0.0,
            );
            path.line_to(corner_end(
                CornerPlacement::BottomLeft,
                rect,
                corners.bottom_left.radii,
            ));
            append_corner_range(
                path,
                CornerPlacement::BottomLeft,
                rect,
                corners.bottom_left,
                1.0,
                splits.bottom_left,
            );
        }
    }
}

fn side_segment_start_with_splits(contour: BoxContour, side: Side, splits: Corners<f64>) -> Point {
    let rect = contour.rect;
    let corners = contour.corners;
    match side {
        Side::Top => corner_point(
            CornerPlacement::TopLeft,
            rect,
            corners.top_left,
            splits.top_left,
        ),
        Side::Right => corner_point(
            CornerPlacement::TopRight,
            rect,
            corners.top_right,
            splits.top_right,
        ),
        Side::Bottom => corner_point(
            CornerPlacement::BottomRight,
            rect,
            corners.bottom_right,
            splits.bottom_right,
        ),
        Side::Left => corner_point(
            CornerPlacement::BottomLeft,
            rect,
            corners.bottom_left,
            splits.bottom_left,
        ),
    }
}

fn side_segment_end_with_splits(contour: BoxContour, side: Side, splits: Corners<f64>) -> Point {
    let rect = contour.rect;
    let corners = contour.corners;
    match side {
        Side::Top => corner_point(
            CornerPlacement::TopRight,
            rect,
            corners.top_right,
            splits.top_right,
        ),
        Side::Right => corner_point(
            CornerPlacement::BottomRight,
            rect,
            corners.bottom_right,
            splits.bottom_right,
        ),
        Side::Bottom => corner_point(
            CornerPlacement::BottomLeft,
            rect,
            corners.bottom_left,
            splits.bottom_left,
        ),
        Side::Left => corner_point(
            CornerPlacement::TopLeft,
            rect,
            corners.top_left,
            splits.top_left,
        ),
    }
}

fn side_band_corner_splits(outer: BoxContour, inner: BoxContour) -> (Corners<f64>, Corners<f64>) {
    (
        contour_corner_miter_splits(outer, outer.rect, inner.rect),
        contour_corner_miter_splits(inner, outer.rect, inner.rect),
    )
}

fn contour_corner_miter_splits(
    contour: BoxContour,
    outer_rect: Rect,
    inner_rect: Rect,
) -> Corners<f64> {
    let top_left = corner_miter_line(CornerPlacement::TopLeft, outer_rect, inner_rect);
    let top_right = corner_miter_line(CornerPlacement::TopRight, outer_rect, inner_rect);
    let bottom_right = corner_miter_line(CornerPlacement::BottomRight, outer_rect, inner_rect);
    let bottom_left = corner_miter_line(CornerPlacement::BottomLeft, outer_rect, inner_rect);

    Corners::new(
        corner_miter_progress(CornerPlacement::TopLeft, contour, top_left),
        corner_miter_progress(CornerPlacement::TopRight, contour, top_right),
        corner_miter_progress(CornerPlacement::BottomRight, contour, bottom_right),
        corner_miter_progress(CornerPlacement::BottomLeft, contour, bottom_left),
    )
}

fn corner_miter_line(placement: CornerPlacement, outer: Rect, inner: Rect) -> (Point, Point) {
    (rect_corner(placement, outer), rect_corner(placement, inner))
}

fn corner_miter_progress(
    placement: CornerPlacement,
    contour: BoxContour,
    line: (Point, Point),
) -> f64 {
    const SAMPLES: usize = 64;
    const BISECT_STEPS: usize = 48;

    let (line_start, line_end) = line;
    let line_epsilon = line_start.distance(line_end).max(1.0) * 1e-9;
    if line_start.distance(line_end) <= f64::EPSILON {
        return 0.5;
    }

    let corner = corner_for_placement(contour, placement);
    let mut best_progress = 0.0;
    let mut best_distance = signed_line_distance(
        line_start,
        line_end,
        corner_point(placement, contour.rect, corner, 0.0),
    )
    .abs();
    let mut previous_progress = 0.0;
    let mut previous_distance = signed_line_distance(
        line_start,
        line_end,
        corner_point(placement, contour.rect, corner, previous_progress),
    );

    if previous_distance.abs() <= line_epsilon {
        return previous_progress;
    }

    for i in 1..=SAMPLES {
        let progress = i as f64 / SAMPLES as f64;
        let point = corner_point(placement, contour.rect, corner, progress);
        let distance = signed_line_distance(line_start, line_end, point);

        if distance.abs() < best_distance {
            best_distance = distance.abs();
            best_progress = progress;
        }

        if distance.abs() <= line_epsilon {
            return progress;
        }

        if previous_distance.is_sign_positive() != distance.is_sign_positive() {
            let mut low = previous_progress;
            let mut high = progress;
            let mut low_distance = previous_distance;

            for _ in 0..BISECT_STEPS {
                let middle = (low + high) * 0.5;
                let middle_distance = signed_line_distance(
                    line_start,
                    line_end,
                    corner_point(placement, contour.rect, corner, middle),
                );

                if middle_distance.abs() <= line_epsilon {
                    return middle;
                }

                if low_distance.is_sign_positive() == middle_distance.is_sign_positive() {
                    low = middle;
                    low_distance = middle_distance;
                } else {
                    high = middle;
                }
            }

            return (low + high) * 0.5;
        }

        previous_progress = progress;
        previous_distance = distance;
    }

    best_progress
}

fn corner_for_placement(contour: BoxContour, placement: CornerPlacement) -> ResolvedCorner {
    match placement {
        CornerPlacement::TopRight => contour.corners.top_right,
        CornerPlacement::BottomRight => contour.corners.bottom_right,
        CornerPlacement::BottomLeft => contour.corners.bottom_left,
        CornerPlacement::TopLeft => contour.corners.top_left,
    }
}

fn rect_corner(placement: CornerPlacement, rect: Rect) -> Point {
    match placement {
        CornerPlacement::TopRight => Point::new(rect.x1, rect.y0),
        CornerPlacement::BottomRight => Point::new(rect.x1, rect.y1),
        CornerPlacement::BottomLeft => Point::new(rect.x0, rect.y1),
        CornerPlacement::TopLeft => Point::new(rect.x0, rect.y0),
    }
}

fn signed_line_distance(start: Point, end: Point, point: Point) -> f64 {
    let line_x = end.x - start.x;
    let line_y = end.y - start.y;
    let point_x = point.x - start.x;
    let point_y = point.y - start.y;
    line_x * point_y - line_y * point_x
}

#[derive(Clone, Copy, Debug)]
enum CornerPlacement {
    TopRight,
    BottomRight,
    BottomLeft,
    TopLeft,
}

fn append_corner(
    path: &mut BezPath,
    placement: CornerPlacement,
    rect: Rect,
    corner: ResolvedCorner,
    end: Point,
) {
    let radii = corner.radii;
    if radii.width <= 0.0 || radii.height <= 0.0 {
        path.line_to(end);
        return;
    }

    match corner.shape {
        CornerShape::Round => append_round_corner(path, placement, rect, radii, end),
        CornerShape::Square => append_square_corner(path, placement, rect, end),
        CornerShape::Bevel => path.line_to(end),
        CornerShape::Superellipse(superellipse) => {
            append_superellipse_corner(path, placement, rect, radii, superellipse, end);
        }
    }
}

fn append_corner_range(
    path: &mut BezPath,
    placement: CornerPlacement,
    rect: Rect,
    corner: ResolvedCorner,
    start: f64,
    end: f64,
) {
    let start = positive_unit(start);
    let end = positive_unit(end);
    if distance(start, end) <= f64::EPSILON {
        return;
    }

    if corner.radii.width <= 0.0 || corner.radii.height <= 0.0 {
        path.line_to(corner_point(placement, rect, corner, end));
        return;
    }

    match corner.shape {
        CornerShape::Round => {
            let cubic = round_corner_cubic(placement, rect, corner.radii);
            append_cubic_range(path, cubic, start, end);
        }
        CornerShape::Superellipse(superellipse)
            if distance(superellipse.parameter(), 1.0) <= 1e-9 =>
        {
            let cubic = round_corner_cubic(placement, rect, corner.radii);
            append_cubic_range(path, cubic, start, end);
        }
        _ => append_sampled_corner_range(path, placement, rect, corner, start, end),
    }
}

fn append_cubic_range(path: &mut BezPath, cubic: CubicBez, start: f64, end: f64) {
    if start < end {
        let segment = cubic.subsegment(start..end);
        path.curve_to(segment.p1, segment.p2, segment.p3);
    } else {
        let segment = cubic.subsegment(end..start);
        path.curve_to(segment.p2, segment.p1, segment.p0);
    }
}

fn append_sampled_corner_range(
    path: &mut BezPath,
    placement: CornerPlacement,
    rect: Rect,
    corner: ResolvedCorner,
    start: f64,
    end: f64,
) {
    for i in 1..=SUPERELLIPSE_SEGMENTS {
        let progress = start + (end - start) * (i as f64 / SUPERELLIPSE_SEGMENTS as f64);
        path.line_to(corner_point(placement, rect, corner, progress));
    }
}

fn corner_point(
    placement: CornerPlacement,
    rect: Rect,
    corner: ResolvedCorner,
    progress: f64,
) -> Point {
    let progress = positive_unit(progress);
    let radii = corner.radii;
    if radii.width <= 0.0 || radii.height <= 0.0 {
        return corner_end(placement, rect, radii);
    }

    match corner.shape {
        CornerShape::Round => round_corner_cubic(placement, rect, radii).eval(progress),
        CornerShape::Square => piecewise_corner_point(
            corner_start(placement, rect, radii),
            square_corner_point(placement, rect),
            corner_end(placement, rect, radii),
            progress,
        ),
        CornerShape::Bevel => lerp_point(
            corner_start(placement, rect, radii),
            corner_end(placement, rect, radii),
            progress,
        ),
        CornerShape::Superellipse(superellipse) => {
            let parameter = superellipse.parameter();
            if distance(parameter, 1.0) <= 1e-9 {
                round_corner_cubic(placement, rect, radii).eval(progress)
            } else if distance(parameter, 0.0) <= 1e-9 {
                lerp_point(
                    corner_start(placement, rect, radii),
                    corner_end(placement, rect, radii),
                    progress,
                )
            } else if parameter >= SUPERELLIPSE_LIMIT || parameter == f64::INFINITY {
                piecewise_corner_point(
                    corner_start(placement, rect, radii),
                    square_corner_point(placement, rect),
                    corner_end(placement, rect, radii),
                    progress,
                )
            } else if parameter <= -SUPERELLIPSE_LIMIT || parameter == f64::NEG_INFINITY {
                piecewise_corner_point(
                    corner_start(placement, rect, radii),
                    notch_corner_point(placement, rect, radii),
                    corner_end(placement, rect, radii),
                    progress,
                )
            } else {
                let exponent = powf(2.0, parameter);
                if !exponent.is_finite() || exponent <= 0.0 {
                    return lerp_point(
                        corner_start(placement, rect, radii),
                        corner_end(placement, rect, radii),
                        progress,
                    );
                }
                let (u, v) = superellipse_point(placement, progress, exponent);
                point_in_corner(rect, placement, radii, u, v)
            }
        }
    }
}

fn append_round_corner(
    path: &mut BezPath,
    placement: CornerPlacement,
    rect: Rect,
    radii: Size,
    end: Point,
) {
    let cubic = round_corner_cubic(placement, rect, radii);
    path.curve_to(cubic.p1, cubic.p2, end);
}

fn append_square_corner(path: &mut BezPath, placement: CornerPlacement, rect: Rect, end: Point) {
    path.line_to(square_corner_point(placement, rect));
    path.line_to(end);
}

fn append_notch_corner(
    path: &mut BezPath,
    placement: CornerPlacement,
    rect: Rect,
    radii: Size,
    end: Point,
) {
    path.line_to(notch_corner_point(placement, rect, radii));
    path.line_to(end);
}

fn append_superellipse_corner(
    path: &mut BezPath,
    placement: CornerPlacement,
    rect: Rect,
    radii: Size,
    superellipse: Superellipse,
    end: Point,
) {
    let parameter = superellipse.parameter();
    if distance(parameter, 1.0) <= 1e-9 {
        append_round_corner(path, placement, rect, radii, end);
        return;
    }
    if distance(parameter, 0.0) <= 1e-9 {
        path.line_to(end);
        return;
    }
    if parameter >= SUPERELLIPSE_LIMIT || parameter == f64::INFINITY {
        append_square_corner(path, placement, rect, end);
        return;
    }
    if parameter <= -SUPERELLIPSE_LIMIT || parameter == f64::NEG_INFINITY {
        append_notch_corner(path, placement, rect, radii, end);
        return;
    }

    let exponent = powf(2.0, parameter);
    if !exponent.is_finite() || exponent <= 0.0 {
        path.line_to(end);
        return;
    }

    for i in 1..=SUPERELLIPSE_SEGMENTS {
        let progress = i as f64 / SUPERELLIPSE_SEGMENTS as f64;
        let (u, v) = superellipse_point(placement, progress, exponent);
        path.line_to(point_in_corner(rect, placement, radii, u, v));
    }
}

fn superellipse_point(placement: CornerPlacement, progress: f64, exponent: f64) -> (f64, f64) {
    match placement {
        CornerPlacement::TopRight => (forward_superellipse(progress, exponent), progress),
        CornerPlacement::BottomRight => (reverse_superellipse(progress, exponent), progress),
        CornerPlacement::BottomLeft => {
            let q = 1.0 - progress;
            (1.0 - reverse_superellipse(q, exponent), q)
        }
        CornerPlacement::TopLeft => (progress, 1.0 - forward_superellipse(progress, exponent)),
    }
}

fn forward_superellipse(progress: f64, exponent: f64) -> f64 {
    let q = 1.0 - progress;
    powf(positive_unit(1.0 - powf(q, exponent)), 1.0 / exponent)
}

fn reverse_superellipse(progress: f64, exponent: f64) -> f64 {
    powf(
        positive_unit(1.0 - powf(progress, exponent)),
        1.0 / exponent,
    )
}

fn point_in_corner(rect: Rect, placement: CornerPlacement, radii: Size, u: f64, v: f64) -> Point {
    let (x0, y0) = match placement {
        CornerPlacement::TopRight => (rect.x1 - radii.width, rect.y0),
        CornerPlacement::BottomRight => (rect.x1 - radii.width, rect.y1 - radii.height),
        CornerPlacement::BottomLeft => (rect.x0, rect.y1 - radii.height),
        CornerPlacement::TopLeft => (rect.x0, rect.y0),
    };
    Point::new(x0 + radii.width * u, y0 + radii.height * v)
}

fn round_corner_cubic(placement: CornerPlacement, rect: Rect, radii: Size) -> CubicBez {
    let start = corner_start(placement, rect, radii);
    let end = corner_end(placement, rect, radii);
    let (control1, control2) = match placement {
        CornerPlacement::TopRight => (
            Point::new(rect.x1 - radii.width + ARC_KAPPA * radii.width, rect.y0),
            Point::new(rect.x1, rect.y0 + radii.height - ARC_KAPPA * radii.height),
        ),
        CornerPlacement::BottomRight => (
            Point::new(rect.x1, rect.y1 - radii.height + ARC_KAPPA * radii.height),
            Point::new(rect.x1 - radii.width + ARC_KAPPA * radii.width, rect.y1),
        ),
        CornerPlacement::BottomLeft => (
            Point::new(rect.x0 + radii.width - ARC_KAPPA * radii.width, rect.y1),
            Point::new(rect.x0, rect.y1 - radii.height + ARC_KAPPA * radii.height),
        ),
        CornerPlacement::TopLeft => (
            Point::new(rect.x0, rect.y0 + radii.height - ARC_KAPPA * radii.height),
            Point::new(rect.x0 + radii.width - ARC_KAPPA * radii.width, rect.y0),
        ),
    };
    CubicBez::new(start, control1, control2, end)
}

fn corner_start(placement: CornerPlacement, rect: Rect, radii: Size) -> Point {
    match placement {
        CornerPlacement::TopRight => Point::new(rect.x1 - radii.width, rect.y0),
        CornerPlacement::BottomRight => Point::new(rect.x1, rect.y1 - radii.height),
        CornerPlacement::BottomLeft => Point::new(rect.x0 + radii.width, rect.y1),
        CornerPlacement::TopLeft => Point::new(rect.x0, rect.y0 + radii.height),
    }
}

fn corner_end(placement: CornerPlacement, rect: Rect, radii: Size) -> Point {
    match placement {
        CornerPlacement::TopRight => Point::new(rect.x1, rect.y0 + radii.height),
        CornerPlacement::BottomRight => Point::new(rect.x1 - radii.width, rect.y1),
        CornerPlacement::BottomLeft => Point::new(rect.x0, rect.y1 - radii.height),
        CornerPlacement::TopLeft => Point::new(rect.x0 + radii.width, rect.y0),
    }
}

fn square_corner_point(placement: CornerPlacement, rect: Rect) -> Point {
    match placement {
        CornerPlacement::TopRight => Point::new(rect.x1, rect.y0),
        CornerPlacement::BottomRight => Point::new(rect.x1, rect.y1),
        CornerPlacement::BottomLeft => Point::new(rect.x0, rect.y1),
        CornerPlacement::TopLeft => Point::new(rect.x0, rect.y0),
    }
}

fn notch_corner_point(placement: CornerPlacement, rect: Rect, radii: Size) -> Point {
    match placement {
        CornerPlacement::TopRight => Point::new(rect.x1 - radii.width, rect.y0 + radii.height),
        CornerPlacement::BottomRight => Point::new(rect.x1 - radii.width, rect.y1 - radii.height),
        CornerPlacement::BottomLeft => Point::new(rect.x0 + radii.width, rect.y1 - radii.height),
        CornerPlacement::TopLeft => Point::new(rect.x0 + radii.width, rect.y0 + radii.height),
    }
}

fn piecewise_corner_point(start: Point, middle: Point, end: Point, progress: f64) -> Point {
    if progress <= 0.5 {
        lerp_point(start, middle, progress * 2.0)
    } else {
        lerp_point(middle, end, (progress - 0.5) * 2.0)
    }
}

fn lerp_point(start: Point, end: Point, progress: f64) -> Point {
    Point::new(
        start.x + (end.x - start.x) * progress,
        start.y + (end.y - start.y) * progress,
    )
}

fn positive_unit(value: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn distance(a: f64, b: f64) -> f64 {
    if a >= b { a - b } else { b - a }
}
