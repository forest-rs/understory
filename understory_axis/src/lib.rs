// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Understory Axis: headless numeric axis scale and tick primitives.
//!
//! This crate focuses on one narrow concern: deriving stable, "nice" 1D tick
//! positions from a continuous numeric axis.
//!
//! It owns:
//! - major / medium / minor tick selection
//! - 1-2-5 step sizing
//! - label eligibility decisions based on spacing thresholds
//!
//! It does not own:
//! - domain-specific label formatting
//! - time units or dates
//! - viewport transforms
//! - rendering or text layout
//!
//! The intended split is:
//! - a caller supplies world-units-per-pixel and a visible numeric range
//! - this crate returns tick positions plus their semantic kind
//! - the caller formats tick labels appropriate to its own domain
//!
//! ## Minimal example
//!
//! ```rust
//! use understory_axis::{AxisScale1D, AxisScaleOptions, AxisTickKind};
//!
//! let scale = AxisScale1D::with_options(
//!     0.5,
//!     AxisScaleOptions {
//!         target_major_spacing_px: 100.0,
//!         min_major_step: 0.0,
//!         medium_label_min_spacing_px: 220.0,
//!     },
//! );
//!
//! let ticks = scale.ticks_in_range(0.0..100.0);
//! assert!(ticks.iter().any(|tick| tick.kind == AxisTickKind::Major && tick.labeled));
//! ```
//!
//! This crate is `no_std` and uses `alloc`.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::ops::Range;

/// Semantic classification for an axis tick.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AxisTickKind {
    /// Primary grid/tick mark.
    Major,
    /// Secondary subdivision that may optionally be labeled.
    Medium,
    /// Fine subdivision without labeling.
    Minor,
}

/// A single axis tick position plus label eligibility.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AxisTick {
    /// Tick coordinate in caller-defined world units.
    pub value: f64,
    /// Semantic tick kind.
    pub kind: AxisTickKind,
    /// Whether a higher layer should consider labeling this tick.
    pub labeled: bool,
}

/// Options controlling automatic 1D axis scale derivation.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AxisScaleOptions {
    /// Desired spacing in pixels between major ticks.
    pub target_major_spacing_px: f64,
    /// Lower bound for the major tick step in world units.
    pub min_major_step: f64,
    /// Minimum major spacing in pixels before medium ticks become label-eligible.
    pub medium_label_min_spacing_px: f64,
}

impl Default for AxisScaleOptions {
    fn default() -> Self {
        Self {
            target_major_spacing_px: 96.0,
            min_major_step: 0.0,
            medium_label_min_spacing_px: 220.0,
        }
    }
}

/// A derived 1D axis scale over a continuous numeric domain.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AxisScale1D {
    major_step: f64,
    minor_step: f64,
    subdivisions: usize,
    medium_interval: Option<usize>,
    medium_labels: bool,
}

impl AxisScale1D {
    /// Derive a scale from a world-units-per-pixel ratio using default options.
    #[must_use]
    pub fn new(world_units_per_pixel: f64) -> Self {
        Self::with_options(world_units_per_pixel, AxisScaleOptions::default())
    }

    /// Derive a scale from a world-units-per-pixel ratio and explicit options.
    #[must_use]
    pub fn with_options(world_units_per_pixel: f64, options: AxisScaleOptions) -> Self {
        let target_major_step = world_units_per_pixel.abs() * options.target_major_spacing_px;
        let major_step = choose_step(target_major_step.max(options.min_major_step).max(1e-12));
        let subdivisions = subdivisions_for_step(major_step);
        let minor_step = major_step / subdivisions as f64;
        let medium_interval = if subdivisions.is_multiple_of(2) {
            Some(subdivisions / 2)
        } else {
            None
        };
        let major_spacing_px = major_step / world_units_per_pixel.abs().max(f64::MIN_POSITIVE);
        let medium_labels = major_spacing_px >= options.medium_label_min_spacing_px;

        Self {
            major_step,
            minor_step,
            subdivisions,
            medium_interval,
            medium_labels,
        }
    }

