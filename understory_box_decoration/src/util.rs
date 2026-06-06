// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{Rect, Size};

pub(crate) fn normalize_rect(rect: Rect) -> Rect {
    if rect.x0.is_finite() && rect.y0.is_finite() && rect.x1.is_finite() && rect.y1.is_finite() {
        rect.abs()
    } else {
        Rect::ZERO
    }
}

pub(crate) const fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

pub(crate) const fn clean_size(size: Size) -> Size {
    Size::new(
        finite_non_negative(size.width),
        finite_non_negative(size.height),
    )
}

pub(crate) fn fit_scale(current: f64, side: f64, adjacent_radii: f64) -> f64 {
    if adjacent_radii > side && adjacent_radii > 0.0 {
        current.min(side / adjacent_radii)
    } else {
        current
    }
}

pub(crate) const fn scale_size(size: Size, scale: f64) -> Size {
    Size::new(size.width * scale, size.height * scale)
}

pub(crate) fn distance(a: f64, b: f64) -> f64 {
    if a >= b { a - b } else { b - a }
}
