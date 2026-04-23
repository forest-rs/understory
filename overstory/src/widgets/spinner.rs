// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Timer-driven loading spinner widget.

use alloc::vec::Vec;
use core::f64::consts::FRAC_1_SQRT_2;

use kurbo::{Size, Vec2};
use peniko::{Brush, Color};
use understory_display::{DisplayAlign, DisplayNode, Insets};

use crate::{
    AppendSpec, ElementId, MeasureCtx, MeasureStyle, ResolvedElement, TimerId, TimerQueue, Ui,
    Widget, compose, content_box,
};

const STEP_INTERVAL_NANOS: u64 = 90_000_000;
const DOT_COUNT: u8 = 8;
const DOT_VECTORS: [(f64, f64); 8] = [
    (0.0, -1.0),
    (FRAC_1_SQRT_2, -FRAC_1_SQRT_2),
    (1.0, 0.0),
    (FRAC_1_SQRT_2, FRAC_1_SQRT_2),
    (0.0, 1.0),
    (-FRAC_1_SQRT_2, FRAC_1_SQRT_2),
    (-1.0, 0.0),
    (-FRAC_1_SQRT_2, -FRAC_1_SQRT_2),
];
const DOT_ALPHAS: [u8; 8] = [255, 208, 168, 132, 100, 72, 48, 28];

/// Animated loading indicator built from retained display nodes.
#[derive(Clone, Debug)]
pub struct Spinner {
    size: f64,
    phase: u8,
    timer: Option<TimerId>,
    mount: compose::ElementOptions,
}

impl Default for Spinner {
    fn default() -> Self {
        Self {
            size: 20.0,
            phase: 0,
            timer: None,
            mount: compose::ElementOptions::default(),
        }
    }
}

impl Spinner {
    /// Creates a spinner with the given square size.
    #[must_use]
    pub fn new(size: f64) -> Self {
        Self {
            size: size.max(12.0),
            ..Self::default()
        }
    }

    /// Sets visibility.
    #[must_use]
    pub fn visible(mut self, visible: bool) -> Self {
        self.mount = self.mount.visible(visible);
        self
    }

    /// Returns `true` when the spinner is actively animating.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.timer.is_some()
    }

    /// Starts the animation timer if needed.
    pub fn start(&mut self, timers: &mut TimerQueue, element_id: ElementId, now: u64) {
        if self.timer.is_none() {
            self.timer = Some(timers.request(
                element_id,
                now,
                STEP_INTERVAL_NANOS,
                Some(STEP_INTERVAL_NANOS),
            ));
        }
    }

    /// Stops the animation timer and resets the visible phase.
    pub fn stop(&mut self, timers: &mut TimerQueue) {
        if let Some(id) = self.timer.take() {
            timers.cancel(id);
        }
        self.phase = 0;
    }

    fn alpha_for_dot(&self, index: usize, base_alpha: u8) -> u8 {
        let lead = usize::from(self.phase) % DOT_ALPHAS.len();
        let step = (DOT_ALPHAS.len() + index - lead) % DOT_ALPHAS.len();
        let scaled = (u16::from(base_alpha) * u16::from(DOT_ALPHAS[step])) / 255;
        u8::try_from(scaled).expect("alpha scale stays within u8 range")
    }

    fn dot_node(&self, resolved: &ResolvedElement, index: usize) -> DisplayNode {
        let dot = (self.size / 4.8).max(3.0);
        let radius = (self.size * 0.5 - dot).max(0.0);
        let center = self.size * 0.5 - dot * 0.5;
        let (ux, uy) = DOT_VECTORS[index];
        let offset = Vec2::new(center + radius * ux, center + radius * uy);
        let rgba = resolved.foreground.to_rgba8();
        let brush = Brush::Solid(Color::from_rgba8(
            rgba.r,
            rgba.g,
            rgba.b,
            self.alpha_for_dot(index, rgba.a),
        ));
        DisplayNode::offset(
            offset,
            DisplayNode::fixed_frame(
                Size::new(dot, dot),
                DisplayNode::fill_rounded_rect(dot * 0.5, brush),
            ),
        )
    }
}

impl AppendSpec for Spinner {
    fn append_to(mut self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let mount = core::mem::take(&mut self.mount);
        compose::append_widget_spec(ui, parent, crate::TYPE_SPINNER, self, mount)
    }
}

impl Widget for Spinner {
    fn measure(
        &self,
        _available: Size,
        _style: &MeasureStyle<'_>,
        _ctx: &mut MeasureCtx<'_>,
    ) -> Option<Size> {
        Some(Size::new(self.size, self.size))
    }

    fn display(&self, _id: ElementId, resolved: &ResolvedElement, children: &mut Vec<DisplayNode>) {
        let mut dots = Vec::with_capacity(DOT_VECTORS.len());
        for index in 0..DOT_VECTORS.len() {
            dots.push(self.dot_node(resolved, index));
        }
        children.push(content_box(
            DisplayNode::fixed_frame(Size::new(self.size, self.size), DisplayNode::stack(dots)),
            DisplayAlign::Center,
            DisplayAlign::Center,
            Insets::uniform(0.0),
        ));
    }

    fn on_timer(&mut self, id: TimerId, _now: u64) {
        if self.timer == Some(id) {
            self.phase = (self.phase + 1) % DOT_COUNT;
        }
    }

    crate::impl_widget_any!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ElementId;

    #[test]
    fn start_and_stop_manage_timer_lifecycle() {
        let mut spinner = Spinner::default();
        let mut timers = TimerQueue::new();
        spinner.start(&mut timers, ElementId::new(0), 10);
        assert!(spinner.is_running());
        assert!(timers.next_deadline().is_some());

        spinner.stop(&mut timers);
        assert!(!spinner.is_running());
        assert!(timers.is_empty());
    }

    #[test]
    fn timer_advances_spinner_phase() {
        let mut spinner = Spinner::default();
        let mut timers = TimerQueue::new();
        spinner.start(&mut timers, ElementId::new(0), 0);
        let timer = spinner.timer.expect("spinner timer");
        spinner.on_timer(timer, STEP_INTERVAL_NANOS);
        assert_eq!(spinner.phase, 1);
    }
}
