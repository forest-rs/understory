// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::{String, ToString};

use parlance::{
    BaseDirection, FontFamily, FontStyle, FontWeight, FontWidth, GenericFamily, Language,
    OverflowWrap, TextWrapMode, WordBreak,
};

use crate::Brush;

/// Resolved text drawing intent.
///
/// This is an umbrella over concrete text payloads. Today the presentation
/// crate only provides [`PlainTextPrimitive`]; styled text can be added as a
/// sibling variant without changing the outer [`crate::Primitive::Text`] role.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum TextPrimitive {
    /// Plain single-run text.
    Plain(PlainTextPrimitive),
}

impl TextPrimitive {
    /// Creates a plain text primitive.
    #[must_use]
    pub const fn plain(text: PlainTextPrimitive) -> Self {
        Self::Plain(text)
    }

    /// Returns this text primitive as plain text, if it is plain text.
    #[must_use]
    pub const fn as_plain(&self) -> Option<&PlainTextPrimitive> {
        match self {
            Self::Plain(text) => Some(text),
        }
    }

    /// Returns this text primitive as mutable plain text, if it is plain text.
    #[must_use]
    pub const fn as_plain_mut(&mut self) -> Option<&mut PlainTextPrimitive> {
        match self {
            Self::Plain(text) => Some(text),
        }
    }
}

impl Default for TextPrimitive {
    fn default() -> Self {
        Self::Plain(PlainTextPrimitive::default())
    }
}

/// Resolved plain text drawing intent.
///
/// Text shaping is intentionally not stored here. Shaping usually depends on
/// geometry, so a toolkit lowerer should shape text while lowering this
/// primitive using bounds from its geometry tree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlainTextPrimitive {
    /// Text content to draw.
    pub content: TextContent,

    /// Optional foreground brush.
    pub foreground: Option<Brush>,

    /// Resolved single-run font and typographic inputs.
    pub style: TextStyle,

    /// Resolved layout hints for shaping and painting within the node bounds.
    pub layout: TextLayout,
}

/// Owned text content stored by a text primitive.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum TextContent {
    /// No text content.
    #[default]
    Empty,

    /// Plain UTF-8 text.
    Plain(String),
}

impl TextContent {
    /// Creates plain text content.
    #[must_use]
    pub fn plain(text: impl AsRef<str>) -> Self {
        Self::Plain(text.as_ref().to_string())
    }

    /// Returns true when the content is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Empty => true,
            Self::Plain(text) => text.is_empty(),
        }
    }

    /// Returns the content as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Empty => "",
            Self::Plain(text) => text.as_str(),
        }
    }
}

impl From<&str> for TextContent {
    fn from(value: &str) -> Self {
        Self::plain(value)
    }
}

impl From<String> for TextContent {
    fn from(value: String) -> Self {
        Self::Plain(value)
    }
}

/// Resolved single-run font and typographic inputs.
///
/// This uses `parlance` for font and CSS text leaf types so presentation does
/// not define a parallel text vocabulary.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// Preferred font family list.
    pub font_family: FontFamily<'static>,

    /// Font size in logical pixels.
    pub font_size: f32,

    /// Font width.
    pub font_width: FontWidth,

    /// Font weight.
    pub font_weight: FontWeight,

    /// Font style.
    pub font_style: FontStyle,

    /// Optional content language.
    pub language: Option<Language>,

    /// Line height policy.
    pub line_height: TextLineHeight,

    /// Extra letter spacing in logical pixels.
    pub letter_spacing: f32,

    /// Extra word spacing in logical pixels.
    pub word_spacing: f32,

    /// Control over where words can break.
    pub word_break: WordBreak,

    /// Control over emergency line breaking.
    pub overflow_wrap: OverflowWrap,

    /// Control over non-emergency line breaking.
    pub text_wrap_mode: TextWrapMode,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: FontFamily::from(GenericFamily::SystemUi),
            font_size: 14.0,
            font_width: FontWidth::NORMAL,
            font_weight: FontWeight::NORMAL,
            font_style: FontStyle::Normal,
            language: None,
            line_height: TextLineHeight::default(),
            letter_spacing: 0.0,
            word_spacing: 0.0,
            word_break: WordBreak::Normal,
            overflow_wrap: OverflowWrap::Normal,
            text_wrap_mode: TextWrapMode::NoWrap,
        }
    }
}

/// Line height policy for text layout.
///
/// `parlance` does not currently define a line-height type, so presentation
/// keeps this local and intentionally mirrors the variants a Parley lowerer can
/// map directly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextLineHeight {
    /// Scale the font's preferred metrics line height.
    MetricsRelative(f32),

    /// Scale the font size.
    FontSizeRelative(f32),

    /// Absolute line height in logical pixels.
    Absolute(f32),
}

impl Default for TextLineHeight {
    fn default() -> Self {
        Self::MetricsRelative(1.0)
    }
}

/// Resolved text layout hints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextLayout {
    /// Horizontal alignment within the node bounds.
    pub align: TextAlign,

    /// Overflow behavior.
    pub overflow: TextOverflow,

    /// Optional maximum number of lines.
    pub max_lines: Option<u16>,

    /// Paragraph base direction.
    pub base_direction: BaseDirection,
}

impl Default for TextLayout {
    fn default() -> Self {
        Self {
            align: TextAlign::Start,
            overflow: TextOverflow::Clip,
            max_lines: None,
            base_direction: BaseDirection::Auto,
        }
    }
}

/// Horizontal text alignment.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextAlign {
    /// Align to the start edge of the inline direction.
    #[default]
    Start,

    /// Center within the line box.
    Center,

    /// Align to the end edge of the inline direction.
    End,
}

/// Text overflow behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextOverflow {
    /// Clip overflow.
    #[default]
    Clip,

    /// Request ellipsis when the lowerer supports it.
    Ellipsis,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_content_exposes_str() {
        let content = TextContent::plain("Apply");

        assert!(!content.is_empty());
        assert_eq!(content.as_str(), "Apply");
    }

    #[test]
    fn text_defaults_are_calm() {
        let text = PlainTextPrimitive::default();

        assert!(text.content.is_empty());
        assert_eq!(text.style.font_size, 14.0);
        assert_eq!(
            text.style.font_family,
            FontFamily::from(GenericFamily::SystemUi)
        );
        assert_eq!(text.style.font_width, FontWidth::NORMAL);
        assert_eq!(text.style.font_weight, FontWeight::NORMAL);
        assert_eq!(text.style.text_wrap_mode, TextWrapMode::NoWrap);
        assert_eq!(text.layout.align, TextAlign::Start);
        assert_eq!(text.layout.base_direction, BaseDirection::Auto);
    }
}
