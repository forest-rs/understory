// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

use peniko::color::{self, ColorSpaceTag, DynamicColor, HueDirection, Srgb};
use peniko::{Color, InterpolationAlphaSpace};
use understory_timing::{TimerDuration, TimerInstant};

use crate::TimingFunction;
use crate::transition::normalized_progress;
use crate::value::f32_progress;

/// Policy for interpolating colors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorInterpolation {
    color_space: ColorSpaceTag,
    hue_direction: HueDirection,
    alpha_space: InterpolationAlphaSpace,
}

impl ColorInterpolation {
    /// Interpolate in sRGB with premultiplied alpha.
    pub const SRGB: Self = Self::new(
        ColorSpaceTag::Srgb,
        HueDirection::Shorter,
        InterpolationAlphaSpace::Premultiplied,
    );

    /// Interpolate in Oklab with premultiplied alpha.
    pub const OKLAB: Self = Self::new(
        ColorSpaceTag::Oklab,
        HueDirection::Shorter,
        InterpolationAlphaSpace::Premultiplied,
    );

    /// Creates a color interpolation policy.
    #[must_use]
    pub const fn new(
        color_space: ColorSpaceTag,
        hue_direction: HueDirection,
        alpha_space: InterpolationAlphaSpace,
    ) -> Self {
        Self {
            color_space,
            hue_direction,
            alpha_space,
        }
    }

    /// Returns the color space used for interpolation.
    #[must_use]
    pub const fn color_space(self) -> ColorSpaceTag {
        self.color_space
    }

    /// Returns the hue direction used for cylindrical color spaces.
    #[must_use]
    pub const fn hue_direction(self) -> HueDirection {
        self.hue_direction
    }

    /// Returns how alpha participates in interpolation.
    #[must_use]
    pub const fn alpha_space(self) -> InterpolationAlphaSpace {
        self.alpha_space
    }

    /// Interpolates between `from` and `to` using this policy.
    #[must_use]
    pub fn interpolate(self, from: Color, to: Color, progress: f64) -> Color {
        if progress <= 0.0 {
            return from;
        }
        if progress >= 1.0 {
            return to;
        }
        ColorRamp::new(from, to, self).sample(progress)
    }
}

impl Default for ColorInterpolation {
    fn default() -> Self {
        Self::SRGB
    }
}

/// A color transition clip with explicit interpolation policy.
#[derive(Clone, Copy)]
pub struct ColorTransition {
    from: Color,
    to: Color,
    started_at: TimerInstant,
    duration: TimerDuration,
    timing: TimingFunction,
    interpolation: ColorInterpolation,
    ramp: ColorRamp,
}

impl ColorTransition {
    /// Creates a color transition.
    #[must_use]
    pub fn new(
        from: Color,
        to: Color,
        started_at: TimerInstant,
        duration: TimerDuration,
        timing: TimingFunction,
        interpolation: ColorInterpolation,
    ) -> Self {
        Self {
            from,
            to,
            started_at,
            duration,
            timing,
            interpolation,
            ramp: ColorRamp::new(from, to, interpolation),
        }
    }

    /// Returns the transition start color.
    #[must_use]
    pub const fn from(&self) -> Color {
        self.from
    }

    /// Returns the transition target color.
    #[must_use]
    pub const fn to(&self) -> Color {
        self.to
    }

    /// Returns the transition start instant.
    #[must_use]
    pub const fn started_at(&self) -> TimerInstant {
        self.started_at
    }

    /// Returns the transition duration.
    #[must_use]
    pub const fn duration(&self) -> TimerDuration {
        self.duration
    }

    /// Returns the timing function.
    #[must_use]
    pub const fn timing(&self) -> TimingFunction {
        self.timing
    }

    /// Returns the color interpolation policy.
    #[must_use]
    pub const fn interpolation(&self) -> ColorInterpolation {
        self.interpolation
    }

    /// Samples this color transition at `now`.
    #[must_use]
    pub fn sample(self, now: TimerInstant) -> Color {
        let progress = self
            .timing
            .sample(normalized_progress(now, self.started_at, self.duration));
        if progress <= 0.0 {
            return self.from;
        }
        if progress >= 1.0 {
            return self.to;
        }
        self.ramp.sample(progress)
    }

    /// Returns whether this transition has reached its target by `now`.
    #[must_use]
    pub fn is_complete(self, now: TimerInstant) -> bool {
        now.saturating_sub(self.started_at) >= self.duration
    }

    /// Returns the instant this transition reaches its target.
    #[must_use]
    pub fn end_time(self) -> TimerInstant {
        self.started_at.saturating_add(self.duration)
    }
}

impl fmt::Debug for ColorTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ColorTransition")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("started_at", &self.started_at)
            .field("duration", &self.duration)
            .field("timing", &self.timing)
            .field("interpolation", &self.interpolation)
            .finish_non_exhaustive()
    }
}

impl PartialEq for ColorTransition {
    fn eq(&self, other: &Self) -> bool {
        self.from == other.from
            && self.to == other.to
            && self.started_at == other.started_at
            && self.duration == other.duration
            && self.timing == other.timing
            && self.interpolation == other.interpolation
    }
}

#[derive(Clone, Copy)]
enum ColorRamp {
    Premultiplied(color::Interpolator),
    Unpremultiplied(color::UnpremultipliedInterpolator),
}

impl ColorRamp {
    fn new(from: Color, to: Color, interpolation: ColorInterpolation) -> Self {
        let from = DynamicColor::from_alpha_color(from);
        let to = DynamicColor::from_alpha_color(to);
        match interpolation.alpha_space() {
            InterpolationAlphaSpace::Premultiplied => Self::Premultiplied(from.interpolate(
                to,
                interpolation.color_space(),
                interpolation.hue_direction(),
            )),
            InterpolationAlphaSpace::Unpremultiplied => {
                Self::Unpremultiplied(from.interpolate_unpremultiplied(
                    to,
                    interpolation.color_space(),
                    interpolation.hue_direction(),
                ))
            }
        }
    }

    fn sample(self, progress: f64) -> Color {
        let progress = f32_progress(progress);
        match self {
            Self::Premultiplied(ramp) => ramp.eval(progress),
            Self::Unpremultiplied(ramp) => ramp.eval(progress),
        }
        .to_alpha_color::<Srgb>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_transition_uses_color_interpolation_policy() {
        let from = Color::from_rgba8(0xff, 0x00, 0x00, 0xff);
        let to = Color::from_rgba8(0x00, 0xff, 0x00, 0xff);
        let transition = ColorTransition::new(
            from,
            to,
            0,
            100,
            TimingFunction::LINEAR,
            ColorInterpolation::OKLAB,
        );

        assert_eq!(transition.sample(0), from);
        assert_eq!(transition.sample(100), to);
        assert_ne!(
            transition.sample(50),
            ColorInterpolation::SRGB.interpolate(from, to, 0.5)
        );
    }
}
