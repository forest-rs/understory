// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// After you edit the crate's doc comment, run this command, then check README.md for any missing links
// cargo rdme --workspace-project=understory_box_decoration --heading-base-level=0

//! Renderer-neutral box decoration geometry primitives.
//!
//! `understory_box_decoration` owns resolved geometry for painted boxes:
//! physical edge widths, box-area contours, corner shapes, side regions, and
//! on-demand path emission. It deliberately leaves style cascade, CSS parsing,
//! layout, brushes, images, hit policy, and renderer command emission to
//! higher-level crates.
//!
//! If those values come from dependency properties, use
//! `understory_presentation_properties` to register canonical surface
//! properties and resolve them into `understory_presentation` primitives before
//! asking this crate for final geometry.
//!
//! The first implemented contour family covers CSS-style box contours with
//! elliptical radii and shaped corners. It supports round, square, bevel, and
//! superellipse-based corner shapes, scales adjacent radii with the CSS
//! smallest factor rule when they would overlap a side, and derives padding
//! and content contours from concrete border and padding widths.
//!
//! ## Specification baseline
//!
//! The current API is based on the box contour pieces of
//! [CSS Backgrounds and Borders Module Level 3], specifically the
//! `border-radius` model, elliptical corner radii, inner edge derivation, and
//! the radius overlap reduction rule for adjacent corners.
//!
//! [CSS Borders and Box Decorations Module Level 4] informs the contour model,
//! especially `corner-shape` and superellipse corners. Larger Level 4 features
//! such as `border-shape`, partial borders, and richer shadow controls remain
//! roadmap material. This crate is intended to grow toward those features
//! while keeping style parsing and renderer lowering outside the crate
//! boundary.
//!
//! ## Minimal example
//!
//! ```rust
//! use kurbo::{BezPath, Rect, Size};
//! use understory_box_decoration::{
//!     BoxArea, BoxDecorationGeometry, CornerRadii, CornerShape, CornerShapes,
//!     Edges,
//! };
//!
//! let geometry = BoxDecorationGeometry::from_border_box(
//!     Rect::new(0.0, 0.0, 120.0, 80.0),
//!     Edges::all(4.0),
//!     Edges::all(8.0),
//!     CornerRadii::all(Size::new(18.0, 12.0)),
//!     CornerShapes::all(CornerShape::squircle()),
//! );
//!
//! assert_eq!(geometry.padding_box, Rect::new(4.0, 4.0, 116.0, 76.0));
//! assert_eq!(geometry.content_box, Rect::new(12.0, 12.0, 108.0, 68.0));
//!
//! // A renderer can reuse path storage while asking for the concrete paths it
//! // needs for a fill, clip, border, shadow, or hit region.
//! let mut clip_path = BezPath::new();
//! geometry.write_background_clip(BoxArea::Padding, &mut clip_path);
//!
//! let mut border_path = BezPath::new();
//! geometry.write_border_ring_path(&mut border_path);
//! assert!(!clip_path.is_empty());
//! assert!(!border_path.is_empty());
//! ```
//!
//! ## Boundary and invariants
//!
//! This crate treats inputs as already resolved into local coordinate units.
//! Constructors harden those inputs for geometry consumers:
//!
//! - rectangles are normalized to non-negative width and height;
//! - negative or non-finite border widths become zero;
//! - negative or non-finite padding widths become zero;
//! - negative or non-finite radii become zero;
//! - border-edge radii are scaled so top, right, bottom, and left side pairs
//!   fit;
//! - padding and content edge radii are derived from the previous contour's
//!   radii and then scaled to fit their own boxes.
//!
//! The crate itself is `#![no_std]`. The default `libm` feature forwards to
//! Kurbo's libm-backed floating point helpers so ordinary builds remain
//! `no_std`-friendly. Enable the `std` feature when an application wants
//! Kurbo's standard-library support.
//!
//! ## Roadmap
//!
//! Near-term work should add resolved length-percentage radii so CSS parsing
//! layers can defer percentage resolution until the border box is known. After
//! that, the natural coverage expansion is corner transition regions,
//! `box-shadow` spread geometry, and richer background painting areas. Level 4
//! `border-shape` should probably consume a separate CSS-shapes value crate
//! rather than making this crate own every shape syntax.
//!
//! [CSS Backgrounds and Borders Module Level 3]: https://www.w3.org/TR/css-backgrounds-3/#border-radius
//! [CSS Borders and Box Decorations Module Level 4]: https://drafts.csswg.org/css-borders-4/

#![no_std]

mod contour;
mod edges;
mod geometry;
mod path;
mod radii;
mod shape;
mod side;
mod util;

pub use contour::{BoxContour, ContourSideSpan, ResolvedCorner};
pub use edges::Edges;
pub use geometry::{BorderSideGeometry, BoxDecorationGeometry, inset_rect};
pub use radii::{CornerRadii, Corners};
pub use shape::{CornerShape, CornerShapes, Superellipse};
pub use side::{BoxArea, Side};
