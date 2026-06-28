// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::cmp::Ordering;

use kurbo::{Point, Rect};
use understory_anchor::AnchorFrame;

use crate::grace::{grace_shape_between, should_generate_grace};
use crate::{
    FocusScopeId, GracePolicy, GraceRegion, GraceShape, Modality, OverlayEntry, OverlayId,
    OverlayLayer, OverlayStack,
};

/// Geometry supplied for one open overlay during frame building.
///
/// Hosts usually create this from an [`AnchorFrame`] returned by
/// [`understory_anchor::resolve_anchor`]. The overlay rect is required; the
/// anchor rect can be supplied directly or through [`OverlayGeometry::anchor_frame`].
/// All rectangles must be finite; debug builds assert this at construction and
/// frame-building entry points.
#[derive(Clone, Copy, Debug)]
pub struct OverlayGeometry<'a> {
    /// Overlay this geometry belongs to.
    pub overlay: OverlayId,
    /// Resolved overlay rectangle in scene coordinates.
    pub rect: Rect,
    /// Optional anchor rectangle in scene coordinates.
    pub anchor_rect: Option<Rect>,
    /// Optional anchor frame that produced this overlay rectangle.
    pub anchor_frame: Option<&'a AnchorFrame>,
}

impl<'a> OverlayGeometry<'a> {
    /// Creates overlay geometry without anchor details.
    #[must_use]
    pub fn new(overlay: OverlayId, rect: Rect) -> Self {
        debug_assert!(rect.is_finite(), "overlay rectangle must be finite");
        Self {
            overlay,
            rect,
            anchor_rect: None,
            anchor_frame: None,
        }
    }

    /// Creates overlay geometry from a resolved anchor frame.
    #[must_use]
    pub fn from_anchor_frame(overlay: OverlayId, anchor_frame: &'a AnchorFrame) -> Self {
        Self::new(overlay, anchor_frame.rect).with_anchor_frame(anchor_frame)
    }

    /// Sets the anchor rectangle used for dismiss-region interiors.
    #[must_use]
    pub fn with_anchor_rect(mut self, anchor_rect: Rect) -> Self {
        debug_assert!(anchor_rect.is_finite(), "anchor rectangle must be finite");
        self.anchor_rect = Some(anchor_rect);
        self
    }

    /// Sets the anchor frame and replaces the overlay rect with its resolved rect.
    #[must_use]
    pub fn with_anchor_frame(mut self, anchor_frame: &'a AnchorFrame) -> Self {
        debug_assert!(
            anchor_frame.rect.is_finite(),
            "anchor frame rectangle must be finite",
        );
        debug_assert!(
            anchor_frame.reference_rect.is_finite(),
            "anchor frame reference rectangle must be finite",
        );
        debug_assert!(
            anchor_frame.transform_origin.is_finite(),
            "anchor frame transform origin must be finite",
        );
        self.rect = anchor_frame.rect;
        self.anchor_frame = Some(anchor_frame);
        self
    }

    pub(crate) fn resolved_anchor_rect(self) -> Option<Rect> {
        self.anchor_rect
            .or_else(|| self.anchor_frame.map(|frame| frame.reference_rect))
            .map(|rect| rect.abs())
    }
}

/// Inputs used to derive one overlay frame.
///
/// Build one per layout or input frame and pass it to [`build_overlay_frame`].
/// Missing geometry is skipped so that stack mutation and layout can remain
/// loosely coupled. The viewport, geometry rectangles, anchor-frame rectangles,
/// and pointer point must be finite; debug builds assert this contract at
/// [`build_overlay_frame`] entry.
#[derive(Clone, Copy, Debug)]
pub struct OverlayFrameInput<'a> {
    /// Visible viewport in scene coordinates.
    pub viewport: Rect,
    /// Geometry for overlays that have been laid out this frame.
    pub geometries: &'a [OverlayGeometry<'a>],
    /// Current pointer position, when known.
    pub pointer: Option<Point>,
    /// Hover grace policy for this frame.
    pub grace_policy: GracePolicy,
}

