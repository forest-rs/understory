// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained display items and draw operations.

use kurbo::{Rect, RoundedRect, Stroke};
use peniko::Brush;

use crate::{ItemId, SemanticId};

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
}
