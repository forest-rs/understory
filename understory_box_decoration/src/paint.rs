// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{BezPath, ParamCurveArclen};

use crate::math::floor;
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

    /// Returns a stack with one border-ring clip.
    #[must_use]
    pub fn ring(path: BezPath) -> Self {
        Self {
            side: None,
            ring: Some(BorderClip::new(path, BorderFillRule::EvenOdd)),
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
/// This path is open for side fragments. Renderers own brush selection and
/// draw ordering; [`BorderPaintStroke`] carries the resolved stroke parameters.
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

/// Line cap to use when stroking a border fragment.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum BorderStrokeCap {
    /// Stroke endpoints stop exactly at the path endpoint.
    #[default]
    Butt,
    /// Stroke endpoints are rounded.
    Round,
    /// Stroke endpoints extend by half the stroke width.
    Square,
}

/// Resolved two-entry dash pattern for a border stroke.
///
/// The first length is the painted segment and the second length is the gap.
/// Dotted borders use a tiny painted segment with a round cap so backends that
/// discard zero-length dashes still render visible dots.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderDashPattern {
    /// Dash phase offset.
    pub offset: f64,
    /// Painted and unpainted segment lengths.
    pub lengths: [f64; 2],
}

impl BorderDashPattern {
    /// Creates a resolved dash pattern.
    #[must_use]
    pub const fn new(offset: f64, lengths: [f64; 2]) -> Self {
        Self { offset, lengths }
    }
}

/// Resolved stroke parameters for a border stroke command.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BorderStrokeSpec {
    /// Stroke width in local coordinate units.
    pub width: f64,
    /// Stroke endpoint cap.
    pub cap: BorderStrokeCap,
    /// Optional dash pattern.
    pub dash: Option<BorderDashPattern>,
}

impl BorderStrokeSpec {
    /// Creates solid stroke parameters with the default butt cap.
    #[must_use]
    pub const fn solid(width: f64) -> Self {
        Self {
            width,
            cap: BorderStrokeCap::Butt,
            dash: None,
        }
    }

    /// Creates stroke parameters with an explicit cap and dash pattern.
    #[must_use]
    pub const fn with_dash(width: f64, cap: BorderStrokeCap, dash: BorderDashPattern) -> Self {
        Self {
            width,
            cap,
            dash: Some(dash),
        }
    }
}

/// A normalized band inside the border ring.
///
/// `outer` and `inner` are in border-ring interpolation space: `0.0` is the
/// outer border edge and `1.0` is the inner padding edge. Fragment builders
/// clamp values into this interval and clamp `inner` to be no smaller than
/// `outer`; non-finite values are treated as `0.0`.
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

/// Paint sharing mode for a border paint plan.
///
/// This is supplied by the renderer or presentation layer because
/// `understory_box_decoration` does not own brushes. A caller should use
/// [`BorderPaintMode::Shared`] only when the visible sides can share one paint
/// source. Otherwise, use [`BorderPaintMode::PerSide`] so each physical side
/// gets side-owned fragments.
///
/// The mode is an optimization hint, not a command-shape guarantee: a shared
/// plan falls back to per-side lowering whenever the sides do not share one
/// whole-contour lowering (mixed styles, relief styles, or stroked styles
/// with mixed widths), and those commands carry
/// [`BorderPaintSource::Side`] sources. Renderers must therefore map both
/// source variants regardless of the requested mode.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BorderPaintMode {
    /// Visible sides share one paint source where geometry permits it.
    Shared,
    /// Visible sides are painted independently.
    PerSide,
}

/// Source slot for a border paint command.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BorderPaintSource {
    /// Use the caller's shared border paint.
    Shared,
    /// Use the paint for one physical side.
    Side(Side),
}

/// Semantic role for a fill command.
///
/// Renderers use this to choose a brush while leaving geometry and draw order
/// in the plan. Plans paint each border region exactly once, so a role selects
/// the brush for its fragment; roles are never composited on top of each
/// other.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BorderPaintRole {
    /// Paint with the command source's border brush unchanged.
    Base,
    /// Paint with a brush derived from the command source's border brush.
    ///
    /// Relief regions replace [`BorderPaintRole::Base`] for `inset`, `outset`,
    /// `groove`, and `ridge`; they are not translucent overlays above a base
    /// coat. Renderers should lighten or darken the source's border brush in
    /// the manner of CSS relief border colors.
    Relief(BorderReliefShade),
}

/// Relative shade used by relief-style border fills.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BorderReliefShade {
    /// Lighter relief shade.
    Highlight,
    /// Darker relief shade.
    Shadow,
}

/// One fill command in a resolved border paint plan.
#[derive(Clone, Debug, PartialEq)]
pub struct BorderPaintFill {
    /// Paint source to use for this command.
    pub source: BorderPaintSource,
    /// CSS-style border style that produced this command.
    pub style: BorderStyle,
    /// Semantic paint role.
    pub role: BorderPaintRole,
    /// Fill geometry and clip stack.
    pub fragment: BorderFillFragment,
}

impl BorderPaintFill {
    /// Creates a fill command.
    #[must_use]
    pub fn new(
        source: BorderPaintSource,
        style: BorderStyle,
        role: BorderPaintRole,
        fragment: BorderFillFragment,
    ) -> Self {
        Self {
            source,
            style,
            role,
            fragment,
        }
    }
}

/// One stroke command in a resolved border paint plan.
#[derive(Clone, Debug, PartialEq)]
pub struct BorderPaintStroke {
    /// Paint source to use for this command.
    pub source: BorderPaintSource,
    /// CSS-style border style that produced this command.
    pub style: BorderStyle,
    /// Stroke parameters for this command.
    pub stroke: BorderStrokeSpec,
    /// Stroke geometry and clip stack.
    pub fragment: BorderStrokeFragment,
}

