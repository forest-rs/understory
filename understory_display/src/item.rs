// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained display items and draw operations.

use alloc::vec::Vec;

use kurbo::{Point, Rect, RoundedRect, Stroke, Vec2};
#[cfg(feature = "std")]
use parley::FontData;
use peniko::Brush;

use crate::{ItemId, SemanticId};

/// One positioned glyph within a retained glyph run.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayGlyph {
    /// Glyph identifier within the selected font.
    pub id: u32,
    /// Glyph draw origin in display/user space.
    pub origin: Point,
}

/// One retained glyph run with font data and positioned glyphs.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayGlyphRun {
    /// Font resource referenced by this run.
    #[cfg(feature = "std")]
    pub font: FontData,
    /// Font size used during shaping.
    pub font_size: f32,
    /// Normalized variation coordinates used during shaping.
    pub normalized_coords: Vec<i16>,
    /// Brush used to paint the glyphs in this run.
    pub brush: Brush,
    /// Glyphs positioned in display/user space.
    pub glyphs: Vec<DisplayGlyph>,
    /// Conservative logical bounds for the run.
    pub bounds: Rect,
}

impl DisplayGlyphRun {
    /// Returns a translated copy of the run.
    #[must_use]
    pub fn translated(&self, delta: Vec2) -> Self {
        let mut translated = self.clone();
        translated.translate(delta);
        translated
    }

    /// Translates the run in place.
    pub fn translate(&mut self, delta: Vec2) {
        self.bounds = self.bounds + delta;
        for glyph in &mut self.glyphs {
            glyph.origin += delta;
        }
    }
}

/// One retained display item.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayItem {
    /// Stable item id.
    pub id: ItemId,
    /// Bounds in display/user space.
    pub bounds: Rect,
    /// Paint order within the current display list.
    pub z: i32,
    /// Optional semantic or provenance id supplied by the host.
    pub semantic_id: Option<SemanticId>,
    /// Drawing operation for this item.
    pub op: DisplayOp,
}

/// Retained display operations for the initial display-list slice.
#[derive(Clone, Debug, PartialEq)]
pub enum DisplayOp {
    /// Fill an axis-aligned rectangle.
    FillRect {
        /// Rectangle to fill.
        rect: Rect,
        /// Brush used by the fill.
        brush: Brush,
    },
    /// Stroke an axis-aligned rectangle.
    StrokeRect {
        /// Rectangle to stroke.
        rect: Rect,
        /// Stroke style.
        stroke: Stroke,
        /// Brush used by the stroke.
        brush: Brush,
    },
    /// Fill an axis-aligned rounded rectangle.
    FillRoundedRect {
        /// Rounded rectangle to fill.
        rect: RoundedRect,
        /// Brush used by the fill.
        brush: Brush,
    },
    /// Stroke an axis-aligned rounded rectangle.
    StrokeRoundedRect {
        /// Rounded rectangle to stroke.
        rect: RoundedRect,
        /// Stroke style.
        stroke: Stroke,
        /// Brush used by the stroke.
        brush: Brush,
    },
    /// Paint one retained glyph run.
    GlyphRun {
        /// Shaped glyph run to paint.
        run: DisplayGlyphRun,
    },
}
