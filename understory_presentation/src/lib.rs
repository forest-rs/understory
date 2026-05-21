// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// After you edit the crate's doc comment, run this command, then check README.md for any missing links
// cargo rdme --workspace-project=understory_presentation --heading-base-level=0

//! Understory Presentation: retained, resolved drawing intent.
//!
//! This crate stores the "what to draw" layer that sits between a widget tree
//! and paint. A toolkit writes already-resolved drawing primitives into a
//! [`PresentationStore`], keyed by its own geometry ids. Paint can then walk the
//! geometry tree, look up presentation entries, and lower primitives without
//! reading properties or running style cascade resolution.
//!
//! This crate deliberately does **not** own layout bounds, scene traversal
//! order, global transforms/clips, hit testing, style cascade, property
//! storage, behavior dispatch, templates, or renderer command emission.
//!
//! ## Fence
//!
//! This crate owns retained, resolved drawing intent keyed by caller-owned
//! geometry ids; it explicitly does not own layout/scene geometry,
//! property/style resolution, behavior, or paint command emission.
//!
//! ## Concepts and glossary
//!
//! - [`PresentationStore`]: flat keyed cache of presentation nodes plus dirty
//!   tracking.
//! - [`PresentationNode`]: source back-reference and primitive list for one
//!   drawable geometry node.
//! - [`Primitive`]: resolved drawing primitive stored on a presentation node.
//! - [`SurfacePrimitive`]: resolved surface fill/border intent.
//! - [`TextPrimitive`]: umbrella for resolved text drawing intent.
//! - [`PlainTextPrimitive`]: resolved plain-text content, foreground brush, and
//!   `parlance`-based single-run style.
//!
//! ## Model
//!
//! The store is generic over two ids:
//!
//! - `NodeKey`: the caller's geometry key, often an `understory_box_tree`
//!   node id.
//! - `SourceKey`: the caller's widget, element, template part, or diagnostic
//!   key, used for back-references.
//!
//! The presentation store is intentionally flat. It stores no parent/child
//! structure and no layout/scene geometry. Structural truth and traversal
//! order belong to the caller's geometry tree. Individual primitives may still
//! own local drawing geometry, such as future path data.
//!
//! Mutating store operations mark the affected `NodeKey` dirty. Dirty keys are
//! deduplicated and drained in first-dirty order with
//! [`PresentationStore::take_dirty`].
//!
//! ## Feature flags
//!
//! - `default`: enables `libm` so the crate builds as `no_std` by default.
//! - `libm`: forwards `peniko/libm` for `no_std` float math.
//! - `std`: forwards `peniko/std` and `parlance/std`.
//!
//! If default features are disabled, callers must enable either `libm` or
//! `std`.
//!
//! ```sh
//! cargo check -p understory_presentation --no-default-features --features libm
//! cargo check -p understory_presentation --no-default-features --features std
//! ```
//!
//! ## Minimal example
//!
//! ```rust
//! use understory_presentation::{
//!     Brush, Color, PresentationStore, Primitive, RoundedRectRadii, TextContent,
//! };
//!
//! #[derive(Clone, Copy, Debug, PartialEq, Eq)]
//! struct SourceKey {
//!     widget: u32,
//!     role: &'static str,
//! }
//!
//! let root = 1_u32;
//! let label = 2_u32;
//! let source_background = SourceKey { widget: 10, role: "background" };
//! let source_content = SourceKey { widget: 10, role: "content" };
//!
//! let mut store: PresentationStore<u32, SourceKey> = PresentationStore::new();
//! store.insert(root, source_background);
//! store.insert(label, source_content);
//!
//! let surface = store.surface_mut(root).unwrap();
//! surface.set_background(Color::from_rgb8(38, 92, 142));
//! surface.corner_radii = RoundedRectRadii::from_single_radius(6.0);
//!
//! let text = store.plain_text_mut(label).unwrap();
//! text.content = TextContent::plain("Run");
//! text.foreground = Some(Brush::from(Color::WHITE));
//!
//! let dirty: Vec<_> = store.take_dirty().collect();
//! assert_eq!(dirty, vec![root, label]);
//!
//! let label_node = store.node(label).unwrap();
//! assert_eq!(label_node.source().role, "content");
//! assert!(matches!(label_node.primitives()[0], Primitive::Text(_)));
//! ```

#![no_std]

extern crate alloc;

mod primitive;
mod store;

pub use parlance::{
    BaseDirection, FontFamily, FontFamilyName, FontStyle, FontWeight, FontWidth, GenericFamily,
    Language, OverflowWrap, TextWrapMode, WordBreak,
};
pub use peniko::kurbo::RoundedRectRadii;
pub use peniko::{Brush, Color};
pub use primitive::{
    BackgroundLayer, Border, BorderSide, PlainTextPrimitive, Primitive, Shadow, SurfacePrimitive,
    TextAlign, TextContent, TextLayout, TextLineHeight, TextOverflow, TextPrimitive, TextStyle,
};
pub use store::{PresentationNode, PresentationStore};
