// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use kurbo::{Insets, Point, Rect, Size};

use crate::{AnchorOptionKey, Placement, Side};

/// Candidate measurements exposed for diagnostics and custom choice logic.
///
/// You get these from [`AnchorCandidate::metrics`] in the [`CollisionReport`]
/// returned through [`AnchorFrame::collision`]. They are measurements, not a
/// stable public scoring unit.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CandidateMetrics {
    /// Area of the candidate visible within the viewport and boundary.
    pub visible_area: f64,
    /// Positive overflow on left, top, right, and bottom edges.
    pub overflow: Insets,
    /// Candidate area that lies outside the viewport and boundary.
    pub overflow_area: f64,
    /// Distance from the anchor reference center to the candidate rectangle.
    pub anchor_distance: f64,
    /// Distance the candidate was shifted by collision constraints.
    pub shifted_distance: f64,
    /// Absolute difference between desired and resolved floating size.
    pub size_delta: Size,
    /// Whether this candidate is the preferred placement.
    pub is_preferred: bool,
    /// Whether this candidate matches the previous frame placement.
    pub is_incumbent: bool,
}

/// Diagnostic scores produced by the built-in scorer.
///
/// You get this from [`AnchorCandidate::diagnostics`]. The score is useful for
/// debugging and deterministic comparisons, but callers should not treat its
/// numeric value as a stable semantic contract.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CandidateDiagnostics {
    /// Score from the built-in scorer.
    ///
    /// This value is for diagnostics and deterministic tie-breaking. It is not
    /// a stable semantic unit.
    pub default_score: f64,
}

/// A generated placement candidate.
///
/// You get candidates from [`CollisionReport::candidates`] after
/// [`resolve_anchor`](crate::resolve_anchor). They explain every placement the
/// resolver considered, including rejected candidates.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AnchorCandidate {
    /// Original option index: `0` is the preferred option, then fallbacks.
    pub option_index: usize,
    /// Stable option key, when supplied by the host.
    pub option_key: AnchorOptionKey,
    /// Candidate placement.
    pub placement: Placement,
    /// Resolved candidate rectangle in scene coordinates.
    pub rect: Rect,
    /// Resolved floating size after size constraints.
    pub floating_size: Size,
    /// Optional arrow geometry in scene coordinates.
    pub arrow: Option<ArrowFrame>,
    /// Collision, visibility, and stability metrics.
    pub metrics: CandidateMetrics,
    /// Diagnostic data for the default scorer.
    pub diagnostics: CandidateDiagnostics,
    /// Whether the candidate is rejected for selection.
    pub rejected: bool,
    /// Reason for rejection, when rejected.
    pub reject_reason: Option<AnchorRejectReason>,
}

/// Lightweight previous-frame state used for placement hysteresis.
///
/// Store this per logical overlay and pass it back through
/// [`AnchorInput::previous`](crate::AnchorInput::previous) on the next update.
/// Callers get one from [`PreviousAnchorFrame::from`] after resolving an
/// [`AnchorFrame`]; they do not need to construct this state by hand.
#[derive(Clone, Copy, Debug)]
pub struct PreviousAnchorFrame {
    option_index: usize,
    option_key: AnchorOptionKey,
    placement: Placement,
    rect: Rect,
    reference_rect: Rect,
    floating_size: Size,
}

impl From<&AnchorFrame> for PreviousAnchorFrame {
    fn from(frame: &AnchorFrame) -> Self {
        Self {
            option_index: frame.option_index,
            option_key: frame.option_key,
            placement: frame.placement,
            rect: frame.rect,
            reference_rect: frame.reference_rect,
            floating_size: frame.floating_size,
        }
    }
}

impl PreviousAnchorFrame {
    /// Returns the previous chosen option index.
    ///
    /// `0` is the preferred option passed to [`AnchorPolicy`](crate::AnchorPolicy);
    /// higher indexes correspond to fallback options in their original order.
    #[must_use]
    pub const fn option_index(&self) -> usize {
        self.option_index
    }

    /// Returns the previous chosen option key.
    #[must_use]
    pub const fn option_key(&self) -> AnchorOptionKey {
        self.option_key
    }

    /// Returns the previous chosen placement.
    ///
    /// Use this when a caller needs to inspect the stored hysteresis state.
    #[must_use]
    pub const fn placement(&self) -> Placement {
        self.placement
    }

