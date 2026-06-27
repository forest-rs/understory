// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;
use alloc::vec::Vec;

use crate::{Point, Rect, TabBarPlacement};

pub(crate) fn split_tab_bar(
    rect: Rect,
    placement: TabBarPlacement,
    thickness: f64,
) -> (Rect, Rect) {
    debug_assert!(rect.is_finite(), "rectangle must be finite");
    debug_assert!(
        rect.width() >= 0.0 && rect.height() >= 0.0,
        "tab bar splitting requires a normalized rectangle",
    );
    debug_assert!(
        thickness.is_finite() && thickness >= 0.0,
        "tab bar thickness must be finite and non-negative",
    );
    match placement {
        TabBarPlacement::Hidden => (Rect::default(), rect),
        TabBarPlacement::Top => {
            let thickness = thickness.min(rect.height());
            (
                Rect::new(rect.x0, rect.y0, rect.x1, rect.y0 + thickness),
                Rect::new(rect.x0, rect.y0 + thickness, rect.x1, rect.y1),
            )
        }
        TabBarPlacement::Bottom => {
            let thickness = thickness.min(rect.height());
            (
                Rect::new(rect.x0, rect.y1 - thickness, rect.x1, rect.y1),
                Rect::new(rect.x0, rect.y0, rect.x1, rect.y1 - thickness),
            )
        }
        TabBarPlacement::Left => {
            let thickness = thickness.min(rect.width());
            (
                Rect::new(rect.x0, rect.y0, rect.x0 + thickness, rect.y1),
                Rect::new(rect.x0 + thickness, rect.y0, rect.x1, rect.y1),
            )
        }
        TabBarPlacement::Right => {
            let thickness = thickness.min(rect.width());
            (
                Rect::new(rect.x1 - thickness, rect.y0, rect.x1, rect.y1),
                Rect::new(rect.x0, rect.y0, rect.x1 - thickness, rect.y1),
            )
        }
    }
}

pub(crate) fn tab_rects(rect: Rect, placement: TabBarPlacement, count: usize) -> Vec<Rect> {
    debug_assert!(rect.is_finite(), "rectangle must be finite");
    debug_assert!(
        rect.width() >= 0.0 && rect.height() >= 0.0,
        "tab rectangle generation requires a normalized rectangle",
    );
    if count == 0 {
        return Vec::new();
    }
    let mut rects = Vec::with_capacity(count);
    match placement {
        TabBarPlacement::Top | TabBarPlacement::Bottom | TabBarPlacement::Hidden => {
            let width = rect.width() / count as f64;
            for index in 0..count {
                let x0 = rect.x0 + width * index as f64;
                let x1 = if index + 1 == count {
                    rect.x1
                } else {
                    x0 + width
                };
                rects.push(Rect::new(x0, rect.y0, x1, rect.y1));
            }
        }
        TabBarPlacement::Left | TabBarPlacement::Right => {
            let height = rect.height() / count as f64;
            for index in 0..count {
                let y0 = rect.y0 + height * index as f64;
                let y1 = if index + 1 == count {
                    rect.y1
                } else {
                    y0 + height
                };
                rects.push(Rect::new(rect.x0, y0, rect.x1, y1));
            }
        }
    }
    rects
}

pub(crate) fn solve_lengths(total: f64, shares: &[f64], min_major: f64) -> Vec<f64> {
    let count = shares.len();
    if count == 0 {
        return Vec::new();
    }
    debug_assert!(
        total.is_finite() && total >= 0.0,
        "split solver total length must be finite and non-negative",
    );
    debug_assert!(
        min_major.is_finite() && min_major >= 0.0,
        "split solver minimum length must be finite and non-negative",
    );
    if total == 0.0 {
        return vec![0.0; count];
    }

    let min_total = min_major * count as f64;
    if min_major > 0.0 && min_total >= total {
        return vec![total / count as f64; count];
    }

    debug_assert!(
        shares.iter().all(|share| share.is_finite() && *share > 0.0),
        "split solver shares must be finite and positive",
    );
    let share_sum = shares.iter().copied().sum::<f64>();
    let mut lengths: Vec<_> = shares
        .iter()
        .map(|share| total * (*share / share_sum))
        .collect();

    if min_major <= 0.0 {
        return lengths;
    }

    let mut fixed = vec![false; count];
    loop {
        let mut changed = false;
        let mut fixed_total = 0.0;
        let mut flexible_share_total = 0.0;
        for (index, length) in lengths.iter_mut().enumerate() {
            if fixed[index] {
                fixed_total += *length;
            } else if *length < min_major {
                *length = min_major;
                fixed[index] = true;
                fixed_total += min_major;
                changed = true;
            } else {
                flexible_share_total += shares[index];
            }
        }
        if !changed {
            break;
        }
        let remaining = (total - fixed_total).max(0.0);
        if flexible_share_total <= 0.0 {
            break;
        }
        for (index, length) in lengths.iter_mut().enumerate() {
            if !fixed[index] {
                *length = remaining * (shares[index] / flexible_share_total);
            }
        }
    }

    lengths
}

pub(crate) fn repaired_shares(count: usize, shares: &[f64]) -> Vec<f64> {
    if count == 0 {
        return Vec::new();
    }
    if shares.len() != count {
        return vec![1.0; count];
    }
    let mut repaired = Vec::with_capacity(count);
    let mut valid = false;
    for share in shares {
        if share.is_finite() && *share > 0.0 {
            valid = true;
            repaired.push(*share);
        } else {
            repaired.push(1.0);
        }
    }
    if valid { repaired } else { vec![1.0; count] }
}

pub(crate) fn is_valid_split_fraction(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value < 1.0
}

pub(crate) fn rect_distance(rect: Rect, point: Point) -> f64 {
    debug_assert!(rect.is_finite(), "rectangle must be finite");
    debug_assert!(point.is_finite(), "point must be finite");
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
