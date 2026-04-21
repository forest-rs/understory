// Copyright 2026 the Overstory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text input widget backed by Parley's `PlainEditor`.

use alloc::vec::Vec;
use core::any::Any;
use core::cell::Cell;

use kurbo::{Point, Rect, Vec2};
use parley::PlainEditor;
use peniko::{Brush, Color};
use understory_display::{DisplayAlign, DisplayNode, Insets, TextEngine};
use ui_events::keyboard::{Key, KeyboardEvent, Modifiers, NamedKey};

use understory_style::ResourceKey;

use crate::{Element, ElementId, Interaction, InteractionBatch, ResolvedElement, ThemeKeys, Widget};

const DEFAULT_FONT_SIZE: f64 = 16.0;
const DEFAULT_LABEL_PADDING: f64 = 12.0;
const DEFAULT_FONT_FAMILY: &str = "sans-serif";

impl core::fmt::Debug for TextInputWidget {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TextInputWidget")
            .field("text_len", &self.editor.raw_text().len())
            .finish_non_exhaustive()
    }
}

/// Text input widget with cursor movement, selection, and word-aware editing.
///
/// Wraps Parley's [`PlainEditor`] and provides keyboard event handling,
/// click-to-position cursor placement, and cursor/selection rendering.
// PlainEditor doesn't implement Debug, so we can't derive it.
pub struct TextInputWidget {
    editor: PlainEditor<Brush>,
    cached_cursor_rect: Option<Rect>,
    cached_selection_rects: Vec<Rect>,
    placeholder: Option<alloc::string::String>,
    /// Last measured content width, used to set editor wrap width in `refresh_layout`.
    last_content_width: Cell<Option<f32>>,
}

impl TextInputWidget {
    /// Creates a new text input widget with the given default font size.
    #[must_use]
    pub fn new(font_size: f32) -> Self {
        Self {
            editor: PlainEditor::new(font_size),
            cached_cursor_rect: None,
            cached_selection_rects: Vec::new(),
            placeholder: None,
            last_content_width: Cell::new(None),
        }
    }

    /// Returns the current text buffer content.
    #[must_use]
    pub fn text(&self) -> &str {
        self.editor.raw_text()
    }

    /// Sets the placeholder text shown when the input is empty and unfocused.
    pub fn set_placeholder(&mut self, placeholder: impl Into<alloc::string::String>) {
        self.placeholder = Some(placeholder.into());
    }

    /// Clears the text buffer and resets the cursor to the start.
    pub fn clear(&mut self, text: &mut TextEngine) {
        self.editor.set_text("");
        let (font_cx, layout_cx) = text.contexts();
        self.editor.driver(font_cx, layout_cx).move_to_text_start();
    }
}

/// Maximum content height before the input stops growing (padding added by scene).
const MAX_HEIGHT: f64 = 100.0;

