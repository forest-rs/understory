// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{Point, Rect};

/// An anchor supplied by a caller in [`AnchorInput`](crate::AnchorInput).
///
/// Use `Point` for pointer/context-menu anchors, `Rect` for a single widget or
/// caret box, and `Rects` for wrapped selections or multi-rect caret geometry.
#[derive(Clone, Copy, Debug)]
pub enum Anchor<'a> {
    /// A point anchor, represented internally as a zero-size reference rect.
    Point(Point),
    /// A single rectangle anchor.
    Rect(Rect),
    /// A multi-rectangle anchor, such as a wrapped text selection.
    Rects {
        /// The rectangle collection and optional role indexes.
        rects: AnchorRects<'a>,
        /// The reference policy used to derive one placement rectangle.
        reference: RectReference,
    },
}

/// Borrowed rectangles and role indexes for a multi-rectangle anchor.
///
/// Callers construct this for [`Anchor::Rects`] when text layout, selection,
/// or editor geometry exposes several candidate rectangles. `primary` and
/// `focus` indexes are optional because some callers only have ordered rects.
#[derive(Clone, Copy, Debug)]
pub struct AnchorRects<'a> {
    /// The candidate rectangles in caller-defined order.
    pub rects: &'a [Rect],
    /// Optional primary rectangle index.
    pub primary: Option<usize>,
    /// Optional focus rectangle index.
    pub focus: Option<usize>,
}

/// How a multi-rectangle anchor chooses its reference rectangle.
///
/// Callers use this with [`Anchor::Rects`] to decide which geometric fact
/// drives placement. The resolver stores the selected rectangle in
/// [`AnchorFrame::reference_rect`](crate::AnchorFrame::reference_rect).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RectReference {
    /// Use the union of all rectangles.
    BoundingBox,
    /// Use the first rectangle.
    First,
    /// Use the last rectangle.
    Last,
    /// Use the primary rectangle index.
    Primary,
    /// Use the focus rectangle index.
    Focus,
    /// Use the rectangle with the largest area.
    Largest,
}

/// Derive the reference rectangle for an anchor.
///
/// Call this when a higher layer wants to preview or debug the exact rectangle
/// that [`resolve_anchor`](crate::resolve_anchor) will use before constraints
/// are applied. Anchor coordinates must be finite; debug builds assert this
/// contract. Empty multi-rect anchors and missing role indexes return `None`.
#[must_use]
pub fn reference_rect(anchor: Anchor<'_>) -> Option<Rect> {
    match anchor {
        Anchor::Point(point) => {
            debug_assert!(point.is_finite(), "anchor point must be finite");
            Some(Rect::new(point.x, point.y, point.x, point.y))
        }
        Anchor::Rect(rect) => {
            debug_assert!(rect.is_finite(), "anchor rect must be finite");
            Some(rect.abs())
        }
        Anchor::Rects { rects, reference } => {
            debug_assert!(
                rects.rects.iter().all(Rect::is_finite),
                "anchor rect collection must contain only finite rectangles",
            );
            reference_rects(rects, reference)
        }
    }
}

fn reference_rects(rects: AnchorRects<'_>, reference: RectReference) -> Option<Rect> {
    match reference {
        RectReference::BoundingBox => {
            let mut iter = rects.rects.iter().copied().map(|rect| rect.abs());
            let first = iter.next()?;
            Some(iter.fold(first, |acc, rect| acc.union(rect)))
        }
        RectReference::First => rects.rects.first().copied().map(|rect| rect.abs()),
        RectReference::Last => rects.rects.last().copied().map(|rect| rect.abs()),
        RectReference::Primary => rects
            .primary
            .and_then(|index| rects.rects.get(index).copied().map(|rect| rect.abs())),
        RectReference::Focus => rects
            .focus
            .and_then(|index| rects.rects.get(index).copied().map(|rect| rect.abs())),
        RectReference::Largest => rects
            .rects
            .iter()
            .copied()
            .map(|rect| rect.abs())
            .max_by(|a, b| a.area().total_cmp(&b.area())),
    }
}
