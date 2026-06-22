// Copyright 2025 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// After you edit the crate's doc comment, run this command, then check README.md for any missing links
// cargo rdme --workspace-project=understory_box_tree --heading-base-level=0

//! Understory Box Tree: a Kurbo-native, spatially indexed box tree.
//!
//! Understory Box Tree is a reusable building block for UIs, canvas and vector editors, and CAD viewers.
//!
//! - Represents a hierarchy of regions with local transforms, clips, z-order, and flags.
//! - Provides hit testing and rectangle intersection queries over world-space AABBs.
//! - Supports batched updates with a [`Tree::commit`] step that yields coarse damage regions.
//!
//! It aims for a stable, minimal API and leaves room to evolve internals (for example a pluggable spatial index) without churn at call sites.
//!
//! ## Where this fits: three-tree model
//!
//! We’re standardizing on a simple separation of concerns for UI stacks.
//! - Widget tree: interaction/state.
//! - Box tree: geometry/spatial indexing (this crate).
//! - Render tree: display list (future crate).
//!
//! The box tree computes world-space content and hit AABBs from local bounds,
//! hit slop, transforms, and clips, and synchronizes hit AABBs into a spatial
//! index for fast hit testing and coarse visibility queries.
//! This decouples scene structure from the spatial acceleration and makes debugging and incremental updates tractable.
//!
//! ## Not a layout engine
//!
//! This crate does not perform layout (measurement or arrangement) or apply layout policies such as flex, grid, or stack.
//! Upstream code is expected to compute positions and sizes using whatever layout system you choose and then update this tree with the resulting world-space boxes, transforms, optional clips, and z-order.
//! Think of this as a scene and spatial index, not a layout system.
//!
//! This crate also does not model stacking contexts, opacity, or blend modes. It provides a single
//! global z ordering (`z_index`) over boxes plus hit-testing and visibility queries. Higher-level
//! code is expected to introduce groups, stacking semantics, and paint order if needed.
//!
//! ## Integration with Understory Index
//!
//! This crate uses [`understory_index`] for spatial queries. You can choose the backend and scalar to
//! fit your workload (flat vector, grid, R-tree, or BVH). Float inputs are
//! assumed to be finite (no NaNs). AABBs are loose for non-axis-aligned transforms and rounded
//! clips: we store an axis-aligned box that fully contains what is drawn, but it is not
//! guaranteed to be tight.
//!
//! See [`understory_index::Index`],
//! [`understory_index::backends::GridF32`]/[`understory_index::backends::GridF64`]/[`understory_index::backends::GridI64`],
//! [`understory_index::backends::RTreeF32`]/[`understory_index::backends::RTreeF64`]/[`understory_index::backends::RTreeI64`],
//! and
//! [`understory_index::backends::BvhF32`]/[`understory_index::backends::BvhF64`]/[`understory_index::backends::BvhI64`]
//! for details.
//!
//! ## API overview
//!
//! - [`Tree`]: container managing nodes and the spatial index synchronization.
//! - [`LocalNode`]: per-node local data (bounds, hit slop, transform, optional clip, z, flags).
//!   See [`LocalNode::flags`] for visibility/picking/focusable controls and
//!   [`LocalNode::hit_slop`] for per-node hit-target expansion.
//! - [`NodeFlags`]: visibility, picking, and focusable controls.
//! - [`NodeId`]: generational handle of a node.
//! - [`QueryFilter`]: restricts hit/intersect results (visible/pickable/focusable).
//!   See [`NodeFlags::VISIBLE`], [`NodeFlags::PICKABLE`], and [`NodeFlags::FOCUSABLE`].
//!
//! Key operations:
//! - [`Tree::insert`] → [`NodeId`]
//! - [`Tree::set_local_transform`] / [`Tree::set_local_clip`] /
//!   [`Tree::set_local_bounds`] / [`Tree::set_hit_slop`] / [`Tree::set_flags`]
//! - [`Tree::commit`] → damage summary; updates world data and the spatial index.
//! - [`Tree::hit_test_point`] and [`Tree::intersect_rect`].
//! - [`Tree::z_index`] exposes the stacking order of a live [`NodeId`].
//! - [`Tree::parent_of`] returns the parent of a live [`NodeId`].
//! - [`Tree::flags`] returns the [`NodeFlags`] of a live [`NodeId`].
//! - [`Tree::world_transform`] / [`Tree::world_bounds`] / [`Tree::world_hit_bounds`]
//!   expose the local→world transform, the world-space content AABB, and the
//!   world-space hit AABB (content expanded by hit slop) for a live [`NodeId`].
//! - [`Tree::local_transform`] / [`Tree::local_bounds`] / [`Tree::hit_slop`] /
//!   [`Tree::local_clip`] expose the node's current local geometry state for a
//!   live [`NodeId`].
//! - [`Tree::children_of`] returns the children of a live [`NodeId`].
//! - [`Tree::next_depth_first`] and [`Tree::prev_depth_first`] provide depth-first tree traversal.
//!
//! ## Damage and debugging notes
//!
//! - [`Tree::commit`] batches adds/updates/removals and produces coarse damage (added/removed AABBs and
//!   old/new pairs for moved nodes). The reported rectangles may overlap and are not a minimal cover,
//!   but are sufficient to bound a paint traversal in most UIs.
//! - World AABBs are loose under rotation/shear and rounded-rect clips are approximated by
//!   their axis-aligned bounds for acceleration; precise hit-filtering is applied where cheap.
//! - Hit geometry and content geometry are kept separate. [`LocalNode::hit_slop`] expands a
//!   node's hit-test target (for example to meet a minimum touch-target size) without changing
//!   its painted content. Only [`Tree::hit_test_point`] honors the slop; `world_bounds`,
//!   `intersect_rect`, `containing_point`, and damage continue to report true content. The
//!   expanded region is still subject to the node's own clip and all ancestor clips.
//!
//! ## Examples
//!
//! - `examples/basic_box_tree.rs`: builds a trivial tree, commits, and runs a couple of queries.
//! - `examples/visible_list.rs`: demonstrates using `intersect_rect` to compute a visible set,
//!   a building block for virtualization.
//!
//! This crate is `no_std` and uses `alloc`.

#![no_std]

extern crate alloc;

mod damage;
mod tree;
mod types;
mod util;

pub use damage::Damage;
pub use tree::{Hit, QueryFilter, Tree};
pub use types::{LocalNode, NodeFlags, NodeId};
