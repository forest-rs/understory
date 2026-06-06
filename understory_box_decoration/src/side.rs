// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// A physical side of a rectangular box.
///
/// `Side` deliberately uses physical, y-down coordinates: top, right, bottom,
/// and left. Logical sides, writing modes, and shorthand expansion belong in a
/// value or style layer before geometry is resolved.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Side {
    /// The smaller-y side of the box.
    Top,
    /// The larger-x side of the box.
    Right,
    /// The larger-y side of the box.
    Bottom,
    /// The smaller-x side of the box.
    Left,
}

impl Side {
    /// All physical sides in clockwise order.
    pub const ALL: [Self; 4] = [Self::Top, Self::Right, Self::Bottom, Self::Left];
}

/// A CSS-style box area that can be clipped or painted.
///
/// The geometry kernel resolves these areas only after it has concrete border
/// and padding widths. Margin geometry is intentionally absent because margin
/// belongs to layout rather than box decoration painting.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BoxArea {
    /// The border edge / border box.
    Border,
    /// The padding edge / padding box.
    Padding,
    /// The content edge / content box.
    Content,
}
