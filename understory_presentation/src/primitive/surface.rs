// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use smallvec::SmallVec;

use crate::{BoxDecorationGeometry, Brush, Color, CornerRadii, CornerShapes, Edges};
use peniko::kurbo::Rect;

/// Resolved box-decoration drawing intent.
///
/// Geometry such as bounds and transforms lives outside this crate. Surface
/// backgrounds, borders, padding widths, corner shapes, and shadows are
/// resolved inputs that a toolkit lowerer applies to the node's final
/// geometry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfacePrimitive {
    /// Background layers, painted in list order.
    pub backgrounds: SmallVec<[BackgroundLayer; 1]>,

    /// Per-side border model.
    pub border: Border,

    /// Physical padding widths in logical pixels.
    ///
    /// These widths are the decoration padding used to derive the content
    /// contour in [`SurfacePrimitive::decoration_geometry`]. Toolkits that
    /// also use padding during layout should resolve padding once and copy the
    /// same values here; independently-resolved layout and decoration padding
    /// can make content insets drift from painted contours.
    pub padding_widths: Edges<f64>,

    /// Per-corner radii in logical pixels.
    pub corner_radii: CornerRadii,

    /// Per-corner contour shapes.
    pub corner_shapes: CornerShapes,

    /// Outer shadows, painted in list order.
    pub shadows: SmallVec<[Shadow; 1]>,
}

impl SurfacePrimitive {
    /// Replaces the surface with a single background brush.
    pub fn set_background(&mut self, brush: impl Into<Brush>) {
        self.backgrounds.clear();
        self.backgrounds.push(BackgroundLayer::new(brush));
    }

    /// Returns true when the surface has no visible decoration.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.backgrounds.is_empty() && self.border.is_empty() && self.shadows.is_empty()
    }

    /// Resolve geometry for this surface's border box.
    ///
    /// Presentation stores decoration intent without owning layout bounds.
    /// Call this from a lowerer once the caller's geometry tree has supplied
    /// the surface's final border box. The returned geometry can be used to
    /// fill or clip named box areas, ask for contour paths, or construct
    /// renderer-specific border paths without storing materialized paths in
    /// the presentation primitive. The content contour is derived from
    /// [`SurfacePrimitive::padding_widths`], so callers should keep those
    /// values in sync with any layout padding used for the same box.
    #[must_use]
    pub fn decoration_geometry(&self, border_box: Rect) -> BoxDecorationGeometry {
        BoxDecorationGeometry::from_border_box(
            border_box,
            self.border.visible_widths(),
            self.padding_widths,
            self.corner_radii,
            self.corner_shapes,
        )
    }
}

/// A single resolved background layer.
#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundLayer {
    /// Brush used to fill the node's local bounds.
    pub brush: Brush,
}

impl BackgroundLayer {
    /// Creates a background layer from a brush.
    #[must_use]
    pub fn new(brush: impl Into<Brush>) -> Self {
        Self {
            brush: brush.into(),
        }
    }
}

/// Per-side border model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Border {
    /// Top border side.
    pub top: BorderSide,

    /// Right border side.
    pub right: BorderSide,

    /// Bottom border side.
    pub bottom: BorderSide,

    /// Left border side.
    pub left: BorderSide,
}

impl Border {
    /// Creates the same border side on all four edges.
    #[must_use]
    pub fn uniform(brush: impl Into<Brush>, width: f64) -> Self {
        let side = BorderSide::new(brush, width);
        Self {
            top: side.clone(),
            right: side.clone(),
            bottom: side.clone(),
            left: side,
        }
    }

    /// Returns true when every side is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.top.is_empty()
            && self.right.is_empty()
            && self.bottom.is_empty()
            && self.left.is_empty()
    }

    /// Return the widths of visible border sides.
    ///
    /// A side with no brush is treated as width zero, matching
    /// [`BorderSide::is_empty`]. The result is suitable for
    /// [`BoxDecorationGeometry::from_border_box`].
    #[must_use]
    pub fn visible_widths(&self) -> Edges<f64> {
        Edges::new(
            self.top.visible_width(),
            self.right.visible_width(),
            self.bottom.visible_width(),
            self.left.visible_width(),
        )
    }

    /// Return the shared visible side when all four sides are equal.
    ///
    /// This is useful for lowerers that only have a uniform-border fast path.
    /// Empty borders return `None`, and non-uniform side widths or brushes
    /// also return `None`.
    #[must_use]
    pub fn uniform_visible_side(&self) -> Option<&BorderSide> {
        if self.top.is_empty() {
            return None;
        }
        if self.top == self.right && self.top == self.bottom && self.top == self.left {
            Some(&self.top)
        } else {
            None
        }
    }
}

/// One side of a border.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BorderSide {
    /// Border brush.
    pub brush: Option<Brush>,

    /// Border width in logical pixels.
    pub width: f64,
}

impl BorderSide {
    /// Creates a border side.
    #[must_use]
    pub fn new(brush: impl Into<Brush>, width: f64) -> Self {
        Self {
            brush: Some(brush.into()),
            width,
        }
    }

