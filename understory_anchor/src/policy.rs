// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{Insets, Rect, Size};

use crate::{Anchor, Placement, PreviousAnchorFrame};

/// Current scene facts passed to [`resolve_anchor`](crate::resolve_anchor).
///
/// Construct one input per layout/update frame. `previous` should be derived
/// from the resolver's last [`AnchorFrame`](crate::AnchorFrame) for the same
/// logical overlay when available; this keeps placement stable without making
/// the resolver stateful.
///
/// Anchor geometry, `floating_size`, `viewport`, and `boundary` must be
/// finite. `floating_size` must also be non-negative. Debug builds assert
/// these contracts at resolver entry points; release builds rely on callers to
/// uphold them.
#[derive(Clone, Copy, Debug)]
pub struct AnchorInput<'a> {
    /// The anchor to resolve against.
    pub anchor: Anchor<'a>,
    /// Desired floating surface size before constraints are applied.
    ///
    /// Must be finite and non-negative.
    pub floating_size: Size,
    /// The visible viewport in scene coordinates.
    pub viewport: Rect,
    /// The collision boundary in scene coordinates.
    pub boundary: Rect,
    /// Lightweight previous resolved frame, if any, for placement hysteresis.
    ///
    /// Get this from [`PreviousAnchorFrame::from`] after resolving the previous
    /// frame for the same logical overlay.
    pub previous: Option<&'a PreviousAnchorFrame>,
}

/// Placement option, collision, scoring, and stability policy.
///
/// Callers pass an `AnchorPolicy` to [`resolve_anchor`](crate::resolve_anchor)
/// alongside [`AnchorInput`]. The policy says what the caller prefers and how
/// collision should be handled; it should not duplicate scene facts from the
/// input.
///
/// Numeric policy values are expected to be finite. Insets, sizes, padding, and
/// hysteresis thresholds are expected to be non-negative where that is stated
/// on the field. Debug builds assert these contracts at resolver entry points.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct AnchorPolicy<'a> {
    /// Preferred position option, evaluated before fallbacks unless reordered.
    pub preferred: AnchorPositionOption<'a>,
    /// Fallback position options evaluated after the preferred option.
    pub fallbacks: &'a [AnchorPositionOption<'a>],
    /// Optional ordering pass applied before candidate generation.
    pub order: PositionTryOrder,
    /// Weights used by the default candidate scorer.
    pub scoring: ScoringPolicy,
    /// Previous-frame stability policy.
    pub hysteresis: HysteresisPolicy,
}

impl<'a> AnchorPolicy<'a> {
    /// Creates a policy from a preferred option and fallback options.
    #[must_use]
    pub fn new(
        preferred: AnchorPositionOption<'a>,
        fallbacks: &'a [AnchorPositionOption<'a>],
    ) -> Self {
        Self {
            preferred,
            fallbacks,
            order: PositionTryOrder::Normal,
            scoring: ScoringPolicy::default(),
            hysteresis: HysteresisPolicy::default(),
        }
    }

    /// Creates a policy with one unconstrained preferred placement.
    #[must_use]
    pub fn placement(preferred: Placement) -> Self {
        Self::new(AnchorPositionOption::new(preferred), &[])
    }

    /// Returns this policy with a different option ordering pass.
    #[must_use]
    pub const fn with_order(mut self, order: PositionTryOrder) -> Self {
        self.order = order;
        self
    }
}

/// A resolved-position option considered by [`AnchorPolicy`].
///
/// This is the unit that CSS-like adapters should generate from base styles,
/// fallback styles, or `@position-try`-style rules. Each option can carry its
/// own placement, constraints, and host-defined key, so fallback options can
/// vary offsets, size behavior, arrows, and detachment behavior instead of only
/// changing sides.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct AnchorPositionOption<'a> {
    /// Placement requested by this option.
    pub placement: Placement,
    /// Constraints applied only to this option.
    pub constraints: &'a [AnchorConstraint],
    /// Stable host-defined identity for previous-frame matching.
    pub key: AnchorOptionKey,
}

impl<'a> AnchorPositionOption<'a> {
    /// Creates an unconstrained option for a placement.
    #[must_use]
    pub const fn new(placement: Placement) -> Self {
        Self {
            placement,
            constraints: &[],
            key: AnchorOptionKey::Auto,
        }
    }

