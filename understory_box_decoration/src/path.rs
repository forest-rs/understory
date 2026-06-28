// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{BezPath, Point, Rect, Size};

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

fn append_round_corner(
    path: &mut BezPath,
    placement: CornerPlacement,
    rect: Rect,
    radii: Size,
    end: Point,
) {
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
    path.curve_to(control1, control2, end);
}

fn append_square_corner(path: &mut BezPath, placement: CornerPlacement, rect: Rect, end: Point) {
    path.line_to(match placement {
        CornerPlacement::TopRight => Point::new(rect.x1, rect.y0),
        CornerPlacement::BottomRight => Point::new(rect.x1, rect.y1),
        CornerPlacement::BottomLeft => Point::new(rect.x0, rect.y1),
        CornerPlacement::TopLeft => Point::new(rect.x0, rect.y0),
    });
    path.line_to(end);
}

fn append_notch_corner(
    path: &mut BezPath,
    placement: CornerPlacement,
    rect: Rect,
    radii: Size,
    end: Point,
) {
    path.line_to(match placement {
        CornerPlacement::TopRight => Point::new(rect.x1 - radii.width, rect.y0 + radii.height),
        CornerPlacement::BottomRight => Point::new(rect.x1 - radii.width, rect.y1 - radii.height),
        CornerPlacement::BottomLeft => Point::new(rect.x0 + radii.width, rect.y1 - radii.height),
        CornerPlacement::TopLeft => Point::new(rect.x0 + radii.width, rect.y0 + radii.height),
    });
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