impl<'a> OverlayFrameInput<'a> {
    /// Creates frame input with no pointer and default hover grace policy.
    #[must_use]
    pub fn new(viewport: Rect, geometries: &'a [OverlayGeometry<'a>]) -> Self {
        debug_assert!(viewport.is_finite(), "viewport rectangle must be finite");
        Self {
            viewport,
            geometries,
            pointer: None,
            grace_policy: GracePolicy::default(),
        }
    }

    /// Sets the current pointer point.
    #[must_use]
    pub fn with_pointer(mut self, pointer: Point) -> Self {
        debug_assert!(pointer.is_finite(), "pointer must be finite");
        self.pointer = Some(pointer);
        self
    }

    /// Sets the hover grace policy.
    #[must_use]
    pub fn with_grace_policy(mut self, grace_policy: GracePolicy) -> Self {
        debug_assert_grace_policy(grace_policy);
        self.grace_policy = grace_policy;
        self
    }
}

/// Derived frame for rendering, hit testing, dismissal, focus, and modality.
///
/// You get this from [`build_overlay_frame`]. It is an inert snapshot: hosts
/// are expected to render from it and pass it to [`resolve_event`](crate::resolve_event),
/// not mutate it in place.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OverlayFrame {
    /// Visible overlay rectangles in increasing z-order.
    pub overlays: Vec<OverlayFrameEntry>,
    /// Underlays generated by modal and blocking overlays.
    pub underlays: Vec<UnderlayFrame>,
    /// Rectangular hit regions for overlays, underlays, and diagnostics.
    pub hit_regions: Vec<OverlayHitRegion>,
    /// Dismissal interiors and triggers by overlay.
    pub dismiss_regions: Vec<DismissRegion>,
    /// Pointer-travel grace regions between related overlays.
    pub grace_regions: Vec<GraceRegion>,
    /// Focus scopes derived from overlay focus policy.
    pub focus_scopes: Vec<FocusScopeFrame>,
}

/// Derived overlay rectangle and z-order.
///
/// You get these from [`OverlayFrame::overlays`]. A renderer can draw surfaces
/// in slice order or sort by [`OverlayFrameEntry::z`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OverlayFrameEntry {
    /// Overlay id.
    pub id: OverlayId,
    /// Parent overlay id, if any.
    pub parent: Option<OverlayId>,
    /// Overlay rectangle in scene coordinates.
    pub rect: Rect,
    /// Deterministic z-order. Larger values are visually above smaller values.
    pub z: i32,
    /// Semantic layer.
    pub layer: OverlayLayer,
    /// Overlay modality.
    pub modality: Modality,
}

/// Underlay generated by a modal or blocking overlay.
///
/// You get these from [`OverlayFrame::underlays`]. Rendering policy, color, and
/// animation remain host-owned.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnderlayFrame {
    /// Overlay that owns this underlay.
    pub owner: OverlayId,
    /// Underlay rectangle, usually the viewport.
    pub rect: Rect,
    /// Deterministic z-order, below the owning overlay.
    pub z: i32,
    /// Whether this underlay should block pointer input behind it.
    pub blocks_pointer: bool,
}

/// Rectangular hit region emitted in a derived frame.
///
/// You get these from [`OverlayFrame::hit_regions`] for coarse routing and
/// diagnostics. Dismiss regions themselves carry richer inside/grace data.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OverlayHitRegion {
    /// Overlay that owns this hit region.
    pub overlay: OverlayId,
    /// Region rectangle in scene coordinates.
    pub rect: Rect,
    /// Deterministic z-order.
    pub z: i32,
    /// Region kind.
    pub kind: OverlayHitKind,
}

/// Kind of an [`OverlayHitRegion`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OverlayHitKind {
    /// The overlay surface itself.
    Overlay,
    /// A modality underlay.
    Underlay,
    /// A coarse rectangle associated with a dismiss region.
    DismissRegion,
    /// A coarse rectangle associated with a grace region.
    GraceRegion,
}

/// Bitset of dismiss triggers derived from [`DismissPolicy`](crate::DismissPolicy).
///
/// You get this from [`DismissRegion::triggers`]. It is intentionally a tiny
/// hand-written bitset to keep the core crate dependency surface small.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DismissTriggers(u8);

impl DismissTriggers {
    /// Empty trigger set.
    pub const EMPTY: Self = Self(0);
    /// Escape-key dismissal.
    pub const ESCAPE_KEY: Self = Self(1 << 0);
    /// Pointer-down-outside dismissal.
    pub const POINTER_DOWN_OUTSIDE: Self = Self(1 << 1);
    /// Pointer-up-outside dismissal.
    pub const POINTER_UP_OUTSIDE: Self = Self(1 << 2);
    /// Focus-outside dismissal.
    pub const FOCUS_OUTSIDE: Self = Self(1 << 3);
    /// Anchor-blur dismissal.
    pub const ANCHOR_BLUR: Self = Self(1 << 4);

