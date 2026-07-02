// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::BezPath;

use crate::path::write_contour_side_segment_path;
use crate::{BorderStyle, BoxDecorationGeometry, Side};

/// Fill rule required to interpret a border fragment path.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum BorderFillRule {
    /// The nonzero winding rule.
    #[default]
    NonZero,
    /// The even-odd winding rule.
    EvenOdd,
}

/// A path used as a clip while painting a border fragment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BorderClip {
    /// Clip path.
    pub path: BezPath,
    /// Fill rule for the clip path.
    pub fill_rule: BorderFillRule,
}

impl BorderClip {
    /// Creates a clip path with an explicit fill rule.
    #[must_use]
    pub fn new(path: BezPath, fill_rule: BorderFillRule) -> Self {
        Self { path, fill_rule }
    }
}

/// Clip stack needed to paint a resolved border fragment.
///
/// Renderers should push `side` first and `ring` second when both are present.
/// This order makes side ownership the broad partition and the ring the final
/// border-area constraint.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BorderClipStack {
    /// Optional side-ownership clip for a physical side.
    pub side: Option<BorderClip>,
    /// Optional border-ring clip.
    pub ring: Option<BorderClip>,
}

impl BorderClipStack {
    /// Returns an empty clip stack.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            side: None,
            ring: None,
        }
    }

    /// Returns a stack with one side-ownership clip.
    #[must_use]
    pub fn side(path: BezPath) -> Self {
        Self {
            side: Some(BorderClip::new(path, BorderFillRule::NonZero)),
            ring: None,
        }
    }

    /// Returns a stack with a side-ownership clip and a border-ring clip.
    #[must_use]
    pub fn side_and_ring(side: BezPath, ring: BezPath) -> Self {
        Self {
            side: Some(BorderClip::new(side, BorderFillRule::NonZero)),
            ring: Some(BorderClip::new(ring, BorderFillRule::EvenOdd)),
        }
    }
}

/// A fillable border fragment.
///
/// The fragment may be a full border ring, a band inside that ring, or a
/// side-owned view of either. Renderers own brush selection and draw ordering;
/// this type only describes resolved geometry and clipping.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BorderFillFragment {
    /// Fill path.
    pub path: BezPath,
    /// Fill rule for `path`.
    pub fill_rule: BorderFillRule,
    /// Clips to push before filling `path`.
    pub clips: BorderClipStack,
}

impl BorderFillFragment {
    /// Creates a fill fragment.
    #[must_use]
    pub fn new(path: BezPath, fill_rule: BorderFillRule, clips: BorderClipStack) -> Self {
        Self {
            path,
            fill_rule,
            clips,
        }
    }
}

/// A strokeable border fragment.
///
/// This path is open for side fragments. Renderers own stroke width, caps,
/// joins, dash pattern, brush selection, and draw ordering.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BorderStrokeFragment {
    /// Stroke path.
    pub path: BezPath,
    /// Clips to push before stroking `path`.
    pub clips: BorderClipStack,
}

impl BorderStrokeFragment {
    /// Creates a stroke fragment.
    #[must_use]
    pub fn new(path: BezPath, clips: BorderClipStack) -> Self {
        Self { path, clips }
    }
}

/// A normalized band inside the border ring.
///
/// `outer` and `inner` are in border-ring interpolation space: `0.0` is the
/// outer border edge and `1.0` is the inner padding edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderBand {
    /// Outer contour interpolation value.
    pub outer: f64,
    /// Inner contour interpolation value.
    pub inner: f64,
}

impl BorderBand {
    /// The full border ring.
    pub const FULL: Self = Self::new(0.0, 1.0);

    /// Creates a band from outer and inner interpolation values.
    #[must_use]
    pub const fn new(outer: f64, inner: f64) -> Self {
        Self { outer, inner }
    }
}

/// Resolved paint geometry for a box border.
///
/// This is the canonical border-lowering entry point. It groups the ring,
/// band, side, and stroke-fragment operations so renderers do not need to
/// reconstruct side ownership from raw box geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderPaintGeometry {
    geometry: BoxDecorationGeometry,
}

impl BorderPaintGeometry {
    /// Creates border paint geometry from resolved box decoration geometry.
    #[must_use]
    pub const fn new(geometry: BoxDecorationGeometry) -> Self {
        Self { geometry }
    }

    /// Returns the underlying resolved box decoration geometry.
    #[must_use]
    pub const fn geometry(self) -> BoxDecorationGeometry {
        self.geometry
    }