    /// Returns the previous chosen floating rectangle in scene coordinates.
    ///
    /// This comes from the [`AnchorFrame::rect`] used to create this state.
    #[must_use]
    pub const fn rect(&self) -> Rect {
        self.rect
    }

    /// Returns the previous reference rectangle derived from the anchor.
    ///
    /// This comes from the [`AnchorFrame::reference_rect`] used to create this
    /// state.
    #[must_use]
    pub const fn reference_rect(&self) -> Rect {
        self.reference_rect
    }

    /// Returns the previous resolved floating size after constraints.
    ///
    /// This comes from the [`AnchorFrame::floating_size`] used to create this
    /// state.
    #[must_use]
    pub const fn floating_size(&self) -> Size {
        self.floating_size
    }
}

/// The chosen anchor geometry and candidate diagnostics.
///
/// This is returned by [`resolve_anchor`](crate::resolve_anchor). Use
/// [`PreviousAnchorFrame`] when the next update should use placement
/// hysteresis.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AnchorFrame {
    /// Original chosen option index: `0` is the preferred option, then fallbacks.
    pub option_index: usize,
    /// Chosen option key, when supplied by the host.
    pub option_key: AnchorOptionKey,
    /// Chosen floating rectangle in scene coordinates.
    pub rect: Rect,
    /// Chosen placement.
    pub placement: Placement,
    /// Reference rectangle derived from the anchor.
    pub reference_rect: Rect,
    /// Chosen floating size after constraints.
    pub floating_size: Size,
    /// Optional arrow geometry in scene coordinates.
    pub arrow: Option<ArrowFrame>,
    /// Transform origin in overlay-local coordinates.
    pub transform_origin: Point,
    /// Whether the chosen frame should be visible.
    pub visible: bool,
    /// Whether the chosen frame is clipped by the viewport or boundary.
    pub clipped: bool,
    /// Whether the anchor is detached from the viewport and boundary.
    pub detached: bool,
    /// Full candidate report for diagnostics.
    pub collision: CollisionReport,
}

/// Arrow geometry associated with an anchor candidate.
///
/// You get this from [`AnchorFrame::arrow`] or [`AnchorCandidate::arrow`] when
/// the policy includes [`AnchorConstraint::Arrow`](crate::AnchorConstraint::Arrow).
/// Coordinates are in the same scene space as the chosen floating rectangle.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArrowFrame {
    /// Arrow bounding rectangle in scene coordinates.
    pub rect: Rect,
    /// Arrow tip in scene coordinates, pointing toward the anchor.
    pub tip: Point,
    /// Placement side that produced the arrow.
    ///
    /// This is the chosen [`Placement`] side, not a drawing
    /// instruction for the floating rectangle edge. For example, `Side::Bottom`
    /// means the surface is below the anchor; a renderer will typically draw
    /// the arrow on the surface's top edge.
    pub side: Side,
    /// Arrow center in scene coordinates.
    pub center: Point,
    /// Whether the arrow center was clamped by arrow padding.
    pub clamped: bool,
}

/// Collision and choice diagnostics for an anchor resolution.
///
/// You get this from [`AnchorFrame::collision`]. It preserves all candidates
/// and reports whether hysteresis affected the chosen candidate.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CollisionReport {
    /// All generated candidates, in deterministic evaluation order.
    pub candidates: Vec<AnchorCandidate>,
    /// Index of the chosen candidate within [`CollisionReport::candidates`].
    pub chosen: usize,
    /// Previous placement supplied by [`AnchorInput::previous`](crate::AnchorInput::previous).
    pub previous_placement: Option<Placement>,
    /// Whether hysteresis changed the winner to keep the incumbent placement.
    pub hysteresis_applied: bool,
}

/// Reason a candidate could not be selected.
///
/// You get this from [`AnchorCandidate::reject_reason`] when a candidate was
/// rejected. Anchor resolution itself returns an [`AnchorFrame`] with
/// diagnostics instead of failing with an error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AnchorRejectReason {
    /// The anchor could not produce a reference rectangle.
    EmptyAnchor,
    /// The collision boundary is empty.
    EmptyBoundary,
    /// The viewport is empty.
    EmptyViewport,
    /// The candidate has no visible area after collision.
    NoVisibleArea,
    /// The anchor is detached from the viewport and boundary.
    Detached,
    /// A size constraint could not satisfy its minimum size.
    CannotSatisfyMinSize,
}