    /// Returns this option with per-option constraints.
    #[must_use]
    pub const fn with_constraints(mut self, constraints: &'a [AnchorConstraint]) -> Self {
        self.constraints = constraints;
        self
    }

    /// Returns this option with a stable host-defined key.
    ///
    /// Use an explicit key when the option list can be reordered or rebuilt
    /// across frames and previous-frame hysteresis should follow logical option
    /// identity instead of the option's original index.
    #[must_use]
    pub const fn with_key(mut self, key: AnchorOptionKey) -> Self {
        self.key = key;
        self
    }
}

/// Stable identity for an [`AnchorPositionOption`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AnchorOptionKey {
    /// Match previous-frame state by the option's original index.
    #[default]
    Auto,
    /// Match previous-frame state by a host-defined stable identifier.
    Id(u64),
}

impl AnchorOptionKey {
    /// Creates a host-defined option key.
    #[must_use]
    pub const fn id(id: u64) -> Self {
        Self::Id(id)
    }
}

/// Candidate ordering pass applied before scoring.
///
/// This is intentionally smaller than CSS `position-try-order`: it operates on
/// already-resolved physical geometry. CSS adapters can map logical
/// inline/block variants to width or height after they resolve writing mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PositionTryOrder {
    /// Preserve preferred-then-fallback order.
    #[default]
    Normal,
    /// Try options with more available physical width first.
    MostWidth,
    /// Try options with more available physical height first.
    MostHeight,
}

/// A per-candidate constraint supplied through [`AnchorPositionOption::constraints`].
///
/// Constraints are applied to each generated placement candidate before
/// scoring. Use them to express offsets, collision shifting, size bounds,
/// arrows, and detachment behavior without changing the scene facts.
///
/// Duplicate constraints are deterministic:
///
/// - `Offset` values are summed.
/// - `Size` constraints are applied in slice order.
/// - `Shift` and `KeepInBounds` both translate without resizing, in slice
///   order. Use `Shift` for the normal collision-response pass that keeps a
///   candidate visible when possible. Use `KeepInBounds` as a final invariant
///   clamp when an embedding layer requires the floating rectangle to remain
///   inside padded bounds after earlier constraints.
/// - The first `Arrow` constraint emits arrow geometry; later `Arrow`
///   constraints are ignored.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AnchorConstraint {
    /// Translate the candidate relative to the anchor.
    Offset {
        /// Distance away from the anchor along the placement side normal.
        ///
        /// Must be finite.
        main_axis: f64,
        /// Lateral adjustment along the placement side's cross axis.
        ///
        /// Must be finite.
        cross_axis: f64,
    },
    /// Shift the candidate into the boundary without resizing it.
    ///
    /// Use this as the ordinary collision-response constraint. It preserves
    /// the requested floating size and records the resulting movement in
    /// [`CandidateMetrics::shifted_distance`](crate::CandidateMetrics::shifted_distance).
    Shift {
        /// Minimum inset from the collision boundary.
        ///
        /// Must be finite and non-negative on every side.
        padding: Insets,
    },
    /// Clamp or shrink the candidate's floating size.
    Size {
        /// Minimum acceptable size after constraint application.
        ///
        /// Must be finite and non-negative.
        min: Size,
        /// Optional maximum size before collision shrink is considered.
        ///
        /// Must be finite and non-negative when present.
        max: Option<Size>,
        /// Whether the candidate may shrink to fit available boundary space.
        allow_shrink: bool,
    },
    /// Emit arrow geometry for the candidate.
    Arrow {
        /// Arrow bounding-box size.
        ///
        /// Must be finite and non-negative.
        size: Size,
        /// Minimum cross-axis distance from floating surface corners.
        ///
        /// Must be finite and non-negative.
        padding: f64,
    },
    /// Mark the candidate detached and invisible when its anchor is outside the boundary.
    HideWhenDetached,
    /// Clamp the candidate into the boundary after other constraints.
    ///
    /// Use this as a final invariant clamp when a caller needs to guarantee a
    /// padded boundary even if other constraints or offsets moved the
    /// candidate. It translates only; size changes should be expressed with
    /// [`AnchorConstraint::Size`].
    KeepInBounds {
        /// Minimum inset from the collision boundary.
        ///
        /// Must be finite and non-negative on every side.
        padding: Insets,
    },
}