    /// Returns the full border ring as an even-odd fill fragment.
    #[must_use]
    pub fn ring(self) -> BorderFillFragment {
        BorderFillFragment::new(
            self.geometry.to_border_ring_path(),
            BorderFillRule::EvenOdd,
            BorderClipStack::empty(),
        )
    }

    /// Returns a band inside the border ring as an even-odd fill fragment.
    #[must_use]
    pub fn band(self, band: BorderBand) -> BorderFillFragment {
        BorderFillFragment::new(
            self.geometry.to_border_band_path(band.outer, band.inner),
            BorderFillRule::EvenOdd,
            BorderClipStack::empty(),
        )
    }

    /// Returns geometry for one physical border side.
    #[must_use]
    pub const fn side(self, side: Side) -> BorderSidePaintGeometry {
        BorderSidePaintGeometry::new(self.geometry, side)
    }
}

/// Resolved paint geometry for one physical border side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderSidePaintGeometry {
    geometry: BoxDecorationGeometry,
    side: Side,
}

impl BorderSidePaintGeometry {
    /// Creates side paint geometry from resolved box decoration geometry.
    #[must_use]
    pub const fn new(geometry: BoxDecorationGeometry, side: Side) -> Self {
        Self { geometry, side }
    }

    /// Returns the physical side.
    #[must_use]
    pub const fn side(self) -> Side {
        self.side
    }

    /// Returns the resolved style for this side.
    #[must_use]
    pub const fn style(self) -> BorderStyle {
        match self.side {
            Side::Top => self.geometry.border_styles.top,
            Side::Right => self.geometry.border_styles.right,
            Side::Bottom => self.geometry.border_styles.bottom,
            Side::Left => self.geometry.border_styles.left,
        }
    }

    /// Returns the visible border width for this side.
    #[must_use]
    pub const fn width(self) -> f64 {
        match self.side {
            Side::Top => self.geometry.border_widths.top,
            Side::Right => self.geometry.border_widths.right,
            Side::Bottom => self.geometry.border_widths.bottom,
            Side::Left => self.geometry.border_widths.left,
        }
    }

    /// Returns true when this side has no visible border pixels.
    #[must_use]
    pub fn is_empty(self) -> bool {
        !self.style().paints_border() || !self.width().is_finite() || self.width() <= 0.0
    }

    /// Returns this side's owned share of the full border ring.
    #[must_use]
    pub fn ring(self) -> BorderFillFragment {
        BorderFillFragment::new(
            self.geometry.to_border_ring_path(),
            BorderFillRule::EvenOdd,
            BorderClipStack::side(self.geometry.to_border_side_clip_path(self.side)),
        )
    }

    /// Returns this side's owned share of a band inside the border ring.
    #[must_use]
    pub fn band(self, band: BorderBand) -> BorderFillFragment {
        BorderFillFragment::new(
            self.geometry.to_border_band_path(band.outer, band.inner),
            BorderFillRule::EvenOdd,
            BorderClipStack::side(self.geometry.to_border_side_clip_path(self.side)),
        )
    }

    /// Returns an open contour stroke fragment for this side.
    ///
    /// The path spans the central side plus the side-owned halves of the two
    /// adjacent corners. It is clipped to both the side ownership polygon and
    /// the border ring so renderer stroke caps and joins cannot leak outside
    /// the resolved border area.
    #[must_use]
    pub fn stroke_contour(self, position: f64) -> BorderStrokeFragment {
        let contour = self.geometry.border_contour_at(position);
        let mut path = BezPath::new();
        write_contour_side_segment_path(contour, self.side, &mut path);
        BorderStrokeFragment::new(
            path,
            BorderClipStack::side_and_ring(
                self.geometry.to_border_side_clip_path(self.side),
                self.geometry.to_border_ring_path(),
            ),
        )
    }
}

impl BoxDecorationGeometry {
    /// Returns the canonical border paint geometry view for this box.
    #[must_use]
    pub const fn border_paint(self) -> BorderPaintGeometry {
        BorderPaintGeometry::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CornerRadii, CornerShape, CornerShapes, Edges, Superellipse};
    use kurbo::{PathEl, Rect, Size};

