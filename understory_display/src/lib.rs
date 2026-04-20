// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained display-tree and display-list primitives between toolkit/runtime
//! state and paint backends.
//!
//! `understory_display` provides:
//!
//! - a small retained display tree for local measure/place,
//! - a flat retained display list as a lowering target,
//! - and a calm 2D drawing vocabulary.
//!
//! It is intended to sit between:
//!
//! - retained/widget/runtime layers such as `overstory`, and
//! - renderer-facing paint backends such as `imaging`.
//!
//! This crate intentionally does **not** own widget semantics, renderer
//! backends, compositor policy, or higher-level rich text/editor semantics.
//!
//! ## First slice
//!
//! The first retained display-tree slice is deliberately small:
//!
//! - structural nodes such as stacks, padding, alignment, offsets, and fixed frames,
//! - shape leaves such as rects and rounded rects,
//! - and, with the `std` feature, Parley-backed text leaves and glyph runs.
//!
//! ## Text
//!
//! Text shaping is available behind the `std` feature through [`TextEngine`].
//! This crate keeps that slice narrow:
//!
//! - shape text into retained glyph runs with Parley,
//! - let retained text leaves participate in local measure/place,
//! - carry font data and positioned glyphs in lowered display items,
//! - leave backend-specific glyph rendering to adapters above this crate.
//!
//! ## Example
//!
//! See `overstory_visual_demo.rs` in the workspace examples crate for one
//! concrete flow:
//!
//! - `overstory::SceneSnapshot` -> retained `understory_display::DisplayTree`
//! - retained `understory_display::DisplayTree` -> flat `understory_display::DisplayList`
//! - `understory_display::DisplayList` -> `imaging::record::Scene`
//! - `imaging_vello_cpu` -> pixels in a window

#![no_std]

extern crate alloc;

mod builder;
mod ids;
mod item;
mod list;
#[cfg(feature = "std")]
mod text;
mod tree;

pub use builder::DisplayListBuilder;
pub use ids::{ItemId, SemanticId};
pub use item::{DisplayGlyph, DisplayGlyphRun, DisplayItem, DisplayOp};
pub use list::DisplayList;
#[cfg(feature = "std")]
pub use text::{TextEngine, TextRunRequest};
#[cfg(feature = "std")]
pub use tree::DisplayText;
pub use tree::{
    BoxConstraints, DisplayAlign, DisplayLayout, DisplayNode, DisplayNodeKind, DisplayTree, Insets,
};

pub use kurbo;
#[cfg(feature = "std")]
pub use parley;
pub use peniko;
