// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Presentation primitives.

use alloc::boxed::Box;

mod surface;
mod text;

pub use surface::{BackgroundLayer, Border, BorderSide, Shadow, SurfacePrimitive};
pub use text::{
    PlainTextPrimitive, TextAlign, TextContent, TextLayout, TextLineHeight, TextOverflow,
    TextPrimitive, TextStyle,
};

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
pub enum Primitive {
    /// A resolved box-decoration surface.
    Surface(Box<SurfacePrimitive>),

    /// Resolved text intent.
    Text(Box<TextPrimitive>),
}

impl Primitive {
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

    /// Returns this primitive as a surface primitive, if it is one.
    #[must_use]
    pub fn as_surface(&self) -> Option<&SurfacePrimitive> {
        match self {
            Self::Surface(surface) => Some(surface.as_ref()),
            Self::Text(_) => None,
        }
    }

    /// Returns this primitive as a mutable surface primitive, if it is one.
    #[must_use]
    pub fn as_surface_mut(&mut self) -> Option<&mut SurfacePrimitive> {
        match self {
            Self::Surface(surface) => Some(surface.as_mut()),
            Self::Text(_) => None,
        }
    }

    /// Returns this primitive as a text primitive, if it is one.
    #[must_use]
    pub fn as_text(&self) -> Option<&TextPrimitive> {
        match self {
            Self::Surface(_) => None,
            Self::Text(text) => Some(text.as_ref()),
        }
    }

    /// Returns this primitive as a mutable text primitive, if it is one.
    #[must_use]
    pub fn as_text_mut(&mut self) -> Option<&mut TextPrimitive> {
        match self {
            Self::Surface(_) => None,
            Self::Text(text) => Some(text.as_mut()),
        }
    }
}