impl Widget for TextInputWidget {
    fn measure(
        &self,
        available: kurbo::Size,
        ctx: &mut crate::MeasureCtx<'_>,
    ) -> Option<kurbo::Size> {
        // Subtract label_padding to match what display() uses for text wrapping.
        // This must match the Insets::symmetric(label_padding, 0.0) in display().
        let h_padding = DEFAULT_LABEL_PADDING;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "Display coordinates are small positive values."
        )]
        let text_width = (available.width - h_padding * 2.0).max(1.0) as f32;
        // Store for refresh_layout to set editor wrap width.
        self.last_content_width.set(Some(text_width));

        let text = self.editor.raw_text();
        let font_size = self.editor.get_font_size();
        let line_height = f64::from(font_size) * 1.4;
        // Vertical padding matches the element's padding (applied by layout).
        // Use line_height as minimum content height.
        if text.is_empty() {
            return Some(kurbo::Size::new(available.width, line_height + h_padding * 2.0));
        }
        let text_size = ctx.measure_text(text, font_size, "sans-serif", Some(text_width));
        // If text ends with a newline, the cursor is on an empty line below
        // the measured text. Add one line height for that cursor line.
        let trailing_newline_extra = if text.ends_with('\n') {
            line_height
        } else {
            0.0
        };
        let content_h = (text_size.height + trailing_newline_extra).max(line_height);
        let height = (content_h + h_padding * 2.0).min(MAX_HEIGHT);
        Some(kurbo::Size::new(available.width, height))
    }

    fn display(
        &self,
        _id: ElementId,
        resolved: &ResolvedElement,
        children: &mut Vec<DisplayNode>,
    ) {
        // Render the text content.
        let is_empty = resolved.label.as_deref().is_none_or(|l| l.is_empty());
        let show_placeholder = is_empty && self.placeholder.is_some();

        let display_text = if show_placeholder {
            self.placeholder.as_deref()
        } else {
            resolved.label.as_deref()
        };

        if let Some(label) = display_text
            && !label.is_empty()
        {
            let font_size = if resolved.font_size > 0.0 {
                resolved.font_size
            } else {
                DEFAULT_FONT_SIZE
            };
            let label_padding = if resolved.label_padding > 0.0 {
                resolved.label_padding
            } else {
                DEFAULT_LABEL_PADDING
            };
            let font_family = if resolved.font_family.is_empty() {
                DEFAULT_FONT_FAMILY
            } else {
                &resolved.font_family
            };
            let text_brush = if show_placeholder {
                // Dim the placeholder text.
                let fg = resolved.foreground.to_rgba8();
                Brush::Solid(Color::from_rgba8(fg.r, fg.g, fg.b, 100))
            } else {
                Brush::Solid(resolved.foreground)
            };
            #[allow(
                clippy::cast_possible_truncation,
                reason = "Font size is a small positive value; f32 is sufficient."
            )]
            let text_node = DisplayNode::text(
                label,
                text_brush,
                font_size as f32,
                font_family,
                resolved.text_align,
            );
            children.push(DisplayNode::align(
                DisplayAlign::Start,
                DisplayAlign::Center,
                DisplayNode::padding(Insets::symmetric(label_padding, 0.0), text_node),
            ));
        }

        // Render selection and cursor overlays.
        let label_padding = resolved.label_padding;
        let selection_brush = Brush::Solid(Color::from_rgba8(80, 140, 220, 100));
        let cursor_brush = Brush::Solid(resolved.foreground);

        let mut overlay_nodes = Vec::new();
        for sel_rect in &self.cached_selection_rects {
            overlay_nodes.push(DisplayNode::offset(
                Vec2::new(sel_rect.x0, sel_rect.y0),
                DisplayNode::fixed_frame(sel_rect.size(), DisplayNode::fill_rect(selection_brush.clone())),
            ));
        }
        if let Some(cursor) = &self.cached_cursor_rect {
            overlay_nodes.push(DisplayNode::offset(
                Vec2::new(cursor.x0, cursor.y0),
                DisplayNode::fixed_frame(cursor.size(), DisplayNode::fill_rect(cursor_brush)),
            ));
        }
        if !overlay_nodes.is_empty() {
            children.push(DisplayNode::align(
                DisplayAlign::Start,
                DisplayAlign::Center,
                DisplayNode::padding(
                    Insets::symmetric(label_padding, 0.0),
                    DisplayNode::stack(overlay_nodes),
                ),
            ));
        }
    }

    fn keyboard_event(
        &mut self,
        id: ElementId,
        event: &KeyboardEvent,
        text: &mut TextEngine,
        batch: &mut InteractionBatch,
    ) -> bool {
        if !event.state.is_down() {
            return false;
        }

        let (font_cx, layout_cx) = text.contexts();
        let mut driver = self.editor.driver(font_cx, layout_cx);
        let action_mod = event.modifiers.contains(Modifiers::META)
            || event.modifiers.contains(Modifiers::CONTROL);

        match &event.key {
            Key::Character(ch) if action_mod && ch.as_str() == "a" => {
                driver.select_all();
                true
            }
            Key::Character(ch) if action_mod => {
                let _ = ch;
                false
            }
            Key::Character(ch) => {
                driver.insert_or_replace_selection(ch);
                true
            }
            Key::Named(named) => match named {
                NamedKey::Backspace if action_mod => {
                    driver.backdelete_word();
                    true
                }
                NamedKey::Backspace => {
                    driver.backdelete();
                    true
                }
                NamedKey::Delete => {
                    driver.delete();
                    true
                }
                NamedKey::ArrowLeft if action_mod => {
                    driver.move_to_line_start();
                    true
                }
                NamedKey::ArrowRight if action_mod => {
                    driver.move_to_line_end();
                    true
                }
                NamedKey::ArrowLeft if event.modifiers.contains(Modifiers::SHIFT) => {
                    driver.select_left();
                    true
                }
                NamedKey::ArrowRight if event.modifiers.contains(Modifiers::SHIFT) => {
                    driver.select_right();
                    true
                }
                NamedKey::ArrowLeft => {
                    driver.move_left();
                    true
                }
                NamedKey::ArrowRight => {
                    driver.move_right();
                    true
                }
                NamedKey::Home => {
                    driver.move_to_line_start();
                    true
                }
                NamedKey::End => {
                    driver.move_to_line_end();
                    true
                }
                NamedKey::Enter
                    if action_mod || event.modifiers.contains(Modifiers::SHIFT) =>
                {
                    batch.push(Interaction::Submitted(id));
                    false // don't mark layout dirty
                }
                NamedKey::Enter => {
                    driver.insert_or_replace_selection("\n");
                    true
                }
                _ => false,
            },
        }
    }

    fn click(
        &mut self,
        _id: ElementId,
        point: Point,
        resolved: &ResolvedElement,
        text: &mut TextEngine,
    ) {
        let label_padding = resolved.label_padding;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "Parley move_to_point takes f32; display coordinates are small."
        )]
        let local_x = (point.x - resolved.rect.x0 - label_padding) as f32;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "Parley move_to_point takes f32; display coordinates are small."
        )]
        let local_y = (point.y - resolved.rect.y0) as f32;
        let (font_cx, layout_cx) = text.contexts();
        self.editor.driver(font_cx, layout_cx).move_to_point(local_x, local_y);
    }

    fn refresh_layout(&mut self, text: &mut TextEngine) {
        // Apply the wrap width from the last measure pass.
        if let Some(w) = self.last_content_width.get() {
            self.editor.set_width(Some(w));
        }
        let (font_cx, layout_cx) = text.contexts();
        self.editor.refresh_layout(font_cx, layout_cx);
        self.cached_cursor_rect = self
            .editor
            .cursor_geometry(2.0)
            .map(|bb| Rect::new(bb.x0, bb.y0, bb.x1, bb.y1));
        let mut rects = Vec::new();
        self.editor.selection_geometry_with(|bb, _line| {
            rects.push(Rect::new(bb.x0, bb.y0, bb.x1, bb.y1));
        });
        self.cached_selection_rects = rects;
    }

    fn label(&self) -> Option<&str> {
        let text = self.editor.raw_text();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    fn background_key(&self, _element: &Element) -> Option<ResourceKey> {
        Some(ThemeKeys::PANEL_BACKGROUND)
    }

    fn height_key(&self) -> Option<ResourceKey> {
        // No theme height — TextInput uses Widget::measure() for dynamic sizing.
        None
    }

    fn default_pickable(&self) -> bool {
        true
    }

    fn default_focusable(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
