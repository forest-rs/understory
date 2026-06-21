// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::f64::consts::E;

#[cfg(all(not(test), not(feature = "std")))]
use kurbo::common::FloatFuncs;
use understory_timing::TimerDuration;

/// Physical spring parameters for one-dimensional sampling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// Spring stiffness coefficient.
    pub stiffness: f64,
    /// Damping coefficient.
    pub damping: f64,
    /// Simulated mass.
    pub mass: f64,
    /// Velocity below which the spring may be considered at rest.
    pub rest_speed: f64,
    /// Distance below which the spring may be considered at rest.
    pub rest_delta: f64,
}

impl Spring {
    /// Creates spring parameters.
    #[must_use]
    pub const fn new(
        stiffness: f64,
        damping: f64,
        mass: f64,
        rest_speed: f64,
        rest_delta: f64,
    ) -> Self {
        Self {
            stiffness,
            damping,
            mass,
            rest_speed,
            rest_delta,
        }
    }

    /// A responsive default for direct manipulation and UI feedback.
    #[must_use]
    pub const fn snappy() -> Self {
        Self::new(520.0, 38.0, 1.0, 0.01, 0.01)
    }

    /// Samples scalar spring motion at `elapsed`.
    ///
    /// `from` and `to` are position values. `initial_velocity` is expressed in
    /// value units per second.
    #[must_use]
    pub fn sample_scalar(
        self,
        from: f64,
        to: f64,
        initial_velocity: f64,
        elapsed: TimerDuration,
    ) -> SpringSample {
        if !self.is_solvable() {
            return SpringSample {
                value: to,
                velocity: 0.0,
            };
        }

        let seconds = seconds(elapsed);
        let displacement = from - to;
        let natural = (self.stiffness / self.mass).sqrt();
        let damping_ratio = self.damping / (2.0 * (self.stiffness * self.mass).sqrt());
        let (offset, velocity) = solve_damped_oscillator(
            displacement,
            initial_velocity,
            seconds,
            natural,
            damping_ratio,
        );

        SpringSample {
            value: to + offset,
            velocity,
        }
    }

    /// Returns whether `sample` is close enough to the target to stop.
    #[must_use]
    pub fn is_at_rest(self, sample: SpringSample, target: f64) -> bool {
        (sample.velocity.abs() <= self.rest_speed)
            && ((sample.value - target).abs() <= self.rest_delta)
    }

    fn is_solvable(self) -> bool {
        self.stiffness.is_finite()
            && self.damping.is_finite()
            && self.mass.is_finite()
            && self.stiffness > 0.0
            && self.damping >= 0.0
            && self.mass > 0.0
    }
}

impl Default for Spring {
    fn default() -> Self {
        Self::snappy()
    }
}

/// A sampled spring value and velocity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringSample {
    /// Sampled scalar value.
    pub value: f64,
    /// Sampled velocity in value units per second.
    pub velocity: f64,
}

/// Exponential scalar decay parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decay {
    /// Time constant controlling how quickly velocity fades.
    pub time_constant: TimerDuration,
    /// Velocity below which the decay may be considered at rest.
    pub rest_speed: f64,
}

impl Decay {
    /// Creates decay parameters.
    #[must_use]
    pub const fn new(time_constant: TimerDuration, rest_speed: f64) -> Self {
        Self {
            time_constant,
            rest_speed,
        }
    }

    /// A conservative default suitable for pointer-release inertia.
    #[must_use]
    pub const fn pointer_inertia() -> Self {
        Self::new(325_000_000, 0.01)
    }

    /// Samples scalar decay at `elapsed`.
    ///
    /// `initial_velocity` is expressed in value units per second.
    #[must_use]
    pub fn sample_scalar(
        self,
        from: f64,
        initial_velocity: f64,
        elapsed: TimerDuration,
    ) -> DecaySample {
        if self.time_constant == 0 || !initial_velocity.is_finite() {
            return DecaySample {
                value: from,
                velocity: 0.0,
            };
        }

        let tau = seconds(self.time_constant);
        let t = seconds(elapsed);
        let decay = exp(-t / tau);
        DecaySample {
            value: from + (initial_velocity * tau * (1.0 - decay)),
            velocity: initial_velocity * decay,
        }
    }

    /// Returns whether `sample` is slow enough to stop.
    #[must_use]
    pub fn is_at_rest(self, sample: DecaySample) -> bool {
        sample.velocity.abs() <= self.rest_speed
    }
}

impl Default for Decay {
    fn default() -> Self {
        Self::pointer_inertia()
    }
}

/// A sampled decay value and velocity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecaySample {
    /// Sampled scalar value.
    pub value: f64,
    /// Sampled velocity in value units per second.
    pub velocity: f64,
}

fn solve_damped_oscillator(
    displacement: f64,
    velocity: f64,
    seconds: f64,
    natural: f64,
    damping_ratio: f64,
) -> (f64, f64) {
    const CRITICAL_EPSILON: f64 = 1.0e-6;

    if damping_ratio < 1.0 - CRITICAL_EPSILON {
        let damped = natural * (1.0 - damping_ratio * damping_ratio).sqrt();
        let envelope = exp(-damping_ratio * natural * seconds);
        let a = displacement;
        let b = (velocity + (damping_ratio * natural * displacement)) / damped;
        let cos = (damped * seconds).cos();
        let sin = (damped * seconds).sin();
        let offset = envelope * ((a * cos) + (b * sin));
        let velocity = envelope
            * ((-damping_ratio * natural * ((a * cos) + (b * sin)))
                + ((-a * damped * sin) + (b * damped * cos)));
        (offset, velocity)
    } else if damping_ratio <= 1.0 + CRITICAL_EPSILON {
        let envelope = exp(-natural * seconds);
        let b = velocity + (natural * displacement);
        let offset = envelope * (displacement + (b * seconds));
        let velocity = envelope * (b - (natural * (displacement + (b * seconds))));
        (offset, velocity)
    } else {
        let root = (damping_ratio * damping_ratio - 1.0).sqrt();
        let r1 = -natural * (damping_ratio - root);
        let r2 = -natural * (damping_ratio + root);
        let c1 = (velocity - (r2 * displacement)) / (r1 - r2);
        let c2 = displacement - c1;
        let e1 = exp(r1 * seconds);
        let e2 = exp(r2 * seconds);
        ((c1 * e1) + (c2 * e2), (c1 * r1 * e1) + (c2 * r2 * e2))
    }
}

fn seconds(duration: TimerDuration) -> f64 {
    duration as f64 / 1_000_000_000.0
}

fn exp(value: f64) -> f64 {
    E.powf(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_samples_toward_target_and_rest() {
        let spring = Spring::new(120.0, 28.0, 1.0, 0.01, 0.01);
        let start = spring.sample_scalar(0.0, 100.0, 0.0, 0);
        let later = spring.sample_scalar(0.0, 100.0, 0.0, 1_000_000_000);

        assert_eq!(start.value, 0.0);
        assert!(later.value > 95.0);
        assert!(spring.is_at_rest(spring.sample_scalar(0.0, 100.0, 0.0, 4_000_000_000), 100.0));
    }

    #[test]
    fn decay_preserves_absolute_time_sampling() {
        let decay = Decay::new(500_000_000, 0.01);
        let start = decay.sample_scalar(0.0, 100.0, 0);
        let later = decay.sample_scalar(0.0, 100.0, 500_000_000);

        assert_eq!(start.value, 0.0);
        assert!(later.value > 0.0);
        assert!(later.velocity < start.velocity);
    }
}
