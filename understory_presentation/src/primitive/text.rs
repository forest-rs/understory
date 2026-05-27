// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::{String, ToString};
use core::hash::{Hash, Hasher};

use parlance::{
    BaseDirection, FontFamily, FontFamilyName, FontStyle, FontWeight, FontWidth, GenericFamily,
    Language, OverflowWrap, TextWrapMode, WordBreak,
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
///
/// Equality and hashing use the same float key policy as the presentation
/// cache: `-0.0` and `0.0` compare equal, while other `f32` values compare by
/// their bit pattern. NaN payloads are therefore stable and self-equal only
/// when their bit pattern matches.
#[derive(Clone, Debug)]
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

impl PartialEq for TextStyle {
    fn eq(&self, other: &Self) -> bool {
        self.font_family == other.font_family
            && scalar_eq(self.font_size, other.font_size)
            && scalar_eq(self.font_width.ratio(), other.font_width.ratio())
            && scalar_eq(self.font_weight.value(), other.font_weight.value())
            && font_style_eq(self.font_style, other.font_style)
            && self.language == other.language
            && self.line_height == other.line_height
            && scalar_eq(self.letter_spacing, other.letter_spacing)
            && scalar_eq(self.word_spacing, other.word_spacing)
            && self.word_break == other.word_break
            && self.overflow_wrap == other.overflow_wrap
            && self.text_wrap_mode == other.text_wrap_mode
    }
}

impl Eq for TextStyle {}

impl Hash for TextStyle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_font_family(&self.font_family, state);
        ScalarKey::new(self.font_size).hash(state);
        ScalarKey::new(self.font_width.ratio()).hash(state);
        ScalarKey::new(self.font_weight.value()).hash(state);
        hash_font_style(self.font_style, state);
        self.language.hash(state);
        self.line_height.hash(state);
        ScalarKey::new(self.letter_spacing).hash(state);
        ScalarKey::new(self.word_spacing).hash(state);
        hash_word_break(self.word_break, state);
        hash_overflow_wrap(self.overflow_wrap, state);
        hash_text_wrap_mode(self.text_wrap_mode, state);
    }
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
///
/// Equality and hashing normalize `-0.0` to `0.0`; other `f32` values compare
/// and hash by bit pattern.
#[derive(Clone, Copy, Debug)]
pub enum TextLineHeight {
    /// Scale the font's preferred metrics line height.
    MetricsRelative(f32),

    /// Scale the font size.
    FontSizeRelative(f32),

    /// Absolute line height in logical pixels.
    Absolute(f32),
}

impl PartialEq for TextLineHeight {
    fn eq(&self, other: &Self) -> bool {
        match (*self, *other) {
            (Self::MetricsRelative(a), Self::MetricsRelative(b))
            | (Self::FontSizeRelative(a), Self::FontSizeRelative(b))
            | (Self::Absolute(a), Self::Absolute(b)) => scalar_eq(a, b),
            _ => false,
        }
    }
}

impl Eq for TextLineHeight {}

impl Hash for TextLineHeight {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match *self {
            Self::MetricsRelative(value) => {
                0_u8.hash(state);
                ScalarKey::new(value).hash(state);
            }
            Self::FontSizeRelative(value) => {
                1_u8.hash(state);
                ScalarKey::new(value).hash(state);
            }
            Self::Absolute(value) => {
                2_u8.hash(state);
                ScalarKey::new(value).hash(state);
            }
        }
    }
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ScalarKey(u32);

impl ScalarKey {
    fn new(value: f32) -> Self {
        if value == 0.0 {
            Self(0)
        } else {
            Self(value.to_bits())
        }
    }
}

fn scalar_eq(a: f32, b: f32) -> bool {
    ScalarKey::new(a) == ScalarKey::new(b)
}

fn font_style_eq(a: FontStyle, b: FontStyle) -> bool {
    match (a, b) {
        (FontStyle::Normal, FontStyle::Normal) | (FontStyle::Italic, FontStyle::Italic) => true,
        (FontStyle::Oblique(a), FontStyle::Oblique(b)) => match (a, b) {
            (Some(a), Some(b)) => scalar_eq(a, b),
            (None, None) => true,
            _ => false,
        },
        _ => false,
    }
}

