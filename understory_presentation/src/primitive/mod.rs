// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Presentation primitives.

use alloc::boxed::Box;
use peniko::kurbo::{Affine, Rect, Vec2};

mod image;
mod path;
mod surface;
mod text;

pub use image::{ImageFit, ImagePrimitive, ImageSlice, NineSlice, SliceMode};
pub use path::{PathFill, PathPaintOrder, PathPrimitive, PathStroke};
pub use surface::{BackgroundLayer, Border, BorderSide, Shadow, SurfacePrimitive};
pub use text::{
    PlainTextPrimitive, TextAlign, TextContent, TextDecoration, TextDecorations, TextLayout,
    TextLineHeight, TextOverflow, TextPrimitive, TextStyle,
};

/// Returns a brush transform that maps the unit square into `bounds`.
///
/// This is the common base transform for gradients or image brushes authored in
/// unit coordinates and fitted to a node's local bounds. Callers can compose an
/// animated transform with this value before storing it on a brush-carrying
/// primitive.
#[must_use]
pub fn unit_brush_transform(bounds: Rect) -> Affine {
    Affine::translate(Vec2::new(bounds.x0, bounds.y0))
        * Affine::scale_non_uniform(bounds.width(), bounds.height())
}

/// Resolved drawing primitive stored on a presentation node.
///
/// Primitives are intentionally renderer-agnostic. A toolkit lowerer turns
/// these values plus geometry-tree bounds into backend-specific drawing
/// commands during paint.
///
/// Variants are boxed to keep [`Primitive`] small and keep the
/// `SmallVec<[Primitive; 1]>` storage in [`PresentationNode`] predictable. This
/// intentionally allocates one box per stored primitive.
///
/// [`PresentationNode`]: crate::PresentationNode
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Primitive<ImageKey = u64> {
    /// A resolved box-decoration surface.
    Surface(Box<SurfacePrimitive>),

    /// Resolved text intent.
    Text(Box<TextPrimitive>),

    /// Resolved image intent.
    Image(Box<ImagePrimitive<ImageKey>>),

    /// Resolved path drawing intent.
    Path(Box<PathPrimitive>),
}

impl<ImageKey> Primitive<ImageKey> {
    /// Creates a surface primitive.
    #[must_use]
    pub fn surface(surface: SurfacePrimitive) -> Self {
        Self::Surface(Box::new(surface))
    }

    /// Creates a text primitive.
    #[must_use]
    pub fn text(text: TextPrimitive) -> Self {
        Self::Text(Box::new(text))
    }

    /// Creates a plain text primitive.
    #[must_use]
    pub fn plain_text(text: PlainTextPrimitive) -> Self {
        Self::text(TextPrimitive::plain(text))
    }

    /// Creates an image primitive.
    #[must_use]
    pub fn image(image: ImagePrimitive<ImageKey>) -> Self {
        Self::Image(Box::new(image))
    }

    /// Creates a path primitive.
    #[must_use]
    pub fn path(path: PathPrimitive) -> Self {
        Self::Path(Box::new(path))
    }

    /// Returns this primitive as a surface primitive, if it is one.
    #[must_use]
    pub fn as_surface(&self) -> Option<&SurfacePrimitive> {
        match self {
            Self::Surface(surface) => Some(surface.as_ref()),
            Self::Text(_) | Self::Image(_) | Self::Path(_) => None,
        }
    }

    /// Returns this primitive as a mutable surface primitive, if it is one.
    #[must_use]
    pub fn as_surface_mut(&mut self) -> Option<&mut SurfacePrimitive> {
        match self {
            Self::Surface(surface) => Some(surface.as_mut()),
            Self::Text(_) | Self::Image(_) | Self::Path(_) => None,
        }
    }

    /// Returns this primitive as a text primitive, if it is one.
    #[must_use]
    pub fn as_text(&self) -> Option<&TextPrimitive> {
        match self {
            Self::Surface(_) | Self::Image(_) | Self::Path(_) => None,
            Self::Text(text) => Some(text.as_ref()),
        }
    }

    /// Returns this primitive as a mutable text primitive, if it is one.
    #[must_use]
    pub fn as_text_mut(&mut self) -> Option<&mut TextPrimitive> {
        match self {
            Self::Surface(_) | Self::Image(_) | Self::Path(_) => None,
            Self::Text(text) => Some(text.as_mut()),
        }
    }

    /// Returns this primitive as an image primitive, if it is one.
    #[must_use]
    pub fn as_image(&self) -> Option<&ImagePrimitive<ImageKey>> {
        match self {
            Self::Surface(_) | Self::Text(_) | Self::Path(_) => None,
            Self::Image(image) => Some(image.as_ref()),
        }
    }

    /// Returns this primitive as a mutable image primitive, if it is one.
    #[must_use]
    pub fn as_image_mut(&mut self) -> Option<&mut ImagePrimitive<ImageKey>> {
        match self {
            Self::Surface(_) | Self::Text(_) | Self::Path(_) => None,
            Self::Image(image) => Some(image.as_mut()),
        }
    }

    /// Returns this primitive as a path primitive, if it is one.
    #[must_use]
    pub fn as_path(&self) -> Option<&PathPrimitive> {
        match self {
            Self::Surface(_) | Self::Text(_) | Self::Image(_) => None,
            Self::Path(path) => Some(path.as_ref()),
        }
    }

    /// Returns this primitive as a mutable path primitive, if it is one.
    #[must_use]
    pub fn as_path_mut(&mut self) -> Option<&mut PathPrimitive> {
        match self {
            Self::Surface(_) | Self::Text(_) | Self::Image(_) => None,
            Self::Path(path) => Some(path.as_mut()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peniko::kurbo::Point;

    #[test]
    fn unit_brush_transform_maps_unit_square_to_bounds() {
        let transform = unit_brush_transform(Rect::new(10.0, 20.0, 110.0, 70.0));

        assert_eq!(transform * Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        assert_eq!(transform * Point::new(1.0, 1.0), Point::new(110.0, 70.0));
    }
}