    /// Returns the derived major step in world units.
    #[must_use]
    pub fn major_step(&self) -> f64 {
        self.major_step
    }

    /// Returns the derived minor step in world units.
    #[must_use]
    pub fn minor_step(&self) -> f64 {
        self.minor_step
    }

    /// Returns ticks covering the provided visible range plus one minor step on each side.
    #[must_use]
    pub fn ticks_in_range(&self, visible: Range<f64>) -> Vec<AxisTick> {
        let start_index = floor_to_i64(visible.start / self.minor_step) - 1;
        let end_index = ceil_to_i64(visible.end / self.minor_step) + 1;
        let mut ticks = Vec::new();

        for index in start_index..=end_index {
            let value = index as f64 * self.minor_step;
            if value < visible.start - self.minor_step || value > visible.end + self.minor_step {
                continue;
            }
            let sub_index = usize::try_from(index.rem_euclid(self.subdivisions as i64))
                .expect("rem_euclid stays within subdivision count");
            let (kind, labeled) = if sub_index == 0 {
                (AxisTickKind::Major, true)
            } else if self
                .medium_interval
                .is_some_and(|interval| sub_index.is_multiple_of(interval))
            {
                (AxisTickKind::Medium, self.medium_labels)
            } else {
                (AxisTickKind::Minor, false)
            };
            ticks.push(AxisTick {
                value,
                kind,
                labeled,
            });
        }

        ticks
    }
}

fn choose_step(target: f64) -> f64 {
    let mut unit = 1.0_f64;
    if target >= 1.0 {
        while unit * 10.0 <= target {
            unit *= 10.0;
        }
    } else {
        while unit > target {
            unit /= 10.0;
        }
    }

    for mantissa in [1.0_f64, 2.0, 5.0, 10.0] {
        let step = mantissa * unit;
        if step >= target {
            return step;
        }
    }

    10.0 * unit
}

fn subdivisions_for_step(step: f64) -> usize {
    let step = step.abs().max(1e-12);
    let mut scale = 1.0_f64;
    if step >= 1.0 {
        while scale * 10.0 <= step {
            scale *= 10.0;
        }
    } else {
        while scale > step {
            scale /= 10.0;
        }
    }
    let normalized = step / scale;
    if normalized <= 1.0 + 1e-6 {
        10
    } else if normalized <= 2.0 + 1e-6 {
        4
    } else {
        5
    }
}

fn floor_to_i64(value: f64) -> i64 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "deliberate truncation step for small axis tick indexing"
    )]
    let truncated = value as i64;
    if (truncated as f64) > value {
        truncated - 1
    } else {
        truncated
    }
}

fn ceil_to_i64(value: f64) -> i64 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "deliberate truncation step for small axis tick indexing"
    )]
    let truncated = value as i64;
    if (truncated as f64) < value {
        truncated + 1
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::{AxisScale1D, AxisScaleOptions, AxisTickKind};

    #[test]
    fn larger_world_units_produce_larger_major_steps() {
        let coarse = AxisScale1D::new(2.0);
        let fine = AxisScale1D::new(0.2);
        assert!(coarse.major_step() > fine.major_step());
    }

    #[test]
    fn medium_ticks_can_be_label_eligible() {
        let scale = AxisScale1D::with_options(
            0.05,
            AxisScaleOptions {
                target_major_spacing_px: 320.0,
                min_major_step: 0.0,
                medium_label_min_spacing_px: 220.0,
            },
        );
        let ticks = scale.ticks_in_range(0.0..100.0);
        assert!(
            ticks
                .iter()
                .any(|tick| tick.kind == AxisTickKind::Medium && tick.labeled)
        );
    }

    #[test]
    fn ticks_cover_requested_range() {
        let scale = AxisScale1D::new(0.5);
        let ticks = scale.ticks_in_range(10.0..40.0);
        assert!(!ticks.is_empty());
        assert!(ticks.iter().any(|tick| tick.value <= 10.0));
        assert!(ticks.iter().any(|tick| tick.value >= 40.0));
    }
}
