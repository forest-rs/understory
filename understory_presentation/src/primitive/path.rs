// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use peniko::kurbo::{Affine, BezPath, Stroke};
use peniko::{Brush, Fill};

/// Resolved path drawing intent.
///
/// Geometry lives on the primitive because the path outline is the visual
/// content. Node layout bounds and scene traversal still belong to the caller's
/// geometry tree.
#[derive(Clone, Debug, PartialEq)]
pub struct PathPrimitive {
    /// Path geometry in the node's local coordinate space.
    pub path: BezPath,

    /// Transform applied to the path before drawing.
    pub transform: Affine,

    /// Optional fill pass.
    pub fill: Option<PathFill>,

    /// Optional stroke pass.
    pub stroke: Option<PathStroke>,

    /// Relative ordering of the fill and stroke passes.
    pub paint_order: PathPaintOrder,
}

impl PathPrimitive {
    /// Creates a path primitive with no fill or stroke.
    #[must_use]
    pub fn new(path: BezPath) -> Self {
        Self {
            path,
            ..Self::default()
        }
    }

    /// Sets the fill pass.
    #[must_use]
    pub fn fill(mut self, fill: PathFill) -> Self {
        self.fill = Some(fill);
        self
    }

    /// Sets the stroke pass.
    #[must_use]
    pub fn stroke(mut self, stroke: PathStroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    /// Sets the path transform.
    #[must_use]
    pub fn transform(mut self, transform: Affine) -> Self {
        self.transform = transform;
        self
    }

    /// Sets the fill/stroke paint order.
    #[must_use]
    pub fn paint_order(mut self, paint_order: PathPaintOrder) -> Self {
        self.paint_order = paint_order;
        self
    }

    /// Returns true when this path has no visible drawing pass.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fill.is_none() && self.stroke.is_none()
    }
}

impl Default for PathPrimitive {
    fn default() -> Self {
        Self {
            path: BezPath::new(),
            transform: Affine::IDENTITY,
            fill: None,
            stroke: None,
            paint_order: PathPaintOrder::default(),
        }
    }
}

/// Fill pass for a [`PathPrimitive`].
#[derive(Clone, Debug, PartialEq)]
pub struct PathFill {
    /// Brush used to fill the path interior.
    pub brush: Brush,

    /// Optional brush-local to path-local transform.
    ///
    /// `None` means identity: the renderer samples the brush in path-local
    /// coordinates without an additional brush transform.
    pub brush_transform: Option<Affine>,

    /// Fill rule used to determine the path interior.
    pub rule: Fill,
}

impl PathFill {
    /// Creates a non-zero fill with `brush`.
    #[must_use]
    pub fn new(brush: impl Into<Brush>) -> Self {
        Self {
            brush: brush.into(),
            brush_transform: None,
            rule: Fill::NonZero,
        }
    }

    /// Sets the brush-local to path-local transform.
    #[must_use]
    pub fn with_brush_transform(mut self, transform: Affine) -> Self {
        self.brush_transform = Some(transform);
        self
    }

    /// Sets the fill rule.
    #[must_use]
    pub fn rule(mut self, rule: Fill) -> Self {
        self.rule = rule;
        self
    }
}

/// Stroke pass for a [`PathPrimitive`].
#[derive(Clone, Debug, PartialEq)]
pub struct PathStroke {
    /// Brush used to stroke the path outline.
    pub brush: Brush,

    /// Optional brush-local to path-local transform.
    ///
    /// `None` means identity: the renderer samples the brush in path-local
    /// coordinates without an additional brush transform.
    pub brush_transform: Option<Affine>,

    /// Stroke style.
    pub stroke: Stroke,
}

impl PathStroke {
    /// Creates a stroke with `brush` and width in logical pixels.
    #[must_use]
    pub fn new(brush: impl Into<Brush>, width: f64) -> Self {
        Self {
            brush: brush.into(),
            brush_transform: None,
            stroke: Stroke::new(width),
        }
    }

    /// Sets the brush-local to path-local transform.
    #[must_use]
    pub fn with_brush_transform(mut self, transform: Affine) -> Self {
        self.brush_transform = Some(transform);
        self
    }

    /// Replaces the stroke style.
    #[must_use]
    pub fn style(mut self, stroke: Stroke) -> Self {
        self.stroke = stroke;
        self
    }
}

/// Relative ordering of path fill and stroke passes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PathPaintOrder {
    /// Paint fill first, then stroke.
    #[default]
    FillThenStroke,

    /// Paint stroke first, then fill.
    StrokeThenFill,
}

#[cfg(test)]
mod tests {
    use super::*;
    use peniko::Color;

    #[test]
    fn empty_path_has_no_visible_passes() {
        let path = PathPrimitive::new(BezPath::new());

        assert!(path.is_empty());
    }

    #[test]
    fn fill_and_stroke_make_path_visible() {
        let path = PathPrimitive::new(BezPath::new())
            .fill(PathFill::new(Color::WHITE))
            .stroke(PathStroke::new(Color::BLACK, 2.0));

        assert!(!path.is_empty());
        assert_eq!(path.fill.as_ref().unwrap().brush_transform, None);
        assert_eq!(path.stroke.unwrap().stroke.width, 2.0);
    }

    #[test]
    fn fill_and_stroke_can_carry_brush_transforms() {
        let fill_transform = Affine::scale_non_uniform(8.0, 12.0);
        let stroke_transform = Affine::translate((3.0, 5.0));

        let fill = PathFill::new(Color::WHITE).with_brush_transform(fill_transform);
        let stroke = PathStroke::new(Color::BLACK, 2.0).with_brush_transform(stroke_transform);

        assert_eq!(fill.brush_transform, Some(fill_transform));
        assert_eq!(stroke.brush_transform, Some(stroke_transform));
    }
}