impl BorderPaintStroke {
    /// Creates a stroke command.
    #[must_use]
    pub fn new(
        source: BorderPaintSource,
        style: BorderStyle,
        stroke: BorderStrokeSpec,
        fragment: BorderStrokeFragment,
    ) -> Self {
        Self {
            source,
            style,
            stroke,
            fragment,
        }
    }
}

/// One command in a resolved border paint plan.
#[derive(Clone, Debug, PartialEq)]
pub enum BorderPaintCommand {
    /// Fill a border fragment.
    Fill(BorderPaintFill),
    /// Stroke a border fragment.
    Stroke(BorderPaintStroke),
}

/// Resolved border paint plan.
///
/// A plan converts resolved border geometry and per-side styles into a small
/// sequence of renderer-neutral fill and stroke commands. It deliberately does
/// not carry brushes; renderers map [`BorderPaintSource`] and
/// [`BorderPaintRole`] to their own paint commands.
///
/// The plan itself is a cheap copyable description; the paths, arc lengths,
/// and dash fitting are computed on every [`BorderPaintPlan::for_each`] call.
/// Retained renderers should emit once and store the commands, re-running the
/// plan only when the resolved geometry or styles change.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderPaintPlan {
    geometry: BoxDecorationGeometry,
    mode: BorderPaintMode,
}

impl BorderPaintPlan {
    /// Creates a border paint plan from resolved geometry and a sharing mode.
    #[must_use]
    pub const fn new(geometry: BoxDecorationGeometry, mode: BorderPaintMode) -> Self {
        Self { geometry, mode }
    }

    /// Returns the underlying resolved box decoration geometry.
    #[must_use]
    pub const fn geometry(self) -> BoxDecorationGeometry {
        self.geometry
    }

    /// Returns the paint sharing mode.
    #[must_use]
    pub const fn mode(self) -> BorderPaintMode {
        self.mode
    }

    /// Calls `paint` for each command in draw order.
    pub fn for_each(self, mut paint: impl FnMut(BorderPaintCommand)) {
        match self.mode {
            BorderPaintMode::Shared => self.for_each_shared(&mut paint),
            BorderPaintMode::PerSide => self.for_each_per_side(&mut paint),
        }
    }

    fn for_each_shared(self, paint: &mut impl FnMut(BorderPaintCommand)) {
        let Some(style) = uniform_visible_style(self.geometry) else {
            self.for_each_per_side(paint);
            return;
        };

        let source = BorderPaintSource::Shared;
        let paint_geometry = self.geometry.border_paint();
        match style {
            BorderStyle::None | BorderStyle::Hidden => {}
            BorderStyle::Solid => {
                // A ring fill is exact for every corner shape and per-side
                // width mix, unlike a centerline stroke, whose constant width
                // only matches the band between the border and padding
                // contours for circular corners.
                paint(BorderPaintCommand::Fill(BorderPaintFill::new(
                    source,
                    style,
                    BorderPaintRole::Base,
                    paint_geometry.ring(),
                )));
            }
            BorderStyle::Dotted | BorderStyle::Dashed => {
                let Some(width) = shared_visible_width(self.geometry) else {
                    self.for_each_per_side(paint);
                    return;
                };
                let fragment = paint_geometry.stroke_contour(0.5);
                paint(BorderPaintCommand::Stroke(BorderPaintStroke::new(
                    source,
                    style,
                    stroke_spec_for_closed_contour(style, width, &fragment),
                    fragment,
                )));
            }
            BorderStyle::Double => {
                paint(BorderPaintCommand::Fill(BorderPaintFill::new(
                    source,
                    style,
                    BorderPaintRole::Base,
                    paint_geometry.band(BorderBand::new(0.0, 1.0 / 3.0)),
                )));
                paint(BorderPaintCommand::Fill(BorderPaintFill::new(
                    source,
                    style,
                    BorderPaintRole::Base,
                    paint_geometry.band(BorderBand::new(2.0 / 3.0, 1.0)),
                )));
            }
            BorderStyle::Groove | BorderStyle::Ridge | BorderStyle::Inset | BorderStyle::Outset => {
                self.for_each_per_side(paint);
            }
        }
    }