    #[test]
    fn border_paint_ring_and_band_are_even_odd_fragments() {
        let geometry = rounded_geometry().border_paint();

        let ring = geometry.ring();
        let band = geometry.band(BorderBand::new(0.0, 1.0 / 3.0));

        assert_eq!(ring.fill_rule, BorderFillRule::EvenOdd);
        assert_eq!(band.fill_rule, BorderFillRule::EvenOdd);
        assert_eq!(ring.clips, BorderClipStack::empty());
        assert_eq!(band.clips, BorderClipStack::empty());
        assert_eq!(close_count(&ring.path), 2);
        assert_eq!(close_count(&band.path), 2);
        assert!(path_is_finite(&ring.path));
        assert!(path_is_finite(&band.path));
    }

    #[test]
    fn side_fill_fragments_carry_side_ownership_clip() {
        let side = rounded_geometry().border_paint().side(Side::Right);

        let ring = side.ring();
        let band = side.band(BorderBand::new(2.0 / 3.0, 1.0));

        assert_eq!(ring.fill_rule, BorderFillRule::EvenOdd);
        assert_eq!(band.fill_rule, BorderFillRule::EvenOdd);
        assert_eq!(close_count(&ring.path), 2);
        assert_eq!(close_count(&band.path), 2);
        assert_eq!(
            ring.clips.side.as_ref().map(|clip| clip.fill_rule),
            Some(BorderFillRule::NonZero)
        );
        assert!(ring.clips.ring.is_none());
        assert_eq!(
            band.clips.side.as_ref().map(|clip| clip.fill_rule),
            Some(BorderFillRule::NonZero)
        );
        assert!(band.clips.ring.is_none());
    }

    #[test]
    fn side_stroke_fragment_is_open_and_clipped_to_side_and_ring() {
        let stroke = rounded_geometry()
            .border_paint()
            .side(Side::Top)
            .stroke_contour(0.5);

        assert_eq!(close_count(&stroke.path), 0);
        assert!(
            stroke
                .path
                .iter()
                .any(|element| matches!(element, PathEl::CurveTo(..)))
        );
        assert_eq!(
            stroke.clips.side.as_ref().map(|clip| clip.fill_rule),
            Some(BorderFillRule::NonZero)
        );
        assert_eq!(
            stroke.clips.ring.as_ref().map(|clip| clip.fill_rule),
            Some(BorderFillRule::EvenOdd)
        );
        assert_eq!(
            stroke
                .clips
                .side
                .as_ref()
                .map(|clip| close_count(&clip.path)),
            Some(1)
        );
        assert_eq!(
            stroke
                .clips
                .ring
                .as_ref()
                .map(|clip| close_count(&clip.path)),
            Some(2)
        );
        assert!(path_is_finite(&stroke.path));
    }

    #[test]
    fn side_stroke_fragments_preserve_shaped_half_corners() {
        let geometry = BoxDecorationGeometry::from_border_box(
            Rect::new(0.0, 0.0, 140.0, 88.0),
            Edges::all(12.0),
            Edges::ZERO,
            CornerRadii::all(Size::new(26.0, 18.0)),
            CornerShapes::new(
                CornerShape::Square,
                CornerShape::Superellipse(Superellipse::new(2.0)),
                CornerShape::Bevel,
                CornerShape::scoop(),
            ),
        );

        for side in [Side::Top, Side::Right, Side::Bottom, Side::Left] {
            let stroke = geometry.border_paint().side(side).stroke_contour(0.5);
            assert_eq!(close_count(&stroke.path), 0);
            assert!(stroke.path.iter().count() >= 3);
            assert!(path_is_finite(&stroke.path));
        }
    }

    fn rounded_geometry() -> BoxDecorationGeometry {
        BoxDecorationGeometry::from_round_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::new(6.0, 12.0, 18.0, 24.0),
            Edges::ZERO,
            CornerRadii::uniform(20.0),
        )
    }

    fn close_count(path: &BezPath) -> usize {
        path.iter()
            .filter(|element| matches!(element, PathEl::ClosePath))
            .count()
    }

    fn path_is_finite(path: &BezPath) -> bool {
        path.iter().all(|element| match element {
            PathEl::MoveTo(point) | PathEl::LineTo(point) => {
                point.x.is_finite() && point.y.is_finite()
            }
            PathEl::QuadTo(control, point) => {
                control.x.is_finite()
                    && control.y.is_finite()
                    && point.x.is_finite()
                    && point.y.is_finite()
            }
            PathEl::CurveTo(control_0, control_1, point) => {
                control_0.x.is_finite()
                    && control_0.y.is_finite()
                    && control_1.x.is_finite()
                    && control_1.y.is_finite()
                    && point.x.is_finite()
                    && point.y.is_finite()
            }
            PathEl::ClosePath => true,
        })
    }
}
