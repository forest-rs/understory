// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text block widget for multiline wrapped text.

use alloc::vec::Vec;
use core::any::Any;

use peniko::Brush;
use understory_display::{DisplayNode, Insets};

use understory_style::ResourceKey;

use crate::{Element, ElementId, MessageClass, ResolvedElement, ThemeKeys, Widget};

/// Multiline wrapped text block widget.
///
/// Renders its label as top-left aligned, uniformly padded text that wraps
/// at the container width. Height is estimated from the label length and
/// font size during scene layout.
#[derive(Clone, Debug, Default)]
pub struct TextBlockWidget;

impl TextBlockWidget {
    /// Creates a new text block widget.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Widget for TextBlockWidget {
    fn background_key(&self, element: &Element) -> Option<ResourceKey> {
        if element.classes.contains(MessageClass::User.class_id()) {
            Some(ThemeKeys::BUTTON_BACKGROUND)
        } else {
            None
        }
    }

    fn display(&self, _id: ElementId, resolved: &ResolvedElement, children: &mut Vec<DisplayNode>) {
        let Some(label) = resolved.label.as_deref() else {
            return;
        };
        if label.is_empty() {
            return;
        }
        #[allow(
            clippy::cast_possible_truncation,
            reason = "Font size is a small positive value; f32 is sufficient."
        )]
        let text_node = DisplayNode::text(
            label,
            Brush::Solid(resolved.foreground),
            resolved.font_size as f32,
            &*resolved.font_family,
            resolved.text_align,
        );
        children.push(DisplayNode::padding(
            Insets::uniform(resolved.label_padding),
            text_node,
        ));
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