    fn for_each_per_side(self, paint: &mut impl FnMut(BorderPaintCommand)) {
        let paint_geometry = self.geometry.border_paint();
        for side in ordered_visible_sides(self.geometry) {
            let side_geometry = paint_geometry.side(side);
            if side_geometry.is_empty() {
                continue;
            }
            emit_side_commands(side_geometry, paint);
        }
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

    /// Returns a border paint plan for the requested paint sharing mode.
    #[must_use]
    pub const fn plan(self, mode: BorderPaintMode) -> BorderPaintPlan {
        BorderPaintPlan::new(self.geometry, mode)
    }

    /// Returns a paint plan for a shared visible border paint.
    #[must_use]
    pub const fn shared_plan(self) -> BorderPaintPlan {
        self.plan(BorderPaintMode::Shared)
    }

    /// Returns a paint plan for independent per-side border paint.
    #[must_use]
    pub const fn per_side_plan(self) -> BorderPaintPlan {
        self.plan(BorderPaintMode::PerSide)
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

    /// Returns a whole-contour centerline stroke fragment through the border
    /// ring.
    ///
    /// Shared-paint `dashed` and `dotted` borders use this so dash phase
    /// progresses continuously around the resolved shape instead of
    /// restarting at every physical side. The fragment is clipped to the
    /// border ring so renderer stroke width cannot leak outside the resolved
    /// border area. Solid borders should fill [`BorderPaintGeometry::ring`]
    /// instead: a constant-width centerline stroke only matches the border
    /// band exactly for circular corners.
    #[must_use]
    pub fn stroke_contour(self, position: f64) -> BorderStrokeFragment {
        let mut path = BezPath::new();
        self.geometry
            .border_contour_at(position)
            .write_path(&mut path);
        BorderStrokeFragment::new(
            path,
            BorderClipStack::ring(self.geometry.to_border_ring_path()),
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
    ///
    /// Filled styles use a real side-bounded path so adjacent sides meet at
    /// shaped corner miter boundaries. Shared-paint borders can use
    /// [`BorderPaintGeometry::ring`] when they need one whole-ring fill.
    #[must_use]
    pub fn ring(self) -> BorderFillFragment {
        BorderFillFragment::new(
            self.geometry.to_border_side_band_path(self.side, 0.0, 1.0),
            BorderFillRule::NonZero,
            BorderClipStack::empty(),
        )
    }

    /// Returns this side's owned share of a band inside the border ring.
    #[must_use]
    pub fn band(self, band: BorderBand) -> BorderFillFragment {
        BorderFillFragment::new(
            self.geometry
                .to_border_side_band_path(self.side, band.outer, band.inner),
            BorderFillRule::NonZero,
            BorderClipStack::empty(),
        )
    }

    /// Returns an open contour stroke fragment for this side.
    ///
    /// The path spans the central side plus the side-owned miter-bounded parts
    /// of the two adjacent corners. It is clipped to the border ring so
    /// renderer stroke width cannot leak outside the resolved border area;
    /// side ownership is carried by the path itself.
    #[must_use]
    pub fn stroke_contour(self, position: f64) -> BorderStrokeFragment {
        self.stroke_contour_with_clips(
            position,
            BorderClipStack::ring(self.geometry.to_border_ring_path()),
        )
    }

    fn owned_stroke_contour(self, position: f64) -> BorderStrokeFragment {
        self.stroke_contour_with_clips(
            position,
            BorderClipStack::side_and_ring(
                self.geometry.to_border_side_clip_path(self.side),
                self.geometry.to_border_ring_path(),
            ),
        )
    }

    fn stroke_contour_with_clips(
        self,
        position: f64,
        clips: BorderClipStack,
    ) -> BorderStrokeFragment {
        let contour = self.geometry.border_contour_at(position);
        let mut path = BezPath::new();
        write_contour_side_segment_path(
            contour,
            self.geometry.border_box,
            self.geometry.padding_box,
            self.side,
            &mut path,
        );
        BorderStrokeFragment::new(path, clips)
    }
}

fn emit_side_commands(
    geometry: BorderSidePaintGeometry,
    paint: &mut impl FnMut(BorderPaintCommand),
) {
    let side = geometry.side();
    let source = BorderPaintSource::Side(side);
    let style = geometry.style();
    match style {
        BorderStyle::None | BorderStyle::Hidden => {}
        BorderStyle::Solid => {
            paint(BorderPaintCommand::Fill(BorderPaintFill::new(
                source,
                style,
                BorderPaintRole::Base,
                geometry.ring(),
            )));
        }
        BorderStyle::Dotted | BorderStyle::Dashed => {
            let fragment = geometry.owned_stroke_contour(0.5);
            paint(BorderPaintCommand::Stroke(BorderPaintStroke::new(
                source,
                style,
                stroke_spec_for_open_contour(style, geometry.width(), &fragment),
                fragment,
            )));
        }
        BorderStyle::Double => {
            paint(BorderPaintCommand::Fill(BorderPaintFill::new(
                source,
                style,
                BorderPaintRole::Base,
                geometry.band(BorderBand::new(0.0, 1.0 / 3.0)),
            )));
            paint(BorderPaintCommand::Fill(BorderPaintFill::new(
                source,
                style,
                BorderPaintRole::Base,
                geometry.band(BorderBand::new(2.0 / 3.0, 1.0)),
            )));
        }
        BorderStyle::Inset | BorderStyle::Outset => {
            paint(BorderPaintCommand::Fill(BorderPaintFill::new(
                source,
                style,
                BorderPaintRole::Relief(inset_outset_shade(side, style)),
                geometry.ring(),
            )));
        }
        BorderStyle::Groove | BorderStyle::Ridge => {
            let (outer, inner) = groove_ridge_shades(side, style);
            paint(BorderPaintCommand::Fill(BorderPaintFill::new(
                source,
                style,
                BorderPaintRole::Relief(outer),
                geometry.band(BorderBand::new(0.0, 0.5)),
            )));
            paint(BorderPaintCommand::Fill(BorderPaintFill::new(
                source,
                style,
                BorderPaintRole::Relief(inner),
                geometry.band(BorderBand::new(0.5, 1.0)),
            )));
        }
    }
}

fn ordered_visible_sides(geometry: BoxDecorationGeometry) -> [Side; 4] {
    let mut sides = Side::ALL;
    let mut i = 1;
    while i < sides.len() {
        let side = sides[i];
        let mut j = i;
        while j > 0 && side_sort_key(geometry, side) < side_sort_key(geometry, sides[j - 1]) {
            sides[j] = sides[j - 1];
            j -= 1;
        }
        sides[j] = side;
        i += 1;
    }
    sides
}

fn side_sort_key(geometry: BoxDecorationGeometry, side: Side) -> (u8, u8) {
    let style = style_for_side(geometry, side);
    (style_priority(style), side_priority(side))
}

fn style_priority(style: BorderStyle) -> u8 {
    match style {
        BorderStyle::None | BorderStyle::Hidden => 0,
        BorderStyle::Dotted | BorderStyle::Dashed | BorderStyle::Double => 1,
        BorderStyle::Inset | BorderStyle::Groove | BorderStyle::Outset | BorderStyle::Ridge => 2,
        BorderStyle::Solid => 3,
    }
}

fn stroke_spec_for_style(style: BorderStyle, width: f64) -> BorderStrokeSpec {
    const DOT_DASH_LENGTH: f64 = 1.0e-2;

    let width = if width.is_finite() && width > 0.0 {
        width
    } else {
        0.0
    };

    match style {
        BorderStyle::Dotted => {
            // CSS-style dotted borders are zero-length round-capped dashes, but
            // Kurbo drops zero-length dash segments. Emit the smallest useful
            // painted segment and keep the visual period at roughly 2w.
            let dot = width.min(DOT_DASH_LENGTH);
            BorderStrokeSpec::with_dash(
                width,
                BorderStrokeCap::Round,
                BorderDashPattern::new(0.0, [dot, (width * 2.0 - dot).max(width)]),
            )
        }
        BorderStyle::Dashed => {
            let (dash, gap) = if width >= 3.0 {
                (width * 2.0, width)
            } else {
                (width * 3.0, width * 2.0)
            };
            BorderStrokeSpec::with_dash(
                width,
                BorderStrokeCap::Butt,
                BorderDashPattern::new(0.0, [dash, gap]),
            )
        }
        _ => BorderStrokeSpec::solid(width),
    }
}

fn stroke_spec_for_closed_contour(
    style: BorderStyle,
    width: f64,
    fragment: &BorderStrokeFragment,
) -> BorderStrokeSpec {
    let mut stroke = stroke_spec_for_style(style, width);
    let Some(dash) = stroke.dash else {
        return stroke;
    };
    let Some(balanced) = balanced_closed_dash_pattern(dash.lengths, &fragment.path) else {
        return stroke;
    };
    stroke.dash = Some(balanced);
    stroke
}

fn stroke_spec_for_open_contour(
    style: BorderStyle,
    width: f64,
    fragment: &BorderStrokeFragment,
) -> BorderStrokeSpec {
    let mut stroke = stroke_spec_for_style(style, width);
    let Some(dash) = stroke.dash else {
        return stroke;
    };
    let Some(balanced) = balanced_open_dash_pattern(dash.lengths, &fragment.path) else {
        return stroke;
    };
    stroke.dash = Some(balanced);
    stroke
}

/// Balances a dash pattern so full painted segments anchor both endpoints of
/// an open contour.
///
/// The gap is stretched or squeezed to the nearest length where
/// `length = (n + 1) * painted + n * gap` holds for a whole number of gaps
/// `n`, so side strokes start and end with a dash at their corner miter
/// boundaries instead of getting cut mid-dash.
fn balanced_open_dash_pattern(lengths: [f64; 2], path: &BezPath) -> Option<BorderDashPattern> {
    let painted = lengths[0];
    let base_gap = lengths[1];
    if !painted.is_finite() || painted < 0.0 || !base_gap.is_finite() || base_gap <= 0.0 {
        return None;
    }

    let length = path_length(path);
    let base_period = painted + base_gap;
    if !length.is_finite() || length <= painted || base_period <= 0.0 {
        return None;
    }

    let gaps = floor((length - painted) / base_period + 0.5).max(1.0);
    let gap = (length - painted * (gaps + 1.0)) / gaps;
    if !gap.is_finite() || gap <= 0.0 {
        return None;
    }
    Some(BorderDashPattern::new(0.0, [painted, gap]))
}

fn balanced_closed_dash_pattern(lengths: [f64; 2], path: &BezPath) -> Option<BorderDashPattern> {
    let painted = lengths[0];
    let base_gap = lengths[1];
    if !painted.is_finite() || painted < 0.0 || !base_gap.is_finite() || base_gap <= 0.0 {
        return None;
    }

    let contour_length = path_length(path);
    let base_period = painted + base_gap;
    if !contour_length.is_finite() || contour_length <= 0.0 || base_period <= 0.0 {
        return None;
    }

    let count = nearest_period_count(contour_length, base_period, painted);
    let adjusted_period = contour_length / count;
    if !adjusted_period.is_finite() || adjusted_period <= painted {
        return None;
    }

    let gap = adjusted_period - painted;
    Some(BorderDashPattern::new(painted + gap * 0.5, [painted, gap]))
}

fn nearest_period_count(total: f64, base_period: f64, painted: f64) -> f64 {
    let estimate = total / base_period;
    let lower = floor(estimate).max(1.0);
    let upper = lower + 1.0;

    let lower_error = (total / lower - base_period).abs();
    let upper_error = (total / upper - base_period).abs();
    let mut count = if upper_error < lower_error {
        upper
    } else {
        lower
    };

    while count > 1.0 && total / count <= painted {
        count -= 1.0;
    }
    count
}

fn path_length(path: &BezPath) -> f64 {
    const ACCURACY: f64 = 0.1;

    path.segments()
        .map(|segment| segment.arclen(ACCURACY))
        .sum()
}

fn side_priority(side: Side) -> u8 {
    match side {
        Side::Top => 0,
        Side::Bottom => 1,
        Side::Right => 2,
        Side::Left => 3,
    }
}

fn uniform_visible_style(geometry: BoxDecorationGeometry) -> Option<BorderStyle> {
    let first = geometry.border_paint().side(Side::Top);
    if first.is_empty() {
        return None;
    }
    let style = first.style();
    for side in [Side::Right, Side::Bottom, Side::Left] {
        let side = geometry.border_paint().side(side);
        if side.is_empty() || side.style() != style {
            return None;
        }
    }
    Some(style)
}

fn shared_visible_width(geometry: BoxDecorationGeometry) -> Option<f64> {
    let first = geometry.border_paint().side(Side::Top);
    if first.is_empty() {
        return None;
    }
    let width = first.width();
    for side in [Side::Right, Side::Bottom, Side::Left] {
        let side = geometry.border_paint().side(side);
        if side.is_empty() || !same_f64(side.width(), width) {
            return None;
        }
    }
    Some(width)
}

fn style_for_side(geometry: BoxDecorationGeometry, side: Side) -> BorderStyle {
    match side {
        Side::Top => geometry.border_styles.top,
        Side::Right => geometry.border_styles.right,
        Side::Bottom => geometry.border_styles.bottom,
        Side::Left => geometry.border_styles.left,
    }
}

fn inset_outset_shade(side: Side, style: BorderStyle) -> BorderReliefShade {
    let before_edge = matches!(side, Side::Top | Side::Left);
    let highlight = match style {
        BorderStyle::Outset => before_edge,
        BorderStyle::Inset => !before_edge,
        _ => false,
    };
    relief_shade(highlight)
}

fn groove_ridge_shades(side: Side, style: BorderStyle) -> (BorderReliefShade, BorderReliefShade) {
    let before_edge = matches!(side, Side::Top | Side::Left);
    let outer_highlight = match style {
        BorderStyle::Ridge => before_edge,
        BorderStyle::Groove => !before_edge,
        _ => false,
    };
    (
        relief_shade(outer_highlight),
        relief_shade(!outer_highlight),
    )
}

fn relief_shade(highlight: bool) -> BorderReliefShade {
    if highlight {
        BorderReliefShade::Highlight
    } else {
        BorderReliefShade::Shadow
    }
}

fn same_f64(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits()
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
    use kurbo::{PathEl, Point, Rect, Size};

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
        assert!(ring.path.is_finite());
        assert!(band.path.is_finite());
    }

    #[test]
    fn side_fill_fragments_are_closed_side_owned_paths() {
        let side = rounded_geometry().border_paint().side(Side::Right);

        let ring = side.ring();
        let band = side.band(BorderBand::new(2.0 / 3.0, 1.0));

        assert_eq!(ring.fill_rule, BorderFillRule::NonZero);
        assert_eq!(band.fill_rule, BorderFillRule::NonZero);
        assert_eq!(close_count(&ring.path), 1);
        assert_eq!(close_count(&band.path), 1);
        assert_eq!(ring.clips, BorderClipStack::empty());
        assert_eq!(band.clips, BorderClipStack::empty());
        assert!(ring.path.is_finite());
        assert!(band.path.is_finite());
    }

    #[test]
    fn reference_sheet_side_fill_uses_corner_miter_boundary() {
        let bottom = reference_sheet_geometry(Edges::all(BorderStyle::Solid))
            .border_paint()
            .side(Side::Bottom)
            .ring();

        let outer_corner = Point::new(0.0, 66.0);
        let inner_corner = Point::new(8.0, 46.0);
        let outer_join = previous_point_before_line_to(&bottom.path, inner_corner)
            .expect("bottom side should join to the inner bottom-left corner");

        assert!(
            outer_join.x < inner_corner.x,
            "outer join should lie before the inner corner on the bottom-left miter: {outer_join:?}"
        );
        assert!(
            signed_line_distance(outer_corner, inner_corner, outer_join).abs() <= 1e-6,
            "outer join should lie on the bottom-left miter line: {outer_join:?}"
        );
    }

    #[test]
    fn reference_sheet_side_stroke_uses_corner_miter_boundary() {
        let bottom = reference_sheet_geometry(Edges::all(BorderStyle::Dashed))
            .border_paint()
            .side(Side::Bottom)
            .stroke_contour(0.5);

        let outer_corner = Point::new(0.0, 66.0);
        let inner_corner = Point::new(8.0, 46.0);
        let join = last_path_point(&bottom.path).expect("bottom stroke should have an endpoint");

        assert!(
            join.x < inner_corner.x,
            "stroke join should lie before the inner corner on the bottom-left miter: {join:?}"
        );
        assert!(
            signed_line_distance(outer_corner, inner_corner, join).abs() <= 1e-6,
            "stroke join should lie on the bottom-left miter line: {join:?}"
        );
    }

    #[test]
    fn side_stroke_fragment_is_open_and_clipped_to_ring() {
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
        assert!(stroke.clips.side.is_none());
        assert_eq!(
            stroke.clips.ring.as_ref().map(|clip| clip.fill_rule),
            Some(BorderFillRule::EvenOdd)
        );
        assert_eq!(
            stroke
                .clips
                .ring
                .as_ref()
                .map(|clip| close_count(&clip.path)),
            Some(2)
        );
        assert!(stroke.path.is_finite());
    }

    #[test]
    fn shared_double_plan_emits_two_whole_band_fills() {
        let geometry = BoxDecorationGeometry::from_styled_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::new(6.0, 12.0, 18.0, 24.0),
            Edges::all(BorderStyle::Double),
            Edges::ZERO,
            CornerRadii::uniform(20.0),
            CornerShapes::ROUND,
        );

        let mut fill_count = 0;
        geometry
            .border_paint()
            .shared_plan()
            .for_each(|command| match command {
                BorderPaintCommand::Fill(fill) => {
                    fill_count += 1;
                    assert_eq!(fill.source, BorderPaintSource::Shared);
                    assert_eq!(fill.style, BorderStyle::Double);
                    assert_eq!(fill.role, BorderPaintRole::Base);
                    assert_eq!(fill.fragment.fill_rule, BorderFillRule::EvenOdd);
                    assert_eq!(close_count(&fill.fragment.path), 2);
                    assert_eq!(fill.fragment.clips, BorderClipStack::empty());
                }
                BorderPaintCommand::Stroke(_) => panic!("double plan should not emit strokes"),
            });

        assert_eq!(fill_count, 2);
    }

    #[test]
    fn shared_solid_plan_emits_one_ring_fill() {
        let geometry = BoxDecorationGeometry::from_styled_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::all(8.0),
            Edges::all(BorderStyle::Solid),
            Edges::ZERO,
            CornerRadii::uniform(20.0),
            CornerShapes::all(CornerShape::Bevel),
        );

        let mut fill_count = 0;
        geometry
            .border_paint()
            .shared_plan()
            .for_each(|command| match command {
                BorderPaintCommand::Fill(fill) => {
                    fill_count += 1;
                    assert_eq!(fill.source, BorderPaintSource::Shared);
                    assert_eq!(fill.style, BorderStyle::Solid);
                    assert_eq!(fill.role, BorderPaintRole::Base);
                    assert_eq!(fill.fragment.fill_rule, BorderFillRule::EvenOdd);
                    assert_eq!(close_count(&fill.fragment.path), 2);
                    assert_eq!(fill.fragment.clips, BorderClipStack::empty());
                    assert!(fill.fragment.path.is_finite());
                }
                BorderPaintCommand::Stroke(_) => panic!("shared solid plan should not stroke"),
            });

        assert_eq!(fill_count, 1);
    }

    #[test]
    fn shared_solid_plan_keeps_one_ring_fill_for_mixed_widths() {
        let geometry = BoxDecorationGeometry::from_styled_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::new(6.0, 12.0, 18.0, 24.0),
            Edges::all(BorderStyle::Solid),
            Edges::ZERO,
            CornerRadii::uniform(20.0),
            CornerShapes::ROUND,
        );

        let mut fill_count = 0;
        geometry
            .border_paint()
            .shared_plan()
            .for_each(|command| match command {
                BorderPaintCommand::Fill(fill) => {
                    fill_count += 1;
                    assert_eq!(fill.source, BorderPaintSource::Shared);
                    assert_eq!(fill.role, BorderPaintRole::Base);
                    assert_eq!(close_count(&fill.fragment.path), 2);
                }
                BorderPaintCommand::Stroke(_) => panic!("shared solid plan should not stroke"),
            });

        assert_eq!(fill_count, 1);
    }

