// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tooltip widget — promoted overlay surface with text content.

use alloc::{borrow::Cow, boxed::Box, vec::Vec};

use kurbo::Size;
use peniko::Brush;
use understory_display::{DisplayAlign, DisplayNode, Insets};

use crate::{
    AppendSpec, ElementId, MeasureCtx, MeasureStyle, ResolvedElement, SemanticRole, SurfaceRole,
    Ui, Widget, compose,
};

/// Tooltip-specific padding (tighter than the theme default).
const TOOLTIP_PADDING: f64 = 6.0;
/// Tooltip-specific font size (smaller than the theme default).
const TOOLTIP_FONT_SIZE: f64 = 13.0;

/// Tooltip widget that renders as a promoted overlay surface.
///
/// Associates with a trigger element. The tooltip is visible only when
/// the trigger is hovered, and positions itself below the trigger's rect.
/// Use [`crate::Ui::update_tooltips`] to drive visibility and positioning.
#[derive(Clone, Debug)]
pub struct Tooltip {
    trigger: ElementId,
    visible: bool,
    text: Box<str>,
    /// Desired position in root coordinates (set by `update_tooltips`).
    position: Option<kurbo::Point>,
    mount: compose::ElementOptions,
}

impl Tooltip {
    /// Creates a tooltip widget associated with a trigger element.
    #[must_use]
    pub fn new(trigger: ElementId) -> Self {
        Self {
            trigger,
            visible: false,
            text: Box::from(""),
            position: None,
            mount: compose::ElementOptions::default(),
        }
    }

    /// Returns the trigger element this tooltip is associated with.
    #[must_use]
    pub fn trigger(&self) -> ElementId {
        self.trigger
    }

    /// Returns whether the tooltip is currently visible.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Sets the tooltip visibility.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Returns the desired position in root coordinates.
    #[must_use]
    pub fn position(&self) -> Option<kurbo::Point> {
        self.position
    }

    /// Sets the desired position in root coordinates.
    pub fn set_position(&mut self, position: kurbo::Point) {
        self.position = Some(position);
    }

    /// Returns the tooltip text.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        (!self.text.is_empty()).then_some(self.text.as_ref())
    }

    /// Replaces the tooltip text.
    pub fn set_text(&mut self, text: impl Into<Box<str>>) {
        self.text = text.into();
    }

    /// Sets the tooltip text and returns the configured widget.
    #[must_use]
    pub fn with_text(mut self, text: impl Into<Box<str>>) -> Self {
        self.text = text.into();
        self
    }

    /// Sets an explicit width.
    #[must_use]
    pub fn width(mut self, width: f64) -> Self {
        self.mount = self.mount.width(width);
        self
    }

    /// Sets an explicit height.
    #[must_use]
    pub fn height(mut self, height: f64) -> Self {
        self.mount = self.mount.height(height);
        self
    }

    /// Sets corner radius.
    #[must_use]
    pub fn corner_radius(mut self, corner_radius: f64) -> Self {
        self.mount = self.mount.corner_radius(corner_radius);
        self
    }

    /// Sets border width.
    #[must_use]
    pub fn border_width(mut self, border_width: f64) -> Self {
        self.mount = self.mount.border_width(border_width);
        self
    }
}

impl AppendSpec for Tooltip {
    fn append_to(mut self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let mount = core::mem::take(&mut self.mount);
        compose::append_widget_spec(ui, parent, crate::TYPE_TOOLTIP, self, mount)
    }
}

impl Widget for Tooltip {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "Widget measurement uses small display values and Parley APIs take f32."
    )]
    fn measure(
        &self,
        available: Size,
        style: &MeasureStyle<'_>,
        ctx: &mut MeasureCtx<'_>,
    ) -> Option<Size> {
        let text = self.text()?;
        let content_width = (available.width - TOOLTIP_PADDING * 2.0).max(1.0) as f32;
        let text_size = ctx.measure_text(
            text,
            TOOLTIP_FONT_SIZE as f32,
            style.font_family,
            Some(content_width),
        );
        let intrinsic_width = (text_size.width + TOOLTIP_PADDING * 2.0)
            .min(available.width)
            .max(0.0);
        Some(Size::new(
            intrinsic_width,
            text_size.height + TOOLTIP_PADDING * 2.0,
        ))
    }

    fn surface_role(&self) -> Option<SurfaceRole> {
        if self.visible {
            Some(SurfaceRole::Tooltip)
        } else {
            None
        }
    }

    fn display(&self, _id: ElementId, resolved: &ResolvedElement, children: &mut Vec<DisplayNode>) {
        let Some(text) = resolved.text.as_deref() else {
            return;
        };
        if text.is_empty() {
            return;
        }
        #[allow(
            clippy::cast_possible_truncation,
            reason = "Font size is a small positive value; f32 is sufficient."
        )]
        let text_node = DisplayNode::text(
            text,
            Brush::Solid(resolved.foreground),
            TOOLTIP_FONT_SIZE as f32,
            &*resolved.font_family,
            resolved.text_align,
        );
        children.push(crate::content_box(
            text_node,
            DisplayAlign::Start,
            DisplayAlign::Center,
            Insets::uniform(TOOLTIP_PADDING),
        ));
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Tooltip
    }

    fn semantic_name(&self) -> Option<Cow<'_, str>> {
        self.text().map(Cow::Borrowed)
    }

    crate::impl_widget_any!();
}
