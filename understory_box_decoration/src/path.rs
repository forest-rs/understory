// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{BezPath, CubicBez, ParamCurve, Point, Rect, Size};

use crate::math::powf;
use crate::{BoxContour, CornerRadii, CornerShape, ResolvedCorner, Superellipse};

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
    side: crate::Side,
    path: &mut BezPath,
) {
    let rect = contour.rect;
    let corners = contour.corners;

    match side {
        crate::Side::Top => {
            path.move_to(corner_point(
                CornerPlacement::TopLeft,
                rect,
                corners.top_left,
                0.5,
            ));
            append_corner_range(
                path,
                CornerPlacement::TopLeft,
                rect,
                corners.top_left,
                0.5,
                1.0,
            );
            path.line_to(Point::new(rect.x1 - corners.top_right.radii.width, rect.y0));
            append_corner_range(
                path,
                CornerPlacement::TopRight,
                rect,
                corners.top_right,
                0.0,
                0.5,
            );
        }
        crate::Side::Right => {
            path.move_to(corner_point(
                CornerPlacement::TopRight,
                rect,
                corners.top_right,
                0.5,
            ));
            append_corner_range(
                path,
                CornerPlacement::TopRight,
                rect,
                corners.top_right,
                0.5,
                1.0,
            );
            path.line_to(Point::new(
                rect.x1,
                rect.y1 - corners.bottom_right.radii.height,
            ));
            append_corner_range(
                path,
                CornerPlacement::BottomRight,
                rect,
                corners.bottom_right,
                0.0,
                0.5,
            );
        }
        crate::Side::Bottom => {
            path.move_to(corner_point(
                CornerPlacement::BottomRight,
                rect,
                corners.bottom_right,
                0.5,
            ));
            append_corner_range(
                path,
                CornerPlacement::BottomRight,
                rect,
                corners.bottom_right,
                0.5,
                1.0,
            );
            path.line_to(Point::new(
                rect.x0 + corners.bottom_left.radii.width,
                rect.y1,
            ));
            append_corner_range(
                path,
                CornerPlacement::BottomLeft,
                rect,
                corners.bottom_left,
                0.0,
                0.5,
            );
        }
        crate::Side::Left => {
            path.move_to(corner_point(
                CornerPlacement::BottomLeft,
                rect,
                corners.bottom_left,
                0.5,
            ));
            append_corner_range(
                path,
                CornerPlacement::BottomLeft,
                rect,
                corners.bottom_left,
                0.5,
                1.0,
            );
            path.line_to(Point::new(rect.x0, rect.y0 + corners.top_left.radii.height));
            append_corner_range(
                path,
                CornerPlacement::TopLeft,
                rect,
                corners.top_left,
                0.0,
                0.5,
            );
        }
    }
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
    if end <= start {
        return;
    }

    if corner.radii.width <= 0.0 || corner.radii.height <= 0.0 {
        path.line_to(corner_point(placement, rect, corner, end));
        return;
    }

    match corner.shape {
        CornerShape::Round => {
            let cubic = round_corner_cubic(placement, rect, corner.radii);
            let segment = cubic.subsegment(start..end);
            path.curve_to(segment.p1, segment.p2, segment.p3);
        }
        CornerShape::Superellipse(superellipse)
            if distance(superellipse.parameter(), 1.0) <= 1e-9 =>
        {
            let cubic = round_corner_cubic(placement, rect, corner.radii);
            let segment = cubic.subsegment(start..end);
            path.curve_to(segment.p1, segment.p2, segment.p3);
        }
        _ => append_sampled_corner_range(path, placement, rect, corner, start, end),
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