    /// Returns whether all bits in `other` are present.
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns whether no triggers are present.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns the union of two trigger sets.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub(crate) fn from_policy(policy: crate::DismissPolicy) -> Self {
        let mut triggers = Self::EMPTY;
        if policy.escape_key {
            triggers = triggers.union(Self::ESCAPE_KEY);
        }
        if policy.pointer_down_outside {
            triggers = triggers.union(Self::POINTER_DOWN_OUTSIDE);
        }
        if policy.pointer_up_outside {
            triggers = triggers.union(Self::POINTER_UP_OUTSIDE);
        }
        if policy.focus_outside {
            triggers = triggers.union(Self::FOCUS_OUTSIDE);
        }
        if policy.anchor_blur {
            triggers = triggers.union(Self::ANCHOR_BLUR);
        }
        triggers
    }
}

impl core::ops::BitOr for DismissTriggers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

/// Dismissal interior and grace data for one overlay.
///
/// You get these from [`OverlayFrame::dismiss_regions`]. Event resolution
/// treats points inside any listed rect or grace shape as inside the overlay's
/// dismissal boundary.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DismissRegion {
    /// Overlay this region belongs to.
    pub overlay: OverlayId,
    /// Rectangles that count as inside the overlay for outside dismissal.
    pub inside: Vec<Rect>,
    /// Grace shapes that also count as inside.
    pub grace: Vec<GraceShape>,
    /// Dismissal triggers that should consult this region.
    pub triggers: DismissTriggers,
}

/// Focus scope metadata derived from an overlay.
///
/// You get these from [`OverlayFrame::focus_scopes`]. The host owns actual
/// focus traversal and restoration.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FocusScopeFrame {
    /// Focus scope id.
    pub id: FocusScopeId,
    /// Overlay that owns this scope.
    pub overlay: OverlayId,
    /// Scope rectangle in scene coordinates.
    pub rect: Rect,
    /// Whether focus should be contained inside this scope.
    pub contain: bool,
    /// Whether previous focus should be restored when the overlay closes.
    pub restore_on_close: bool,
}

/// Builds a derived overlay frame from stack state and current geometry.
///
/// Call this after resolving overlay layout. Geometry values must be finite;
/// debug builds assert that contract at entry.
#[must_use]
pub fn build_overlay_frame(stack: &OverlayStack, input: OverlayFrameInput<'_>) -> OverlayFrame {
    debug_assert_frame_input(input);

    let viewport = input.viewport.abs();
    let order = ordered_entry_indexes(stack);
    let mut frame = OverlayFrame::default();

    for (order_index, entry_index) in order.iter().copied().enumerate() {
        let entry = &stack.entries()[entry_index];
        let Some(geometry) = geometry_for(input.geometries, entry.id) else {
            continue;
        };
        let rect = geometry.rect.abs();
        let z = z_for_index(order_index);
        let frame_entry = OverlayFrameEntry {
            id: entry.id,
            parent: entry.parent,
            rect,
            z,
            layer: entry.layer,
            modality: entry.modality,
        };
        frame.overlays.push(frame_entry);
        frame.hit_regions.push(OverlayHitRegion {
            overlay: entry.id,
            rect,
            z,
            kind: OverlayHitKind::Overlay,
        });

        if entry.modality != Modality::NonModal {
            let underlay = UnderlayFrame {
                owner: entry.id,
                rect: viewport,
                z: z.saturating_sub(1),
                blocks_pointer: entry.modality == Modality::Blocking,
            };
            frame.underlays.push(underlay);
            frame.hit_regions.push(OverlayHitRegion {
                overlay: entry.id,
                rect: viewport,
                z: underlay.z,
                kind: OverlayHitKind::Underlay,
            });
        }

        push_focus_scope(&mut frame.focus_scopes, entry, rect);
    }

    frame.grace_regions = build_grace_regions(stack, input);
    for grace in &frame.grace_regions {
        let z = frame_z(&frame, grace.child).unwrap_or(0).saturating_sub(1);
        frame.hit_regions.push(OverlayHitRegion {
            overlay: grace.parent,
            rect: grace.shape.bounds(),
            z,
            kind: OverlayHitKind::GraceRegion,
        });
    }

    for entry in stack.entries() {
        let triggers = DismissTriggers::from_policy(entry.dismiss);
        if triggers.is_empty() {
            continue;
        }
        let inside = inside_rects_for_overlay(stack, input.geometries, entry.id);
        if inside.is_empty() {
            continue;
        }
        let grace = frame
            .grace_regions
            .iter()
            .filter(|region| region.parent == entry.id || region.child == entry.id)
            .map(|region| region.shape)
            .collect();
        let dismiss_region = DismissRegion {
            overlay: entry.id,
            inside,
            grace,
            triggers,
        };
        if let Some(bounds) = dismiss_region
            .inside
            .iter()
            .copied()
            .reduce(|bounds, rect| bounds.union(rect))
        {
            frame.hit_regions.push(OverlayHitRegion {
                overlay: entry.id,
                rect: bounds,
                z: frame_z(&frame, entry.id).unwrap_or(0),
                kind: OverlayHitKind::DismissRegion,
            });
        }
        frame.dismiss_regions.push(dismiss_region);
    }

    frame
}

