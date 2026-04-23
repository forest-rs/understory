// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text block widget for multiline wrapped text.

use alloc::{boxed::Box, vec::Vec};

use kurbo::Size;
use peniko::Brush;
use understory_display::{DisplayNode, Insets};

use crate::{
    AppendSpec, ElementId, MeasureCtx, MeasureStyle, MessageClass, ResolvedElement, Ui, Widget,
    compose, text_label_node,
};

/// Multiline wrapped text block widget.
///
/// Renders its label as top-left aligned, uniformly padded text that wraps
/// at the container width. Height is estimated from the label length and
/// font size during scene layout.
#[derive(Clone, Debug, Default)]
pub struct TextBlock {
    text: Box<str>,
    mount: compose::ElementOptions,
}

impl TextBlock {
    /// Creates a new text block widget.
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: Box::from(""),
            mount: compose::ElementOptions::default(),
        }
    }

    /// Creates a text block with initial text.
    #[must_use]
    pub fn with_text(mut self, text: impl Into<Box<str>>) -> Self {
        self.text = text.into();
        self
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

    /// Fills the remaining parent-axis space when supported by the parent.
    #[must_use]
    pub fn fill(mut self) -> Self {
        self.mount = self.mount.fill(true);
        self
    }

    /// Sets uniform inner padding.
    #[must_use]
    pub fn padding(mut self, padding: f64) -> Self {
        self.mount = self.mount.padding(padding);
        self
    }

    /// Sets uniform label padding.
    #[must_use]
    pub fn label_padding(mut self, label_padding: f64) -> Self {
        self.mount = self.mount.label_padding(label_padding);
        self
    }

    /// Sets the font size.
    #[must_use]
    pub fn font_size(mut self, font_size: f64) -> Self {
        self.mount = self.mount.font_size(font_size);
        self
    }

    /// Sets the background color.
    #[must_use]
    pub fn background(mut self, background: crate::Color) -> Self {
        self.mount = self.mount.background(background);
        self
    }

    /// Sets visibility.
    #[must_use]
    pub fn visible(mut self, visible: bool) -> Self {
        self.mount = self.mount.visible(visible);
        self
    }

    /// Sets a corner radius.
    #[must_use]
    pub fn corner_radius(mut self, corner_radius: f64) -> Self {
        self.mount = self.mount.corner_radius(corner_radius);
        self
    }

    /// Applies the built-in user-message class.
    #[must_use]
    pub fn user_message(mut self) -> Self {
        self.mount = self.mount.class(MessageClass::User.class_id());
        self
    }

    /// Sets a display name for inspectors/debug views.
    #[must_use]
    pub fn display_name(mut self, display_name: impl Into<Box<str>>) -> Self {
        self.mount = self.mount.display_name(display_name);
        self
    }
}

impl AppendSpec for TextBlock {
    fn append_to(mut self, ui: &mut Ui, parent: ElementId) -> ElementId {
        let mount = core::mem::take(&mut self.mount);
        compose::append_widget_spec(ui, parent, crate::TYPE_TEXT_BLOCK, self, mount)
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
