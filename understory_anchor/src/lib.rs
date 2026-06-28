// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// After you edit the crate's doc comment, run this command, then check README.md for any missing links
// cargo rdme --workspace-project=understory_anchor --heading-base-level=0

//! Headless anchored overlay geometry resolution.
//!
//! `understory_anchor` owns deterministic geometry for floating surfaces that
//! are attached to an anchor: popovers, menus, tooltips, combobox popups,
//! completion lists, hover cards, context menus, inspector bubbles, and
//! selection/caret anchored affordances. It deliberately does not own overlay
//! lifecycle, modality, dismissal, focus movement, rendering, animation,
//! platform windows, or accessibility integration.
//!
//! The resolver is pure, but not memoryless. Callers pass lightweight
//! [`PreviousAnchorFrame`] state back through [`AnchorInput::previous`] so the
//! scorer can keep a viable incumbent placement when two candidates are nearly
//! tied. This avoids visible flip-flop near viewport or scroll-container edges
//! while keeping the resolver itself stateless.
//!
//! ## Fence
//!
//! This crate owns anchored geometry resolution, candidate diagnostics,
//! collision response, arrows, transform origins, and previous-frame placement
//! stability; it explicitly does not own overlay lifecycle, input events,
//! dismissal, focus, rendering, animation, or platform integration.
//!
//! ## Core flow
//!
//! Resolution is intentionally explicit:
//!
//! 1. [`AnchorInput`] describes the current scene facts.
//! 2. [`AnchorPolicy`] describes preferred and fallback
//!    [`AnchorPositionOption`] values, scoring, ordering, and hysteresis.
//! 3. [`resolve_anchor`] returns an [`AnchorFrame`] containing the chosen
//!    geometry plus every generated [`AnchorCandidate`] in a
//!    [`CollisionReport`].
//!
//! This is a useful foundation for CSS Anchor Positioning adapters, but it is
//! not a CSS implementation. A CSS layer should resolve anchor lookup, cascade,
//! writing mode, logical axes, `position-area`, `anchor()`, `anchor-size()`, and
//! `@position-try` rules into physical rectangles and [`AnchorPositionOption`]
//! values before calling this crate.
//!
//! ## Minimal example
//!
//! ```rust
//! use kurbo::{Insets, Rect, Size};
//! use understory_anchor::{
//!     Anchor, AnchorConstraint, AnchorInput, AnchorPolicy, AnchorPositionOption,
//!     Placement, resolve_anchor,
//! };
//!
//! let constraints = [
//!     AnchorConstraint::Offset {
//!         main_axis: 8.0,
//!         cross_axis: 0.0,
//!     },
//!     AnchorConstraint::Shift {
//!         padding: Insets::uniform(8.0),
//!     },
//!     AnchorConstraint::Arrow {
//!         size: Size::new(12.0, 6.0),
//!         padding: 8.0,
//!     },
//! ];
//! let fallbacks = [
//!     AnchorPositionOption::new(Placement::TOP).with_constraints(&constraints),
//!     AnchorPositionOption::new(Placement::RIGHT).with_constraints(&constraints),
//!     AnchorPositionOption::new(Placement::LEFT).with_constraints(&constraints),
//! ];
//!
//! let input = AnchorInput {
//!     anchor: Anchor::Rect(Rect::new(100.0, 100.0, 180.0, 132.0)),
//!     floating_size: Size::new(260.0, 180.0),
//!     viewport: Rect::new(0.0, 0.0, 1200.0, 800.0),
//!     boundary: Rect::new(0.0, 0.0, 1200.0, 800.0),
//!     previous: None,
//! };
//!
//! let policy = AnchorPolicy::new(
//!     AnchorPositionOption::new(Placement::BOTTOM).with_constraints(&constraints),
//!     &fallbacks,
//! );
//!
//! let frame = resolve_anchor(input, policy);
//! assert_eq!(frame.placement, Placement::BOTTOM);
//! assert!(frame.visible);
//! ```
//!
//! ## Multi-rect anchors
//!
//! Wrapped selections and text carets often expose more than one rectangle.
//! [`Anchor::Rects`] lets callers choose whether placement should use the
//! bounding box, first rect, last rect, primary rect, focus rect, or largest
//! rect.
//!
//! ```rust
//! use kurbo::{Rect, Size};
//! use understory_anchor::{
//!     Anchor, AnchorInput, AnchorPolicy, AnchorRects, Placement, RectReference,
//!     resolve_anchor,
//! };
//!
//! let selection = [
//!     Rect::new(100.0, 100.0, 240.0, 118.0),
//!     Rect::new(80.0, 120.0, 260.0, 138.0),
//!     Rect::new(80.0, 140.0, 140.0, 158.0),
//! ];
//!
//! let frame = resolve_anchor(
//!     AnchorInput {
//!         anchor: Anchor::Rects {
//!             rects: AnchorRects {
//!                 rects: &selection,
//!                 primary: Some(0),
//!                 focus: Some(2),
//!             },
//!             reference: RectReference::Focus,
//!         },
//!         floating_size: Size::new(160.0, 80.0),
//!         viewport: Rect::new(0.0, 0.0, 400.0, 300.0),
//!         boundary: Rect::new(0.0, 0.0, 400.0, 300.0),
//!         previous: None,
//!     },
//!     AnchorPolicy::placement(Placement::BOTTOM_START),
//! );
//!
//! assert_eq!(frame.reference_rect, selection[2]);
//! ```
//!
//! The crate is `no_std` and uses `alloc` when built without the `std`
//! feature. The default `libm` feature forwards Kurbo's libm-backed geometry
//! support for ordinary `no_std` builds. Enable `std` when an application wants
//! Kurbo's standard-library support, and enable `serde` to serialize policy,
//! option, frame, candidate, and diagnostic data.
//!
//! There is no math-free build of this crate: if you disable default features,
//! enable either `libm` or `std` on `understory_anchor` so Kurbo and its geometry
//! dependencies have floating-point math support.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

mod anchor;
mod frame;
mod placement;
mod policy;
mod resolver;

pub use anchor::{Anchor, AnchorRects, RectReference, reference_rect};
pub use frame::{
    AnchorCandidate, AnchorFrame, AnchorRejectReason, ArrowFrame, CandidateDiagnostics,
    CandidateMetrics, CollisionReport, PreviousAnchorFrame,
};
pub use placement::{Align, Placement, Side};
pub use policy::{
    AnchorConstraint, AnchorInput, AnchorOptionKey, AnchorPolicy, AnchorPositionOption,
    HysteresisPolicy, PositionTryOrder, ScoringPolicy,
};
pub use resolver::resolve_anchor;
