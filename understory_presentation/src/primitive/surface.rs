// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use smallvec::SmallVec;

use crate::{Brush, Color};
use peniko::kurbo::RoundedRectRadii;

/// Resolved box-decoration drawing intent.
///
/// Geometry such as bounds and transforms lives outside this crate. Surface
/// backgrounds, borders, corner radii, and shadows are resolved inputs that a
/// toolkit lowerer applies to the node's final geometry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SurfacePrimitive {
    /// Background layers, painted in list order.
    pub backgrounds: SmallVec<[BackgroundLayer; 1]>,

    /// Per-side border model.
    pub border: Border,

    /// Per-corner radii in logical pixels.
    pub corner_radii: RoundedRectRadii,

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
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.brush.is_none() || self.width <= 0.0
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
    fn corner_radii_alone_do_not_make_surface_visible() {
        let surface = SurfacePrimitive {
            corner_radii: RoundedRectRadii::from_single_radius(8.0),
            ..SurfacePrimitive::default()
        };

        assert!(surface.is_empty());
    }
}
