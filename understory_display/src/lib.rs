// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained display-list primitives between toolkit/runtime state and paint
//! backends.
//!
//! `understory_display` provides a small retained display list with stable
//! item ids and a calm 2D drawing vocabulary. It is intended to sit between:
//!
//! - retained/widget/runtime layers such as `overstory`, and
//! - renderer-facing paint backends such as `imaging`.
//!
//! This crate intentionally does **not** own text shaping, widget semantics,
//! renderer backends, or compositor policy.
//!
//! ## First slice
//!
//! The initial op set is deliberately small:
//!
//! - filled rects,
//! - stroked rects,
//! - filled rounded rects,
//! - stroked rounded rects.
//!
//! ## Example
//!
//! See `overstory_visual_demo.rs` in the workspace examples crate for one
//! concrete flow:
//!
//! - `overstory::SceneSnapshot` -> `understory_display::DisplayList`
//! - `understory_display::DisplayList` -> `imaging::record::Scene`
//! - `imaging_vello_cpu` -> pixels in a window

#![no_std]

extern crate alloc;

mod builder;
mod ids;
mod item;
mod list;

pub use builder::DisplayListBuilder;
pub use ids::{ItemId, SemanticId};
pub use item::{DisplayItem, DisplayOp};
pub use list::DisplayList;

pub use kurbo;
pub use peniko;
