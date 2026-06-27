// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Opaque identity for a tile node inside a [`TileTree`](crate::TileTree).
///
/// You usually get one from [`TileTree::root`](crate::TileTree::root),
/// [`TileTree::node`](crate::TileTree::node), layout frames, hit results, or
/// drop targets. Pass it back to operations such as
/// [`TileOp::ReorderTab`](crate::TileOp::ReorderTab) or
/// [`DockTarget::Split`](crate::DockTarget::Split); do not interpret the number
/// as a stable arena index outside this crate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TileId(
    /// Numeric tile id assigned by the tree arena.
    pub u32,
);

/// Opaque identity for an application-owned pane.
///
/// The embedding application creates these ids and passes them into
/// [`TileTree::single_pane`](crate::TileTree::single_pane),
/// [`TileNode::pane`](crate::TileNode::pane), and [`TileOp`](crate::TileOp)
/// values. Layout, hit testing, and interaction frames return the same ids so
/// the app can attach rendered chrome and pane contents.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaneId(
    /// Numeric pane id assigned by the embedding application.
    pub u32,
);

/// Opaque identity for an abstract layout surface.
///
/// Used by [`TileSurface`](crate::TileSurface) to reserve API space for root,
/// floating, and external surfaces. The current layout pass only operates on
/// the tree root.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SurfaceId(
    /// Numeric surface id assigned by the tiling core or embedding layer.
    pub u32,
);

/// Monotonic revision token for layout tree changes.
///
/// Read it with [`TileTree::revision`](crate::TileTree::revision). Layout
/// frames and interaction sessions copy the revision so commits such as
/// [`commit_drag`](crate::commit_drag) can reject stale input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Revision(
    /// Numeric revision value.
    pub u64,
);