pub(crate) fn frame_z(frame: &OverlayFrame, overlay: OverlayId) -> Option<i32> {
    frame
        .overlays
        .iter()
        .find(|entry| entry.id == overlay)
        .map(|entry| entry.z)
}

pub(crate) fn geometry_for<'a, 'b>(
    geometries: &'a [OverlayGeometry<'b>],
    overlay: OverlayId,
) -> Option<&'a OverlayGeometry<'b>> {
    geometries
        .iter()
        .find(|geometry| geometry.overlay == overlay)
}

pub(crate) fn ordered_entry_indexes(stack: &OverlayStack) -> Vec<usize> {
    let mut indexes = (0..stack.entries().len()).collect::<Vec<_>>();
    indexes.sort_by(|a, b| compare_entries(stack, *a, *b));
    indexes
}

fn compare_entries(stack: &OverlayStack, a: usize, b: usize) -> Ordering {
    let left = &stack.entries()[a];
    let right = &stack.entries()[b];
    match left.layer.rank().cmp(&right.layer.rank()) {
        Ordering::Equal => {}
        ordering => return ordering,
    }
    if stack.is_descendant_of(left.id, right.id) {
        return Ordering::Greater;
    }
    if stack.is_descendant_of(right.id, left.id) {
        return Ordering::Less;
    }
    a.cmp(&b)
}

fn z_for_index(index: usize) -> i32 {
    let max_index = i32::MAX / 10 - 1;
    let index = i32::try_from(index).unwrap_or(max_index).min(max_index);
    index * 10 + 10
}

fn push_focus_scope(scopes: &mut Vec<FocusScopeFrame>, entry: &OverlayEntry, rect: Rect) {
    let Some(id) = entry.focus.scope_id(entry.id) else {
        return;
    };
    scopes.push(FocusScopeFrame {
        id,
        overlay: entry.id,
        rect,
        contain: entry.focus.contain(),
        restore_on_close: entry.focus.restore_on_close(),
    });
}

fn build_grace_regions(stack: &OverlayStack, input: OverlayFrameInput<'_>) -> Vec<GraceRegion> {
    let Some(pointer) = input.pointer else {
        return Vec::new();
    };
    let policy = input.grace_policy;
    let mut regions = Vec::new();
    for child in stack.entries() {
        let Some(parent) = child.parent else {
            continue;
        };
        let Some(parent_geometry) = geometry_for(input.geometries, parent) else {
            continue;
        };
        let Some(child_geometry) = geometry_for(input.geometries, child.id) else {
            continue;
        };
        let parent_rect = parent_geometry.rect.abs();
        let child_rect = child_geometry.rect.abs();
        if should_generate_grace(
            stack,
            parent,
            child.id,
            pointer,
            parent_rect,
            child_rect,
            policy,
        ) {
            regions.push(GraceRegion {
                parent,
                child: child.id,
                shape: grace_shape_between(pointer, parent_rect, child_rect),
            });
        }
    }
    regions
}

