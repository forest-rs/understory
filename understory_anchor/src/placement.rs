// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// A placement side plus cross-axis alignment.
///
/// Callers use `Placement` in [`AnchorPolicy`](crate::AnchorPolicy) to express
/// preferred and fallback positions. The resolver returns the chosen placement
/// in [`AnchorFrame`](crate::AnchorFrame), and reports every candidate
/// placement through [`AnchorCandidate`](crate::AnchorCandidate).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Placement {
    /// The side of the anchor occupied by the floating surface.
    pub side: Side,
    /// The alignment along the side's cross axis.
    pub align: Align,
}

impl Placement {
    /// Top side aligned to the anchor start edge.
    pub const TOP_START: Self = Self::new(Side::Top, Align::Start);
    /// Top side aligned to the anchor center.
    pub const TOP: Self = Self::new(Side::Top, Align::Center);
    /// Top side aligned to the anchor end edge.
    pub const TOP_END: Self = Self::new(Side::Top, Align::End);
    /// Right side aligned to the anchor start edge.
    pub const RIGHT_START: Self = Self::new(Side::Right, Align::Start);
    /// Right side aligned to the anchor center.
    pub const RIGHT: Self = Self::new(Side::Right, Align::Center);
    /// Right side aligned to the anchor end edge.
    pub const RIGHT_END: Self = Self::new(Side::Right, Align::End);
    /// Bottom side aligned to the anchor start edge.
    pub const BOTTOM_START: Self = Self::new(Side::Bottom, Align::Start);
    /// Bottom side aligned to the anchor center.
    pub const BOTTOM: Self = Self::new(Side::Bottom, Align::Center);
    /// Bottom side aligned to the anchor end edge.
    pub const BOTTOM_END: Self = Self::new(Side::Bottom, Align::End);
    /// Left side aligned to the anchor start edge.
    pub const LEFT_START: Self = Self::new(Side::Left, Align::Start);
    /// Left side aligned to the anchor center.
    pub const LEFT: Self = Self::new(Side::Left, Align::Center);
    /// Left side aligned to the anchor end edge.
    pub const LEFT_END: Self = Self::new(Side::Left, Align::End);

    /// Creates a placement from a side and alignment.
    #[must_use]
    pub const fn new(side: Side, align: Align) -> Self {
        Self { side, align }
    }

    /// Returns this placement with a different side and the same alignment.
    ///
    /// Use this when an adapter maps toolkit placement preferences to fallback
    /// placements while preserving cross-axis alignment.
    #[must_use]
    pub const fn with_side(self, side: Side) -> Self {
        Self {
            side,
            align: self.align,
        }
    }

    /// Returns the side opposite this placement's side.
    #[must_use]
    pub const fn opposite_side(self) -> Side {
        self.side.opposite()
    }

    /// Returns this placement on the opposite side with the same alignment.
    #[must_use]
    pub const fn opposite(self) -> Self {
        self.with_side(self.opposite_side())
    }
}

/// The side of an anchor occupied by a floating surface.
///
/// Callers usually choose a side through [`Placement`] constants such as
/// [`Placement::BOTTOM`]. The chosen side is also returned from
/// [`ArrowFrame`](crate::ArrowFrame) so renderers know which edge owns the
/// arrow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Side {
    /// The floating surface is above the anchor.
    Top,
    /// The floating surface is to the right of the anchor.
    Right,
    /// The floating surface is below the anchor.
    Bottom,
    /// The floating surface is to the left of the anchor.
    Left,
}

impl Side {
    /// Returns the opposite physical side.
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Right => Self::Left,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
        }
    }
}

/// Cross-axis alignment for a placement.
///
/// Callers use this when constructing a [`Placement`] directly. `Start` and
/// `End` are physical edges in the placement's cross axis; this crate does not
/// own writing-mode or locale mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Align {
    /// Align to the start edge in the placement's cross axis.
    Start,
    /// Align centers in the placement's cross axis.
    Center,
    /// Align to the end edge in the placement's cross axis.
    End,
}

#[cfg(test)]
mod tests {
    use super::{Align, Placement, Side};

    #[test]
    fn placement_side_helpers_preserve_alignment() {
        let placement = Placement::new(Side::Top, Align::End);

        assert_eq!(placement.opposite_side(), Side::Bottom);
        assert_eq!(
            placement.opposite(),
            Placement::new(Side::Bottom, Align::End)
        );
        assert_eq!(
            placement.with_side(Side::Left),
            Placement::new(Side::Left, Align::End),
        );
    }

    #[test]
    fn side_opposite_round_trips() {
        for side in [Side::Top, Side::Right, Side::Bottom, Side::Left] {
            assert_eq!(side.opposite().opposite(), side);
        }
    }
}
