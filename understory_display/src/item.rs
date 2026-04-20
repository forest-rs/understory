// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained display entries, paint items, and draw operations.

use alloc::{boxed::Box, vec::Vec};

use kurbo::{Affine, Point, Rect, RoundedRect, Stroke, Vec2};
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

/// One retained display entry in a flattened command stream.
#[derive(Clone, Debug, PartialEq)]
pub enum DisplayEntry {
    /// One paint item.
    Item(Box<DisplayItem>),
    /// Begin a rectangular clip scope.
    PushClipRect(DisplayClipRect),
    /// End the most recently pushed clip scope.
    PopClip,
    /// Begin an opacity/isolated-group scope.
    PushOpacity(DisplayOpacity),
    /// End the most recently pushed opacity scope.
    PopOpacity,
    /// Begin a transform scope.
    PushTransform(DisplayTransform),
    /// End the most recently pushed transform scope.
    PopTransform,
}

/// Retained rectangular clip scope.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayClipRect {
    /// Rectangle used as the clip region.
    pub rect: Rect,
    /// Optional semantic or provenance id supplied by the host.
    pub semantic_id: Option<SemanticId>,
}

/// Retained opacity scope.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayOpacity {
    /// Alpha multiplier in `0..=1`.
    pub opacity: f32,
    /// Conservative bounds for the grouped content.
    pub bounds: Rect,
    /// Optional semantic or provenance id supplied by the host.
    pub semantic_id: Option<SemanticId>,
}

impl DisplayOpacity {
    /// Creates one retained opacity scope with clamped alpha.
    #[must_use]
    pub fn new(opacity: f32, bounds: Rect, semantic_id: Option<SemanticId>) -> Self {
        Self {
            opacity: opacity.clamp(0.0, 1.0),
            bounds,
            semantic_id,
        }
    }
}

/// Retained transform scope.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayTransform {
    /// Affine transform applied to descendant entries.
    pub transform: Affine,
    /// Optional semantic or provenance id supplied by the host.
    pub semantic_id: Option<SemanticId>,
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