fn inside_rects_for_overlay(
    stack: &OverlayStack,
    geometries: &[OverlayGeometry<'_>],
    overlay: OverlayId,
) -> Vec<Rect> {
    let mut rects = Vec::new();
    for entry in stack.entries() {
        if entry.id != overlay && !stack.is_descendant_of(entry.id, overlay) {
            continue;
        }
        let Some(geometry) = geometry_for(geometries, entry.id) else {
            continue;
        };
        rects.push(geometry.rect.abs());
        if let Some(anchor_rect) = geometry.resolved_anchor_rect() {
            rects.push(anchor_rect);
        }
    }
    rects
}

fn debug_assert_frame_input(input: OverlayFrameInput<'_>) {
    debug_assert!(
        input.viewport.is_finite(),
        "viewport rectangle must be finite"
    );
    debug_assert_grace_policy(input.grace_policy);
    if let Some(pointer) = input.pointer {
        debug_assert!(pointer.is_finite(), "pointer must be finite");
    }
    for geometry in input.geometries {
        debug_assert!(
            geometry.rect.is_finite(),
            "overlay rectangle must be finite"
        );
        if let Some(anchor_rect) = geometry.anchor_rect {
            debug_assert!(anchor_rect.is_finite(), "anchor rectangle must be finite");
        }
        if let Some(anchor_frame) = geometry.anchor_frame {
            debug_assert!(
                anchor_frame.rect.is_finite(),
                "anchor frame rectangle must be finite",
            );
            debug_assert!(
                anchor_frame.reference_rect.is_finite(),
                "anchor frame reference rectangle must be finite",
            );
            debug_assert!(
                anchor_frame.transform_origin.is_finite(),
                "anchor frame transform origin must be finite",
            );
        }
    }
}

fn debug_assert_grace_policy(policy: GracePolicy) {
    debug_assert!(
        policy.max_distance.is_finite() && policy.max_distance >= 0.0,
        "grace max distance must be finite and non-negative",
    );
}

#[cfg(test)]
mod tests {
    use super::{OverlayFrameInput, OverlayGeometry, build_overlay_frame};
    use crate::{
        FocusPolicy, GracePolicy, Modality, OverlayBehavior, OverlayEntry, OverlayHitKind,
        OverlayId, OverlayLayer, OverlayOp, OverlayStack,
    };
    use kurbo::{Point, Rect, Size};
    use understory_anchor::{Anchor, AnchorInput, AnchorPolicy, Placement, resolve_anchor};

    fn entry(id: u64, layer: OverlayLayer, behavior: OverlayBehavior) -> OverlayEntry {
        OverlayEntry::new(OverlayId(id), layer, behavior)
    }

    #[test]
    fn frame_orders_parent_before_child() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open {
                entry: entry(1, OverlayLayer::Menu, OverlayBehavior::Menu),
            })
            .expect("parent should open");
        stack
            .apply(OverlayOp::Open {
                entry: entry(2, OverlayLayer::Menu, OverlayBehavior::Submenu)
                    .with_parent(OverlayId(1)),
            })
            .expect("child should open");
        let geometries = [
            OverlayGeometry::new(OverlayId(2), Rect::new(100.0, 0.0, 180.0, 80.0)),
            OverlayGeometry::new(OverlayId(1), Rect::new(0.0, 0.0, 80.0, 80.0)),
        ];

        let frame = build_overlay_frame(
            &stack,
            OverlayFrameInput::new(Rect::new(0.0, 0.0, 400.0, 300.0), &geometries),
        );

        assert_eq!(frame.overlays[0].id, OverlayId(1), "parent should be first");
        assert_eq!(frame.overlays[1].id, OverlayId(2), "child should be second");
        assert!(
            frame.overlays[0].z < frame.overlays[1].z,
            "child should have higher z",
        );
    }

    #[test]
    fn blocking_overlay_emits_blocking_underlay() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open {
                entry: entry(1, OverlayLayer::Modal, OverlayBehavior::Dialog)
                    .with_modality(Modality::Blocking),
            })
            .expect("dialog should open");
        let geometries = [OverlayGeometry::new(
            OverlayId(1),
            Rect::new(100.0, 100.0, 300.0, 240.0),
        )];

        let frame = build_overlay_frame(
            &stack,
            OverlayFrameInput::new(Rect::new(0.0, 0.0, 400.0, 300.0), &geometries),
        );

        assert_eq!(frame.underlays.len(), 1, "one underlay should be emitted");
        assert!(
            frame.underlays[0].blocks_pointer,
            "blocking modality should block pointer input",
        );
        assert!(
            frame.underlays[0].z < frame.overlays[0].z,
            "underlay should sit below owner",
        );
    }

    #[test]
    fn dismiss_region_includes_descendant_and_anchor_rects() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open {
                entry: entry(1, OverlayLayer::Menu, OverlayBehavior::Menu),
            })
            .expect("menu should open");
        stack
            .apply(OverlayOp::Open {
                entry: entry(2, OverlayLayer::Menu, OverlayBehavior::Submenu)
                    .with_parent(OverlayId(1)),
            })
            .expect("submenu should open");
        let geometries = [
            OverlayGeometry::new(OverlayId(1), Rect::new(0.0, 0.0, 80.0, 80.0))
                .with_anchor_rect(Rect::new(0.0, -20.0, 80.0, 0.0)),
            OverlayGeometry::new(OverlayId(2), Rect::new(100.0, 0.0, 180.0, 80.0)),
        ];

        let frame = build_overlay_frame(
            &stack,
            OverlayFrameInput::new(Rect::new(0.0, 0.0, 400.0, 300.0), &geometries)
                .with_pointer(Point::new(90.0, 40.0)),
        );
        let region = frame
            .dismiss_regions
            .iter()
            .find(|region| region.overlay == OverlayId(1))
            .expect("parent dismiss region should exist");

        assert_eq!(
            region.inside.len(),
            3,
            "parent region should include parent, anchor, and child rects",
        );
        assert_eq!(
            frame.grace_regions.len(),
            1,
            "menu to submenu should emit a grace region",
        );
        assert!(
            frame
                .hit_regions
                .iter()
                .any(|region| region.kind == OverlayHitKind::GraceRegion),
            "grace hit region should be exposed",
        );
    }

    #[test]
    fn disabled_grace_policy_suppresses_grace_regions() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open {
                entry: entry(1, OverlayLayer::Menu, OverlayBehavior::Menu),
            })
            .expect("menu should open");
        stack
            .apply(OverlayOp::Open {
                entry: entry(2, OverlayLayer::Menu, OverlayBehavior::Submenu)
                    .with_parent(OverlayId(1)),
            })
            .expect("submenu should open");
        let geometries = [
            OverlayGeometry::new(OverlayId(1), Rect::new(0.0, 0.0, 80.0, 80.0)),
            OverlayGeometry::new(OverlayId(2), Rect::new(100.0, 0.0, 180.0, 80.0)),
        ];

        let frame = build_overlay_frame(
            &stack,
            OverlayFrameInput::new(Rect::new(0.0, 0.0, 400.0, 300.0), &geometries)
                .with_pointer(Point::new(90.0, 40.0))
                .with_grace_policy(GracePolicy {
                    enabled: false,
                    ..GracePolicy::default()
                }),
        );

        assert!(
            frame.grace_regions.is_empty(),
            "disabled grace policy should suppress grace regions",
        );
    }

    #[test]
    fn anchor_frame_geometry_uses_resolved_overlay_rect() {
        let anchor_frame = resolve_anchor(
            AnchorInput {
                anchor: Anchor::Rect(Rect::new(100.0, 100.0, 140.0, 120.0)),
                floating_size: Size::new(80.0, 40.0),
                viewport: Rect::new(0.0, 0.0, 400.0, 300.0),
                boundary: Rect::new(0.0, 0.0, 400.0, 300.0),
                previous: None,
            },
            AnchorPolicy::placement(Placement::BOTTOM),
        );

        let geometry =
            OverlayGeometry::new(OverlayId(1), Rect::ZERO).with_anchor_frame(&anchor_frame);

        assert_eq!(
            geometry.rect, anchor_frame.rect,
            "anchor-frame geometry should use the resolved overlay rect",
        );
        assert_eq!(
            geometry.resolved_anchor_rect(),
            Some(anchor_frame.reference_rect.abs()),
            "anchor-frame geometry should expose the resolved reference rect",
        );
    }

    #[test]
    fn focus_policy_emits_scope() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open {
                entry: entry(1, OverlayLayer::Modal, OverlayBehavior::Dialog)
                    .with_focus(FocusPolicy::ContainAndRestore),
            })
            .expect("dialog should open");
        let geometries = [OverlayGeometry::new(
            OverlayId(1),
            Rect::new(100.0, 100.0, 300.0, 240.0),
        )];

        let frame = build_overlay_frame(
            &stack,
            OverlayFrameInput::new(Rect::new(0.0, 0.0, 400.0, 300.0), &geometries),
        );

        assert_eq!(frame.focus_scopes.len(), 1, "one focus scope expected");
        assert!(
            frame.focus_scopes[0].contain,
            "dialog focus scope should contain focus",
        );
        assert!(
            frame.focus_scopes[0].restore_on_close,
            "dialog focus scope should restore focus",
        );
    }
}
