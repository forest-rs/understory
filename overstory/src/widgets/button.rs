// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Button widget with centered text content.

use alloc::{boxed::Box, vec::Vec};

use cursor_icon::CursorIcon;
use kurbo::Size;
use peniko::Brush;
use understory_display::{DisplayAlign, DisplayNode, Insets};

use crate::{
    Element, ElementId, MeasureCtx, MeasureStyle, ResolvedElement, Widget, content_box,
    text_label_node,
};

/// Interactive push button widget with horizontally padded, vertically
/// centered label text.
#[derive(Clone, Debug, Default)]
pub struct Button {
    text: Box<str>,
}

impl Button {
    /// Creates a new button widget.
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: Box::from(""),
        }
    }

    /// Returns the button text.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        (!self.text.is_empty()).then_some(self.text.as_ref())
    }

    /// Replaces the button text.
    pub fn set_text(&mut self, text: impl Into<Box<str>>) {
        self.text = text.into();
    }
}

impl Widget for Button {
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
        let content_width = (available.width - style.label_padding * 2.0).max(1.0) as f32;
        let text_size = ctx.measure_text(
            text,
            style.font_size as f32,
            style.font_family,
            Some(content_width),
        );
        let intrinsic_width = (text_size.width + style.label_padding * 2.0)
            .min(available.width)
            .max(0.0);
        Some(Size::new(
            intrinsic_width,
            text_size.height + style.label_padding * 2.0,
        ))
    }

    fn display(&self, _id: ElementId, resolved: &ResolvedElement, children: &mut Vec<DisplayNode>) {
        let Some(text) = resolved.text.as_deref() else {
            return;
        };
        if text.is_empty() {
            return;
        }
        let text_node = text_label_node(text, Brush::Solid(resolved.foreground), resolved);
        children.push(content_box(
            text_node,
            DisplayAlign::Start,
            DisplayAlign::Center,
            Insets::symmetric(resolved.label_padding, 0.0),
        ));
    }

    fn default_pickable(&self) -> bool {
        true
    }

    fn default_focusable(&self) -> bool {
        true
    }

    fn cursor_icon(&self, _element: &Element) -> Option<CursorIcon> {
        Some(CursorIcon::Pointer)
    }

    crate::impl_widget_any!();
}
