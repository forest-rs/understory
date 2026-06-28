// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{Point, Rect};

pub(crate) fn rect_from_points(points: &[Point]) -> Rect {
    debug_assert!(
        !points.is_empty(),
        "rectangle construction requires at least one point",
    );
    let first = points[0];
    let mut rect = Rect::new(first.x, first.y, first.x, first.y);
    for point in &points[1..] {
        rect = rect.union(Rect::new(point.x, point.y, point.x, point.y));
    }
    rect
}

pub(crate) fn point_to_rect_distance_squared(point: Point, rect: Rect) -> f64 {
    let rect = rect.abs();
    let dx = if point.x < rect.x0 {
        rect.x0 - point.x
    } else if point.x > rect.x1 {
        point.x - rect.x1
    } else {
        0.0
    };
    let dy = if point.y < rect.y0 {
        rect.y0 - point.y
    } else if point.y > rect.y1 {
        point.y - rect.y1
    } else {
        0.0
    };
    dx * dx + dy * dy
}
