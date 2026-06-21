// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Data-driven timing function for normalized transition progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimingFunction {
    kind: TimingFunctionKind,
}

impl TimingFunction {
    /// Constant progress rate.
    pub const LINEAR: Self = Self {
        kind: TimingFunctionKind::Linear,
    };

    /// Creates a cubic Bezier timing function from CSS-style control points.
    ///
    /// The curve always starts at `(0, 0)` and ends at `(1, 1)`. `x1` and `x2`
    /// should be finite values in `0.0..=1.0` so the curve can be inverted from
    /// input progress to output progress.
    #[must_use]
    pub const fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        debug_assert!(
            x1.is_finite() && (0.0 <= x1 && x1 <= 1.0),
            "cubic Bezier x1 must be finite and in 0.0..=1.0"
        );
        debug_assert!(
            x2.is_finite() && (0.0 <= x2 && x2 <= 1.0),
            "cubic Bezier x2 must be finite and in 0.0..=1.0"
        );
        debug_assert!(
            y1.is_finite() && y2.is_finite(),
            "cubic Bezier y control points must be finite"
        );
        Self {
            kind: TimingFunctionKind::CubicBezier(CubicBezierTimingFunction { x1, y1, x2, y2 }),
        }
    }

    /// Samples the timing function at normalized input progress `progress`.
    #[must_use]
    pub fn sample(self, progress: f64) -> f64 {
        match self.kind {
            TimingFunctionKind::Linear => progress.clamp(0.0, 1.0),
            TimingFunctionKind::CubicBezier(curve) => curve.sample(progress),
        }
    }
}

impl Default for TimingFunction {
    fn default() -> Self {
        Self::LINEAR
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TimingFunctionKind {
    Linear,
    CubicBezier(CubicBezierTimingFunction),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CubicBezierTimingFunction {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

impl CubicBezierTimingFunction {
    fn sample(self, progress: f64) -> f64 {
        let progress = progress.clamp(0.0, 1.0);
        if progress <= 0.0 || progress >= 1.0 {
            return progress;
        }
        cubic_value(self.solve_x(progress), self.y1, self.y2)
    }

    fn solve_x(self, x: f64) -> f64 {
        const DERIVATIVE_EPSILON: f64 = 1.0e-6;
        const NEWTON_STEPS: usize = 8;
        const BISECTION_STEPS: usize = 12;

        let mut t = x;
        for _ in 0..NEWTON_STEPS {
            let error = cubic_value(t, self.x1, self.x2) - x;
            let derivative = cubic_derivative(t, self.x1, self.x2);
            if derivative.abs() <= DERIVATIVE_EPSILON {
                break;
            }
            let next = t - (error / derivative);
            if !(0.0..=1.0).contains(&next) {
                break;
            }
            t = next;
        }

        let mut low = 0.0;
        let mut high = 1.0;
        for _ in 0..BISECTION_STEPS {
            let value = cubic_value(t, self.x1, self.x2);
            if (value - x).abs() <= DERIVATIVE_EPSILON {
                return t;
            }
            if value < x {
                low = t;
            } else {
                high = t;
            }
            t = (low + high) * 0.5;
        }
        t
    }
}

fn cubic_value(t: f64, p1: f64, p2: f64) -> f64 {
    let inverse = 1.0 - t;
    (3.0 * inverse * inverse * t * p1) + (3.0 * inverse * t * t * p2) + (t * t * t)
}

fn cubic_derivative(t: f64, p1: f64, p2: f64) -> f64 {
    let inverse = 1.0 - t;
    (3.0 * inverse * inverse * p1) + (6.0 * inverse * t * (p2 - p1)) + (3.0 * t * t * (1.0 - p2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_function_samples_expected_shape() {
        let slow_start = TimingFunction::cubic_bezier(0.55, 0.055, 0.675, 0.19);
        let slow_finish = TimingFunction::cubic_bezier(0.215, 0.61, 0.355, 1.0);

        assert_eq!(TimingFunction::LINEAR.sample(0.5), 0.5);
        assert!(slow_start.sample(0.5) < 0.5);
        assert!(slow_finish.sample(0.5) > 0.5);
    }

    #[test]
    fn timing_function_clamps_input() {
        assert_eq!(TimingFunction::LINEAR.sample(-1.0), 0.0);
        assert_eq!(TimingFunction::LINEAR.sample(2.0), 1.0);
    }

    #[test]
    fn cubic_bezier_allows_y_overshoot() {
        let curve = TimingFunction::cubic_bezier(0.25, -0.5, 0.75, 1.5);

        assert!(curve.sample(0.5).is_finite());
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "cubic Bezier x1 must be finite and in 0.0..=1.0")]
    fn cubic_bezier_rejects_invalid_x_in_debug_builds() {
        let _ = TimingFunction::cubic_bezier(-0.1, 0.0, 0.75, 1.0);
    }
}
