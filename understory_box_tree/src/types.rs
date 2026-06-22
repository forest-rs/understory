// Copyright 2025 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Public types for the box tree: node identifiers, flags, and local geometry.

use kurbo::{Affine, Insets, Rect, RoundedRect};

/// Identifier for a node in the tree.
///
/// This is a small, copyable handle that stays stable across updates but becomes
/// invalid when the underlying slot is reused.
/// It consists of a slot index and a generation counter.
///
/// ## Semantics
///
/// - On insert, a fresh slot is allocated with generation `1`.
/// - On remove, the slot is freed; any existing `NodeId` that pointed to that slot is now stale.
/// - On reuse of a freed slot, its generation is incremented, producing a new, distinct `NodeId`.
///
/// ### Newer
///
/// A `NodeId` is considered newer than another when it has a higher generation.
/// If generations are equal, the one with the higher slot index is considered newer.
/// This total order is used only for deterministic tie-breaks in
/// [hit testing](crate::Tree::hit_test_point).
///
/// ### Liveness
///
/// Use [`Tree::is_alive`](crate::Tree::is_alive) to check whether a `NodeId` still refers to a live node.
/// Stale `NodeId`s never alias a different live node because the generation must match.
///
/// ### Notes
///
/// - The generation increments on slot reuse and never decreases.
/// - `u32` is ample for practical lifetimes; behavior on generation overflow is unspecified.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct NodeId {
    idx: u32,
    generation: u32,
}

impl NodeId {
    #[inline(always)]
    pub(crate) const fn new(idx: u32, generation: u32) -> Self {
        Self { idx, generation }
    }

    #[inline(always)]
    pub(crate) const fn idx(self) -> usize {
        self.idx as usize
    }

    #[inline(always)]
    pub(crate) const fn generation(self) -> u32 {
        self.generation
    }
}

bitflags::bitflags! {
    /// Node flags controlling visibility, picking, and focus behavior.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct NodeFlags: u8 {
        /// Node is visible (participates in rendering and intersection queries).
        const VISIBLE  = 0b0000_0001;
        /// Node is pickable (participates in hit testing).
        const PICKABLE = 0b0000_0010;
        /// Node is focusable (can receive keyboard focus).
        const FOCUSABLE = 0b0000_0100;
    }
}

impl Default for NodeFlags {
    #[inline(always)]
    fn default() -> Self {
        Self::VISIBLE | Self::PICKABLE
    }
}

/// Local geometry for a node.
#[derive(Clone, Debug)]
pub struct LocalNode {
    /// Local (untransformed) bounds for this node's own content.
    ///
    /// - Expressed in the node's local coordinate space, before `local_transform`.
    /// - Used to derive the node's world-space content AABB for paint damage and visibility
    ///   queries. Hit testing uses these bounds expanded by [`LocalNode::hit_slop`].
    /// - Children are **not** constrained by their parent's `local_bounds`; their bounds are
    ///   computed independently from their own `LocalNode`.
    ///
    /// For non-axis-aligned content, use a loose AABB that fully contains what is drawn; it may be
    /// larger than the tight bounding box.
    pub local_bounds: Rect,
    /// Per-edge expansion of the node's *hit-test* region, in local coordinates.
    ///
    /// This is interaction slop: it grows (or, with negative values, shrinks) the area that
    /// [hit testing](crate::Tree::hit_test_point) treats as belonging to this node, without
    /// changing what the node draws or occupies for any other purpose. It exists so a small
    /// visual control (a thin slider, a hairline divider, a vector path) can present a larger,
    /// finger-friendly touch target.
    ///
    /// Semantics:
    /// - The hit region is `local_bounds + hit_slop` using kurbo's
    ///   [`Rect + Insets`](kurbo::Rect) convention, so **positive components expand outward**
    ///   and negative components contract.
    /// - Slop applies only to [`Tree::hit_test_point`](crate::Tree::hit_test_point). It does
    ///   **not** affect [`world_bounds`](crate::Tree::world_bounds),
    ///   [`intersect_rect`](crate::Tree::intersect_rect),
    ///   [`containing_point`](crate::Tree::containing_point), visibility, or paint damage, which
    ///   continue to use the true content `local_bounds`.
    /// - The expanded region is still subject to this node's `local_clip` and to every ancestor
    ///   clip, exactly like `local_bounds`. Slop does not let a node grab input outside the
    ///   clips it lives within.
    /// - Slop is expressed in local units and is therefore scaled and rotated along with the
    ///   node by `local_transform`. Callers wanting a constant screen-space target should size
    ///   the insets to undo the node's scale.
    ///
    /// Defaults to [`Insets::ZERO`], i.e. the hit region equals `local_bounds`.
    pub hit_slop: Insets,
    /// Local transform from this node's coordinate space into its parent's.
    ///
    /// - Combined with ancestor transforms to produce `world_transform`.
    /// - Applied to both `local_bounds` and `local_clip` when computing world-space data.
    /// - Order is ancestors * local: the local transform is applied before the ancestor
    ///   transforms to calculate the world transform (`world_transform = ancestors * local`).
    pub local_transform: Affine,
    /// Optional local clip (rounded-rect) applied to this node and its subtree.
    ///
    /// - Expressed in the node's local coordinate space and transformed into world space.
    /// - Combined with any ancestor clip to form an inherited `world_clip`.
    /// - The node's world-space AABB is intersected with this clip for spatial indexing.
    ///
    /// Intuitively:
    /// - Points outside `local_bounds` may still hit children.
    /// - Points outside `local_clip` (once transformed) cannot hit this node or any descendant.
    ///   Backends may still apply more precise clipping during rendering.
    pub local_clip: Option<RoundedRect>,
    /// The node's z-order within the [`Tree`](crate::Tree).
    ///
    /// This does not model stacking contexts.
    ///
    /// - Nodes with higher values are drawn on top of nodes with lower values.
    /// - Hit testing compares `z_index` when nodes within the tree overlap;
    ///   depth in the tree and insertion order are used as secondary tie-breakers.
    pub z_index: i32,
    /// Visibility and interaction flags.
    ///
    /// - [`NodeFlags::VISIBLE`] controls participation in visibility queries and hit testing.
    /// - [`NodeFlags::PICKABLE`] is consulted by hit testing.
    /// - [`NodeFlags::FOCUSABLE`] is consulted by focus/navigation layers.
    ///
    /// Flags do not affect layout; they only influence queries and higher-level behavior.
    pub flags: NodeFlags,
}

impl Default for LocalNode {
    #[inline(always)]
    fn default() -> Self {
        Self {
            local_bounds: Rect::ZERO,
            hit_slop: Insets::ZERO,
            local_transform: Affine::IDENTITY,
            local_clip: None,
            z_index: 0,
            flags: NodeFlags::default(),
        }
    }
}