    /// Returns true when the side should not draw.
    ///
    /// A side is visible only when it has a brush and a positive finite width.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.brush.is_none() || !self.width.is_finite() || self.width <= 0.0
    }

    /// Return the side width when the side has a brush and positive finite
    /// width.
    #[must_use]
    pub fn visible_width(&self) -> f64 {
        if self.is_empty() { 0.0 } else { self.width }
    }
}

/// Resolved outer shadow intent for a surface.
///
/// The lowerer applies the shadow to the node's final geometry. This keeps
/// shadow spread and blur bounds-dependent without making presentation own
/// geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct Shadow {
    /// Shadow color.
    pub color: Color,

    /// Horizontal offset in logical pixels.
    pub offset_x: f64,

    /// Vertical offset in logical pixels.
    pub offset_y: f64,

    /// Blur radius in logical pixels.
    pub blur_radius: f64,

    /// Spread distance in logical pixels.
    pub spread: f64,
}

impl Shadow {
    /// Creates an outer shadow.
    #[must_use]
    pub fn new(color: Color, offset_x: f64, offset_y: f64, blur_radius: f64) -> Self {
        Self {
            color,
            offset_x,
            offset_y,
            blur_radius,
            spread: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_background_replaces_existing_layers() {
        let mut surface = SurfacePrimitive::default();
        surface.set_background(Color::from_rgb8(10, 20, 30));
        surface.set_background(Color::from_rgb8(40, 50, 60));

        assert_eq!(surface.backgrounds.len(), 1);
        assert_eq!(
            surface.backgrounds[0].brush,
            Brush::from(Color::from_rgb8(40, 50, 60))
        );
    }

    #[test]
    fn uniform_border_sets_each_side() {
        let border = Border::uniform(Color::from_rgb8(10, 20, 30), 2.0);
        let expected = BorderSide::new(Color::from_rgb8(10, 20, 30), 2.0);

        assert_eq!(border.top, expected);
        assert_eq!(border.right, expected);
        assert_eq!(border.bottom, expected);
        assert_eq!(border.left, expected);
        assert!(!border.is_empty());
    }

    #[test]
    fn non_finite_border_widths_are_empty() {
        for width in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let side = BorderSide::new(Color::BLACK, width);
            assert!(side.is_empty());
            assert_eq!(side.visible_width(), 0.0);
        }

        let border = Border::uniform(Color::BLACK, f64::NAN);
        assert!(border.is_empty());
        assert_eq!(border.visible_widths(), Edges::ZERO);
        assert_eq!(border.uniform_visible_side(), None);
    }

    #[test]
    fn corner_radii_alone_do_not_make_surface_visible() {
        let surface = SurfacePrimitive {
            corner_radii: CornerRadii::uniform(8.0),
            corner_shapes: CornerShapes::ROUND,
            ..SurfacePrimitive::default()
        };

        assert!(surface.is_empty());
    }

    #[test]
    fn decoration_geometry_uses_visible_widths_and_corner_radii() {
        let surface = SurfacePrimitive {
            border: Border {
                top: BorderSide::new(Color::BLACK, 2.0),
                right: BorderSide {
                    brush: None,
                    width: 6.0,
                },
                bottom: BorderSide::new(Color::BLACK, -4.0),
                left: BorderSide::new(Color::BLACK, 4.0),
            },
            padding_widths: Edges::new(1.0, 2.0, 3.0, 4.0),
            corner_radii: CornerRadii::circular(10.0, 12.0, 14.0, 16.0),
            corner_shapes: CornerShapes::all(crate::CornerShape::squircle()),
            ..SurfacePrimitive::default()
        };

        let geometry = surface.decoration_geometry(Rect::new(0.0, 0.0, 100.0, 50.0));

        assert_eq!(geometry.border_widths, Edges::new(2.0, 0.0, 0.0, 4.0));
        assert_eq!(geometry.padding_widths, Edges::new(1.0, 2.0, 3.0, 4.0));
        assert_eq!(geometry.padding_box, Rect::new(4.0, 2.0, 100.0, 50.0));
        assert_eq!(geometry.content_box, Rect::new(8.0, 3.0, 98.0, 47.0));
        assert_eq!(
            geometry.border_edge.radii(),
            CornerRadii::circular(10.0, 12.0, 14.0, 16.0)
        );
        assert_eq!(
            geometry.border_edge.corners.top_left.shape,
            crate::CornerShape::squircle()
        );
    }

    #[test]
    fn uniform_visible_side_requires_all_sides_to_match() {
        let uniform = Border::uniform(Color::BLACK, 2.0);
        assert_eq!(uniform.uniform_visible_side(), Some(&uniform.top));

        let mixed = Border {
            right: BorderSide::new(Color::WHITE, 2.0),
            ..Border::uniform(Color::BLACK, 2.0)
        };
        assert_eq!(mixed.uniform_visible_side(), None);

        assert_eq!(Border::default().uniform_visible_side(), None);
    }
}