impl AnchorConstraint {
    pub(crate) fn is_finite(&self) -> bool {
        match *self {
            Self::Offset {
                main_axis,
                cross_axis,
            } => main_axis.is_finite() && cross_axis.is_finite(),
            Self::Shift { padding } | Self::KeepInBounds { padding } => padding.is_finite(),
            Self::Size {
                min,
                max,
                allow_shrink: _,
            } => min.is_finite() && max.is_none_or(|max| max.is_finite()),
            Self::Arrow { size, padding } => size.is_finite() && padding.is_finite(),
            Self::HideWhenDetached => true,
        }
    }
}

/// Weights used by the built-in candidate scorer.
///
/// Callers usually keep the default value in [`AnchorPolicy::scoring`]. Tune it
/// when an embedding layer wants a different tradeoff between visible area,
/// overflow, movement, size change, and placement preference.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScoringPolicy {
    /// Weight applied to visible area.
    ///
    /// Must be finite.
    pub visible_area_weight: f64,
    /// Penalty weight applied to overflow area.
    ///
    /// Must be finite.
    pub overflow_area_weight: f64,
    /// Penalty weight applied to anchor distance.
    ///
    /// Must be finite.
    pub anchor_distance_weight: f64,
    /// Penalty weight applied to collision shift distance.
    ///
    /// Must be finite.
    pub shifted_distance_weight: f64,
    /// Penalty weight applied to width plus height size delta.
    ///
    /// Must be finite.
    pub size_delta_weight: f64,
    /// Bonus for the preferred placement.
    ///
    /// Must be finite.
    pub preferred_bonus: f64,
    /// Bonus for the incumbent placement in raw diagnostic scoring.
    ///
    /// Must be finite.
    pub incumbent_bonus: f64,
}

impl Default for ScoringPolicy {
    fn default() -> Self {
        Self {
            visible_area_weight: 1.0,
            overflow_area_weight: 8.0,
            anchor_distance_weight: 0.5,
            shifted_distance_weight: 0.5,
            size_delta_weight: 1.0,
            preferred_bonus: 1000.0,
            incumbent_bonus: 250.0,
        }
    }
}

impl ScoringPolicy {
    pub(crate) fn is_finite(&self) -> bool {
        self.visible_area_weight.is_finite()
            && self.overflow_area_weight.is_finite()
            && self.anchor_distance_weight.is_finite()
            && self.shifted_distance_weight.is_finite()
            && self.size_delta_weight.is_finite()
            && self.preferred_bonus.is_finite()
            && self.incumbent_bonus.is_finite()
    }

    /// Creates a collision-first preset for anchored UI.
    ///
    /// Use this when placement should primarily follow visible area, overflow,
    /// collision shift, and size fit rather than receiving a strong preferred
    /// placement bonus. This is useful for adapters that need traditional
    /// "flip when collision is better" behavior. Hysteresis still comes from
    /// [`AnchorPolicy::hysteresis`].
    #[must_use]
    pub fn collision_first() -> Self {
        Self {
            preferred_bonus: 0.0,
            incumbent_bonus: 0.0,
            ..Self::default()
        }
    }
}

/// Previous-frame stability controls for candidate choice.
///
/// Supply this through [`AnchorPolicy::hysteresis`]. It is only effective when
/// [`AnchorInput::previous`] contains the last frame for the same logical
/// overlay.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HysteresisPolicy {
    /// Whether hysteresis is enabled.
    pub enabled: bool,
    /// Selection-time bonus applied to the previous placement when viable.
    ///
    /// Must be finite.
    pub incumbent_bonus: f64,
    /// Required score advantage before switching away from the incumbent.
    ///
    /// Must be finite and non-negative.
    pub switch_threshold: f64,
}

impl Default for HysteresisPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            incumbent_bonus: 250.0,
            switch_threshold: 50.0,
        }
    }
}

impl HysteresisPolicy {
    pub(crate) const fn disabled() -> Self {
        Self {
            enabled: false,
            incumbent_bonus: 0.0,
            switch_threshold: 0.0,
        }
    }

    pub(crate) fn is_finite(&self) -> bool {
        self.incumbent_bonus.is_finite() && self.switch_threshold.is_finite()
    }
}
