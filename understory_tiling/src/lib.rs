// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// After you edit the crate's doc comment, run this command, then check README.md for any missing links
// cargo rdme --workspace-project=understory_tiling --heading-base-level=0

#![cfg_attr(not(feature = "std"), no_std)]

//! Understory Tiling: headless tiling, docking, and layout interaction primitives.
//!
//! This crate provides a small, renderer-agnostic core for persistent tiling
//! trees and flattened layout frames. It is intended to sit underneath docking
//! widgets and workbench shells without knowing about any particular renderer,
//! widget system, document model, or window system.
//!
//! The core concepts are:
//!
//! - [`TileTree`]: persistent semantic tree of splits, tab groups, and panes.
//! - [`LayoutFrame`]: flattened solved geometry for rendering and hit testing.
//! - [`TileOp`]: semantic committed mutations such as split, move, activate,
//!   reorder, and resize.
//! - [`DockProposal`] and [`ResizeProposal`]: uncommitted interaction results
//!   that can be previewed before they mutate the tree.
//! - [`PaneId`] and [`TileId`]: opaque ids used to integrate with an embedding
//!   application.
//!
//! This crate deliberately does **not** know about:
//!
//! - pane contents,
//! - tab or button drawing,
//! - themes or animation,
//! - document save prompts,
//! - native windows,
//! - accessibility backends,
//! - Overstory-specific integration.
//!
//! ## Fence
//!
//! This crate owns semantic layout structure, layout solving, flattened frames,
//! hit regions, semantic mutation operations, and proposal plumbing; it
//! explicitly does not own pane contents, chrome drawing, app policy, document
//! lifecycle, renderer integration, or widget behavior.
//!
//! ## Minimal example
//!
//! ```rust
//! use kurbo::{Point, Rect, Size};
//! use understory_tiling::{
//!     hit_test, Axis, LayoutInput, PaneId, Placement, TileOp, TileTree,
//! };
//!
//! let mut tree = TileTree::single_pane(PaneId(1));
//! tree.apply(TileOp::SplitPane {
//!     pane: PaneId(1),
//!     axis: Axis::Horizontal,
//!     new_pane: PaneId(2),
//!     placement: Placement::After,
//!     share: 0.5,
//! })?;
//!
//! let frame = tree.layout(LayoutInput {
//!     bounds: Rect::new(0.0, 0.0, 800.0, 600.0),
//!     tab_bar_thickness: 28.0,
//!     split_handle_thickness: 6.0,
//!     min_pane_size: Size::new(80.0, 80.0),
//!     generate_drop_targets: false,
//! });
//!
//! assert_eq!(frame.panes.len(), 2);
//! assert!(hit_test(&frame, Point::new(10.0, 10.0)).is_some());
//! # Ok::<(), understory_tiling::TileError>(())
//! ```
//!
//! This crate is `no_std` and uses `alloc` when built without default features.
//! Enable the `libm` feature for no-std targets that need Kurbo geometry math.
//! Enable the `serde` feature to serialize layout trees, snapshots, frames,
//! policy data, and interaction proposals.

extern crate alloc;

mod frame;
mod ids;
mod interaction;
mod model;
mod ops;
mod policy;
mod snapshot;
#[cfg(test)]
mod tests;
mod tree;
mod util;

pub use frame::{
    FrameChange, FrameDiff, FrameItemDiff, FrameItemId, HitKind, HitRegion, LayoutFrame, PaneFrame,
    SplitChildFrame, SplitHandleFrame, TabBarFrame, TabFrame, diff_frames, hit_test,
};
pub use ids::{PaneId, Revision, SurfaceId, TileId};
pub use interaction::{
    CommitMode, DockProposal, DragIntent, DragOptions, DragSession, DragSource, DragSubject,
    DragUpdate, DraggedFrame, DropTargetFrame, DropTargetId, GhostFrame, GhostKind,
    InteractionFrame, InteractionState, OverlayFrame, OverlayHitRegion, PendingDrag, PreviewFrame,
    Proposal, ResizeOptions, ResizeProposal, ResizeSession, ResizeUpdate, begin_drag,
    begin_pending_drag, begin_resize, commit_drag, commit_resize, drop_targets_for_drag,
    pick_drop_target, update_drag, update_resize,
};
pub use kurbo::{Point, Rect, Size};
pub use model::{
    Axis, LayoutConstraints, LayoutInput, PaneNode, Placement, SplitConstraints, SplitNode,
    SurfaceKind, TabBarPlacement, TabNode, TileNode, TileSurface,
};
pub use ops::{DockTarget, TileError, TileOp};
pub use policy::{
    DockPolicyData, EdgeSet, PaneCapabilities, ProposalValidationInput, ValidatedProposal, ZoneSet,
    commit_proposal, validate_proposal,
};
pub use snapshot::{LayoutSnapshot, RepairAction, RepairReport, RestoreOptions, restore_snapshot};
pub use tree::TileTree;
