// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text block widget for multiline wrapped text.

use alloc::{boxed::Box, vec::Vec};

use kurbo::Size;
use peniko::Brush;
use understory_display::{DisplayNode, Insets};

use crate::{ElementId, MeasureCtx, MeasureStyle, ResolvedElement, Widget, text_label_node};

/// Multiline wrapped text block widget.
///
/// Renders its label as top-left aligned, uniformly padded text that wraps
/// at the container width. Height is estimated from the label length and
/// font size during scene layout.
#[derive(Clone, Debug, Default)]
pub struct TextBlock {
    text: Box<str>,
}

impl TextBlock {
    /// Creates a new text block widget.
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: Box::from(""),
        }
    }

    /// Returns the text block content.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        (!self.text.is_empty()).then_some(self.text.as_ref())
    }

    /// Replaces the text block content.
    pub fn set_text(&mut self, text: impl Into<Box<str>>) {
        self.text = text.into();
    }
}

impl Widget for TextBlock {
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
        children.push(DisplayNode::padding(
            Insets::uniform(resolved.label_padding),
            text_node,
        ));
    }

    crate::impl_widget_any!();
}
