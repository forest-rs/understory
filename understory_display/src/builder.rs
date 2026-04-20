// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Tiny builder helpers for `DisplayList`.

use kurbo::{Rect, RoundedRect, Shape as _, Stroke};
use peniko::Brush;

use crate::{DisplayItem, DisplayList, DisplayOp, ItemId, SemanticId};

/// Builder for one retained [`DisplayList`].
#[derive(Clone, Debug, Default)]
pub struct DisplayListBuilder {
    list: DisplayList,
    next_id: u32,
}

impl DisplayListBuilder {
    /// Creates an empty display-list builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes one fully-formed retained item.
    pub fn push(&mut self, item: DisplayItem) {
        self.next_id = self.next_id.max(item.id.index().saturating_add(1));
        self.list.push(item);
    }

    /// Appends a filled rect item.
    #[must_use]
    pub fn fill_rect(
        &mut self,
        rect: Rect,
        brush: Brush,
        z: i32,
        semantic_id: Option<SemanticId>,
    ) -> ItemId {
        self.push_with_bounds(rect, z, semantic_id, DisplayOp::FillRect { rect, brush })
    }

    /// Appends a stroked rect item.
    #[must_use]
    pub fn stroke_rect(
        &mut self,
        rect: Rect,
        stroke: Stroke,
        brush: Brush,
        z: i32,
        semantic_id: Option<SemanticId>,
    ) -> ItemId {
        let half_width = stroke.width * 0.5;
        let bounds = Rect::new(
            rect.x0 - half_width,
            rect.y0 - half_width,
            rect.x1 + half_width,
            rect.y1 + half_width,
        );
        self.push_with_bounds(
            bounds,
            z,
            semantic_id,
            DisplayOp::StrokeRect {
                rect,
                stroke,
                brush,
            },
        )
    }

    /// Appends a filled rounded-rect item.
    #[must_use]
    pub fn fill_rounded_rect(
        &mut self,
        rect: RoundedRect,
        brush: Brush,
        z: i32,
        semantic_id: Option<SemanticId>,
    ) -> ItemId {
        self.push_with_bounds(
            rect.bounding_box(),
            z,
            semantic_id,
            DisplayOp::FillRoundedRect { rect, brush },
        )
    }

    /// Appends a stroked rounded-rect item.
    #[must_use]
    pub fn stroke_rounded_rect(
        &mut self,
        rect: RoundedRect,
        stroke: Stroke,
        brush: Brush,
        z: i32,
        semantic_id: Option<SemanticId>,
    ) -> ItemId {
        let base = rect.bounding_box();
        let half_width = stroke.width * 0.5;
        let bounds = Rect::new(
            base.x0 - half_width,
            base.y0 - half_width,
            base.x1 + half_width,
            base.y1 + half_width,
        );
        self.push_with_bounds(
            bounds,
            z,
            semantic_id,
            DisplayOp::StrokeRoundedRect {
                rect,
                stroke,
                brush,
            },
        )
    }

    /// Finishes the builder and returns the retained display list.
    #[must_use]
    pub fn build(self) -> DisplayList {
        self.list
    }

    fn push_with_bounds(
        &mut self,
        bounds: Rect,
        z: i32,
        semantic_id: Option<SemanticId>,
        op: DisplayOp,
    ) -> ItemId {
        let id = ItemId::new(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.list.push(DisplayItem {
            id,
            bounds,
            z,
            semantic_id,
            op,
        });
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peniko::Color;

    #[test]
    fn builder_assigns_monotonic_ids() {
        let mut builder = DisplayListBuilder::new();
        let a = builder.fill_rect(
            Rect::new(0.0, 0.0, 10.0, 10.0),
            Brush::Solid(Color::from_rgb8(0, 0, 0)),
            0,
            None,
        );
        let b = builder.fill_rect(
            Rect::new(10.0, 0.0, 20.0, 10.0),
            Brush::Solid(Color::from_rgb8(255, 255, 255)),
            1,
            Some(SemanticId::new(7)),
        );

        assert_eq!(a.index(), 0);
        assert_eq!(b.index(), 1);
        assert_eq!(builder.build().len(), 2);
    }

    #[test]
    fn stroke_bounds_include_half_width_outset() {
        let mut builder = DisplayListBuilder::new();
        let _ = builder.stroke_rect(
            Rect::new(10.0, 20.0, 30.0, 40.0),
            Stroke::new(4.0),
            Brush::Solid(Color::from_rgb8(0, 0, 0)),
            0,
            None,
        );
        let list = builder.build();
        assert_eq!(list.items()[0].bounds, Rect::new(8.0, 18.0, 32.0, 42.0));
    }
}
