// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Resolved CSS `border-style` value for one physical border side.
///
/// Stored in [`BoxDecorationGeometry`](crate::BoxDecorationGeometry) and
/// exposed through [`BorderSidePaintGeometry`](crate::BorderSidePaintGeometry)
/// so renderers can choose side-specific border lowering. This crate records
/// the value and exposes the associated geometry; it does not implement
/// dashed, dotted, double, groove, ridge, inset, or outset painting.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum BorderStyle {
    /// No border is drawn for this side.
    #[default]
    None,
    /// No border is drawn for this side.
    ///
    /// CSS uses `hidden` differently from `none` during table border conflict
    /// resolution. This geometry crate preserves the distinction but treats it
    /// as non-painting.
    Hidden,
    /// A dotted border.
    Dotted,
    /// A dashed border.
    Dashed,
    /// A single solid border.
    Solid,
    /// Two parallel solid border lines.
    Double,
    /// A carved groove border.
    Groove,
    /// A raised ridge border.
    Ridge,
    /// An inset border.
    Inset,
    /// An outset border.
    Outset,
}

impl BorderStyle {
    /// Returns true when this style should produce visible border pixels.
    #[must_use]
    pub const fn paints_border(self) -> bool {
        !matches!(self, Self::None | Self::Hidden)
    }
}