fn hash_font_family<H: Hasher>(font_family: &FontFamily<'static>, state: &mut H) {
    match font_family {
        FontFamily::Source(source) => {
            0_u8.hash(state);
            source.as_ref().hash(state);
        }
        FontFamily::Single(name) => {
            1_u8.hash(state);
            hash_font_family_name(name, state);
        }
        FontFamily::List(list) => {
            2_u8.hash(state);
            list.len().hash(state);
            for name in list.iter() {
                hash_font_family_name(name, state);
            }
        }
    }
}

fn hash_font_family_name<H: Hasher>(name: &FontFamilyName<'static>, state: &mut H) {
    match name {
        FontFamilyName::Named(name) => {
            0_u8.hash(state);
            name.as_ref().hash(state);
        }
        FontFamilyName::Generic(generic) => {
            1_u8.hash(state);
            generic.hash(state);
        }
    }
}

fn hash_font_style<H: Hasher>(style: FontStyle, state: &mut H) {
    match style {
        FontStyle::Normal => 0_u8.hash(state),
        FontStyle::Italic => 1_u8.hash(state),
        FontStyle::Oblique(angle) => {
            2_u8.hash(state);
            angle.map(ScalarKey::new).hash(state);
        }
    }
}

fn hash_word_break<H: Hasher>(word_break: WordBreak, state: &mut H) {
    match word_break {
        WordBreak::Normal => 0_u8.hash(state),
        WordBreak::BreakAll => 1_u8.hash(state),
        WordBreak::KeepAll => 2_u8.hash(state),
    }
}

fn hash_overflow_wrap<H: Hasher>(overflow_wrap: OverflowWrap, state: &mut H) {
    match overflow_wrap {
        OverflowWrap::Normal => 0_u8.hash(state),
        OverflowWrap::Anywhere => 1_u8.hash(state),
        OverflowWrap::BreakWord => 2_u8.hash(state),
    }
}

fn hash_text_wrap_mode<H: Hasher>(text_wrap_mode: TextWrapMode, state: &mut H) {
    match text_wrap_mode {
        TextWrapMode::Wrap => 0_u8.hash(state),
        TextWrapMode::NoWrap => 1_u8.hash(state),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::hash::{Hash, Hasher};

    fn hash_value(value: impl Hash) -> u64 {
        let mut hasher = TestHasher::default();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[derive(Default)]
    struct TestHasher {
        hash: u64,
    }

    impl Hasher for TestHasher {
        fn finish(&self) -> u64 {
            self.hash
        }

        fn write(&mut self, bytes: &[u8]) {
            for byte in bytes {
                self.hash = self.hash.wrapping_mul(16_777_619) ^ u64::from(*byte);
            }
        }
    }

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

    #[test]
    fn text_line_height_hash_matches_equality() {
        let positive_zero = TextLineHeight::FontSizeRelative(0.0);
        let negative_zero = TextLineHeight::FontSizeRelative(-0.0);

        assert_eq!(positive_zero, negative_zero);
        assert_eq!(hash_value(positive_zero), hash_value(negative_zero));
    }

    #[test]
    fn text_style_hash_matches_equality() {
        let mut left = TextStyle {
            font_size: 0.0,
            font_style: FontStyle::Oblique(Some(-0.0)),
            line_height: TextLineHeight::Absolute(-0.0),
            letter_spacing: -0.0,
            word_spacing: 0.0,
            ..TextStyle::default()
        };
        let right = TextStyle {
            font_size: -0.0,
            font_style: FontStyle::Oblique(Some(0.0)),
            line_height: TextLineHeight::Absolute(0.0),
            letter_spacing: 0.0,
            word_spacing: -0.0,
            ..TextStyle::default()
        };

        assert_eq!(left, right);
        assert_eq!(hash_value(&left), hash_value(&right));

        left.font_weight = FontWeight::BOLD;
        assert_ne!(left, right);
    }
}