    #[test]
    fn non_visible_borders_emit_no_paint_commands() {
        let geometry = BoxDecorationGeometry::from_styled_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::new(8.0, 0.0, f64::NAN, 12.0),
            Edges::new(
                BorderStyle::Hidden,
                BorderStyle::Solid,
                BorderStyle::Dashed,
                BorderStyle::None,
            ),
            Edges::ZERO,
            CornerRadii::uniform(20.0),
            CornerShapes::ROUND,
        );

        let mut shared_count = 0;
        geometry
            .border_paint()
            .shared_plan()
            .for_each(|_| shared_count += 1);

        let mut per_side_count = 0;
        geometry
            .border_paint()
            .per_side_plan()
            .for_each(|_| per_side_count += 1);

        assert!(!geometry.has_visible_border());
        assert_eq!(shared_count, 0);
        assert_eq!(per_side_count, 0);
    }

    #[test]
    fn shared_rounded_dashed_plan_balances_one_closed_contour_stroke() {
        let geometry = BoxDecorationGeometry::from_styled_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::all(8.0),
            Edges::all(BorderStyle::Dashed),
            Edges::ZERO,
            CornerRadii::uniform(20.0),
            CornerShapes::ROUND,
        );

        let mut stroke_count = 0;
        geometry
            .border_paint()
            .shared_plan()
            .for_each(|command| match command {
                BorderPaintCommand::Fill(_) => panic!("dashed plan should not emit fills"),
                BorderPaintCommand::Stroke(stroke) => {
                    stroke_count += 1;
                    assert_eq!(stroke.source, BorderPaintSource::Shared);
                    assert_eq!(stroke.style, BorderStyle::Dashed);
                    assert_eq!(stroke.stroke.width, 8.0);
                    assert_eq!(stroke.stroke.cap, BorderStrokeCap::Butt);
                    assert_balanced_closed_dash(&stroke.fragment.path, stroke.stroke.dash, 16.0);
                    assert_eq!(close_count(&stroke.fragment.path), 1);
                    assert!(stroke.fragment.clips.side.is_none());
                    assert_eq!(
                        stroke
                            .fragment
                            .clips
                            .ring
                            .as_ref()
                            .map(|clip| clip.fill_rule),
                        Some(BorderFillRule::EvenOdd)
                    );
                    assert_eq!(
                        stroke
                            .fragment
                            .clips
                            .ring
                            .as_ref()
                            .map(|clip| close_count(&clip.path)),
                        Some(2)
                    );
                    assert!(stroke.fragment.path.is_finite());
                }
            });

        assert_eq!(stroke_count, 1);
    }

    #[test]
    fn shared_rounded_dotted_plan_balances_drawable_round_dashes() {
        let geometry = BoxDecorationGeometry::from_styled_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::all(8.0),
            Edges::all(BorderStyle::Dotted),
            Edges::ZERO,
            CornerRadii::uniform(20.0),
            CornerShapes::ROUND,
        );

        let mut stroke_count = 0;
        geometry
            .border_paint()
            .shared_plan()
            .for_each(|command| match command {
                BorderPaintCommand::Fill(_) => panic!("dotted plan should not emit fills"),
                BorderPaintCommand::Stroke(stroke) => {
                    stroke_count += 1;
                    assert_eq!(stroke.source, BorderPaintSource::Shared);
                    assert_eq!(stroke.style, BorderStyle::Dotted);
                    assert_eq!(stroke.stroke.width, 8.0);
                    assert_eq!(stroke.stroke.cap, BorderStrokeCap::Round);
                    assert_balanced_closed_dash(&stroke.fragment.path, stroke.stroke.dash, 0.01);
                    assert_eq!(close_count(&stroke.fragment.path), 1);
                    assert!(stroke.fragment.clips.side.is_none());
                    assert_eq!(
                        stroke
                            .fragment
                            .clips
                            .ring
                            .as_ref()
                            .map(|clip| clip.fill_rule),
                        Some(BorderFillRule::EvenOdd)
                    );
                    assert_eq!(
                        stroke
                            .fragment
                            .clips
                            .ring
                            .as_ref()
                            .map(|clip| close_count(&clip.path)),
                        Some(2)
                    );
                }
            });

        assert_eq!(stroke_count, 1);
    }

    #[test]
    fn thin_dashed_plan_uses_longer_dash_ratio() {
        let geometry = BoxDecorationGeometry::from_styled_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::all(2.0),
            Edges::all(BorderStyle::Dashed),
            Edges::ZERO,
            CornerRadii::uniform(20.0),
            CornerShapes::ROUND,
        );

        let mut stroke_count = 0;
        geometry
            .border_paint()
            .shared_plan()
            .for_each(|command| match command {
                BorderPaintCommand::Fill(_) => panic!("dashed plan should not emit fills"),
                BorderPaintCommand::Stroke(stroke) => {
                    stroke_count += 1;
                    assert_eq!(stroke.style, BorderStyle::Dashed);
                    assert_eq!(stroke.stroke.width, 2.0);
                    assert_eq!(stroke.stroke.cap, BorderStrokeCap::Butt);
                    assert_balanced_closed_dash(&stroke.fragment.path, stroke.stroke.dash, 6.0);
                }
            });

        assert_eq!(stroke_count, 1);
    }

    #[test]
    fn per_side_dashed_plan_emits_owned_strokes() {
        let geometry = BoxDecorationGeometry::from_styled_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::all(8.0),
            Edges::all(BorderStyle::Dashed),
            Edges::ZERO,
            CornerRadii::uniform(20.0),
            CornerShapes::ROUND,
        );

        let mut stroke_count = 0;
        geometry
            .border_paint()
            .per_side_plan()
            .for_each(|command| match command {
                BorderPaintCommand::Fill(_) => panic!("dashed plan should not emit fills"),
                BorderPaintCommand::Stroke(stroke) => {
                    stroke_count += 1;
                    assert!(matches!(stroke.source, BorderPaintSource::Side(_)));
                    assert_eq!(stroke.style, BorderStyle::Dashed);
                    assert_eq!(stroke.stroke.width, 8.0);
                    assert_eq!(stroke.stroke.cap, BorderStrokeCap::Butt);
                    assert_balanced_open_dash(&stroke.fragment.path, stroke.stroke.dash, 16.0);
                    assert_eq!(close_count(&stroke.fragment.path), 0);
                    assert_eq!(
                        stroke
                            .fragment
                            .clips
                            .side
                            .as_ref()
                            .map(|clip| clip.fill_rule),
                        Some(BorderFillRule::NonZero)
                    );
                    assert_eq!(
                        stroke
                            .fragment
                            .clips
                            .side
                            .as_ref()
                            .map(|clip| close_count(&clip.path)),
                        Some(1)
                    );
                    assert_eq!(
                        stroke
                            .fragment
                            .clips
                            .ring
                            .as_ref()
                            .map(|clip| clip.fill_rule),
                        Some(BorderFillRule::EvenOdd)
                    );
                    assert_eq!(
                        stroke
                            .fragment
                            .clips
                            .ring
                            .as_ref()
                            .map(|clip| close_count(&clip.path)),
                        Some(2)
                    );
                    assert!(stroke.fragment.path.is_finite());
                }
            });

        assert_eq!(stroke_count, 4);
    }

    #[test]
    fn per_side_dotted_plan_anchors_dots_at_corner_boundaries() {
        let geometry = BoxDecorationGeometry::from_styled_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::all(8.0),
            Edges::all(BorderStyle::Dotted),
            Edges::ZERO,
            CornerRadii::uniform(20.0),
            CornerShapes::ROUND,
        );

        let mut stroke_count = 0;
        geometry
            .border_paint()
            .per_side_plan()
            .for_each(|command| match command {
                BorderPaintCommand::Fill(_) => panic!("dotted plan should not emit fills"),
                BorderPaintCommand::Stroke(stroke) => {
                    stroke_count += 1;
                    assert_eq!(stroke.style, BorderStyle::Dotted);
                    assert_eq!(stroke.stroke.cap, BorderStrokeCap::Round);
                    assert_balanced_open_dash(&stroke.fragment.path, stroke.stroke.dash, 0.01);
                }
            });

        assert_eq!(stroke_count, 4);
    }

    #[test]
    fn per_side_double_plan_emits_side_owned_band_fills_without_clips() {
        let geometry = BoxDecorationGeometry::from_styled_border_box(
            Rect::new(0.0, 0.0, 154.0, 68.0),
            Edges::new(7.0, 11.0, 8.0, 5.0),
            Edges::all(BorderStyle::Double),
            Edges::ZERO,
            CornerRadii::new(
                Size::new(24.0, 18.0),
                Size::new(28.0, 18.0),
                Size::new(20.0, 26.0),
                Size::new(18.0, 14.0),
            ),
            CornerShapes::new(
                CornerShape::Superellipse(Superellipse::new(2.0)),
                CornerShape::Bevel,
                CornerShape::scoop(),
                CornerShape::Round,
            ),
        );

        let mut fill_count = 0;
        geometry
            .border_paint()
            .per_side_plan()
            .for_each(|command| match command {
                BorderPaintCommand::Fill(fill) => {
                    fill_count += 1;
                    assert!(matches!(fill.source, BorderPaintSource::Side(_)));
                    assert_eq!(fill.style, BorderStyle::Double);
                    assert_eq!(fill.role, BorderPaintRole::Base);
                    assert_eq!(fill.fragment.fill_rule, BorderFillRule::NonZero);
                    assert_eq!(close_count(&fill.fragment.path), 1);
                    assert_eq!(fill.fragment.clips, BorderClipStack::empty());
                    assert!(fill.fragment.path.iter().count() > 4);
                    assert!(fill.fragment.path.is_finite());
                }
                BorderPaintCommand::Stroke(_) => {
                    panic!("per-side double plan should not emit strokes")
                }
            });

        assert_eq!(fill_count, 8);
    }

    #[test]
    fn inset_plan_emits_one_relief_fill_per_side() {
        let geometry = BoxDecorationGeometry::from_styled_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::all(8.0),
            Edges::all(BorderStyle::Inset),
            Edges::ZERO,
            CornerRadii::uniform(12.0),
            CornerShapes::ROUND,
        );

        let mut fill_count = 0;
        geometry
            .border_paint()
            .shared_plan()
            .for_each(|command| match command {
                BorderPaintCommand::Fill(fill) => {
                    fill_count += 1;
                    assert_eq!(fill.style, BorderStyle::Inset);
                    let BorderPaintRole::Relief(shade) = fill.role else {
                        panic!("inset plan should only emit relief fills: {:?}", fill.role);
                    };
                    let BorderPaintSource::Side(side) = fill.source else {
                        panic!("relief styles should paint per side: {:?}", fill.source);
                    };
                    // Inset looks pressed: before edges are dark, after edges
                    // are light.
                    let expected = match side {
                        Side::Top | Side::Left => BorderReliefShade::Shadow,
                        Side::Right | Side::Bottom => BorderReliefShade::Highlight,
                    };
                    assert_eq!(shade, expected, "wrong inset shade for {side:?}");
                    assert_eq!(close_count(&fill.fragment.path), 1);
                }
                BorderPaintCommand::Stroke(_) => panic!("inset plan should not stroke"),
            });

        assert_eq!(fill_count, 4);
    }

    #[test]
    fn groove_plan_emits_relief_band_pairs_per_side() {
        let geometry = BoxDecorationGeometry::from_styled_border_box(
            Rect::new(0.0, 0.0, 120.0, 80.0),
            Edges::all(8.0),
            Edges::all(BorderStyle::Groove),
            Edges::ZERO,
            CornerRadii::uniform(12.0),
            CornerShapes::ROUND,
        );

        let mut top_shades = [None, None];
        let mut top_index = 0;
        let mut fill_count = 0;
        geometry
            .border_paint()
            .per_side_plan()
            .for_each(|command| match command {
                BorderPaintCommand::Fill(fill) => {
                    fill_count += 1;
                    assert_eq!(fill.style, BorderStyle::Groove);
                    let BorderPaintRole::Relief(shade) = fill.role else {
                        panic!("groove plan should only emit relief fills: {:?}", fill.role);
                    };
                    if fill.source == BorderPaintSource::Side(Side::Top) {
                        top_shades[top_index] = Some(shade);
                        top_index += 1;
                    }
                }
                BorderPaintCommand::Stroke(_) => panic!("groove plan should not stroke"),
            });

        assert_eq!(fill_count, 8);
        // Groove carves inward: the outer half of a before-edge side is dark
        // and the inner half is light.
        assert_eq!(
            top_shades,
            [
                Some(BorderReliefShade::Shadow),
                Some(BorderReliefShade::Highlight)
            ]
        );
    }

    #[test]
    fn side_stroke_fragments_preserve_shaped_corner_segments() {
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
            assert!(stroke.path.is_finite());
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

    fn reference_sheet_geometry(border_styles: Edges<BorderStyle>) -> BoxDecorationGeometry {
        BoxDecorationGeometry::from_styled_border_box(
            Rect::new(0.0, 0.0, 184.0, 66.0),
            Edges::new(4.0, 12.0, 20.0, 8.0),
            border_styles,
            Edges::ZERO,
            CornerRadii::new(
                Size::new(10.0, 30.0),
                Size::new(40.0, 10.0),
                Size::new(30.0, 50.0),
                Size::new(60.0, 20.0),
            ),
            CornerShapes::ROUND,
        )
    }

    fn previous_point_before_line_to(path: &BezPath, target: Point) -> Option<Point> {
        let mut current = None;
        for element in path.iter() {
            match element {
                PathEl::MoveTo(point) => current = Some(point),
                PathEl::LineTo(point) => {
                    if same_point(point, target) {
                        return current;
                    }
                    current = Some(point);
                }
                PathEl::QuadTo(_, point) | PathEl::CurveTo(_, _, point) => current = Some(point),
                PathEl::ClosePath => {}
            }
        }
        None
    }

    fn last_path_point(path: &BezPath) -> Option<Point> {
        let mut current = None;
        for element in path.iter() {
            match element {
                PathEl::MoveTo(point)
                | PathEl::LineTo(point)
                | PathEl::QuadTo(_, point)
                | PathEl::CurveTo(_, _, point) => current = Some(point),
                PathEl::ClosePath => {}
            }
        }
        current
    }

    fn same_point(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() <= 1e-6 && (a.y - b.y).abs() <= 1e-6
    }

    fn signed_line_distance(start: Point, end: Point, point: Point) -> f64 {
        let line_x = end.x - start.x;
        let line_y = end.y - start.y;
        let point_x = point.x - start.x;
        let point_y = point.y - start.y;
        line_x * point_y - line_y * point_x
    }

    fn assert_balanced_open_dash(path: &BezPath, dash: Option<BorderDashPattern>, painted: f64) {
        let dash = dash.expect("stroke should carry a dash pattern");
        let gap = dash.lengths[1];
        let length = path_length(path);
        let gaps = (length - painted) / (painted + gap);

        assert_eq!(dash.lengths[0], painted);
        assert_eq!(dash.offset, 0.0, "open dashes should start on a dash");
        assert!(
            gap > 0.0,
            "balanced dash gap should stay positive: {dash:?}"
        );
        assert!(
            (gaps - floor(gaps + 0.5)).abs() <= 1e-9,
            "open contour should end on a full dash: length={length}, gaps={gaps}, dash={dash:?}"
        );
    }

    fn assert_balanced_closed_dash(path: &BezPath, dash: Option<BorderDashPattern>, painted: f64) {
        let dash = dash.expect("stroke should carry a dash pattern");
        let period = dash.lengths[0] + dash.lengths[1];
        let contour_length = path_length(path);
        let periods = contour_length / period;
        let lower = floor(periods).max(1.0);
        let upper = lower + 1.0;
        let fit_error = (periods - lower).abs().min((periods - upper).abs());

        assert_eq!(dash.lengths[0], painted);
        assert!(
            dash.lengths[1] > 0.0,
            "balanced dash gap should stay positive: {dash:?}"
        );
        assert!(
            dash.offset > painted && dash.offset < period,
            "balanced dash should start inside a gap: {dash:?}"
        );
        assert!(
            fit_error <= 1e-9,
            "balanced dash period should divide the closed contour length: length={contour_length}, periods={periods}, dash={dash:?}"
        );
    }

    fn close_count(path: &BezPath) -> usize {
        path.iter()
            .filter(|element| matches!(element, PathEl::ClosePath))
            .count()
    }
}
