// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use crate::{AnchorId, FocusScopeId, OverlayEvent, OverlayEventResult, OverlayFrame, OverlayId};
use crate::{Revision, resolve_event};

/// Semantic layer for an overlay.
///
/// Store this in [`OverlayEntry::layer`]. [`build_overlay_frame`](crate::build_overlay_frame)
/// uses it for deterministic z-order, while rendering remains entirely owned
/// by the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OverlayLayer {
    /// Lightweight tooltip layer.
    Tooltip,
    /// Popover and hover-card layer.
    Popover,
    /// Menu, submenu, context-menu, and combobox-popup layer.
    Menu,
    /// Modal dialog layer.
    Modal,
    /// System-level overlay layer above ordinary UI overlays.
    System,
}

impl OverlayLayer {
    pub(crate) fn rank(self) -> u8 {
        match self {
            Self::Tooltip => 0,
            Self::Popover => 1,
            Self::Menu => 2,
            Self::Modal => 3,
            Self::System => 4,
        }
    }
}

/// Pointer blocking behavior for an overlay.
///
/// Store this in [`OverlayEntry::modality`]. Frame derivation turns modal and
/// blocking overlays into [`UnderlayFrame`](crate::UnderlayFrame) values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Modality {
    /// Overlay does not create an underlay.
    #[default]
    NonModal,
    /// Overlay creates a non-blocking underlay for visual modality.
    Modal,
    /// Overlay creates an underlay that should block pointer input behind it.
    Blocking,
}

/// Behavioral class for an overlay.
///
/// Store this in [`OverlayEntry::behavior`]. It chooses conservative default
/// dismissal and focus policies in [`OverlayEntry::new`], and it lets frame
/// building derive hover grace between menus and submenus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OverlayBehavior {
    /// Tooltip anchored to a target.
    Tooltip,
    /// Hover card anchored to a target.
    HoverCard,
    /// General popover anchored to a target.
    Popover,
    /// Menu surface.
    Menu,
    /// Child submenu surface.
    Submenu,
    /// Context menu surface.
    ContextMenu,
    /// Combobox popup surface.
    ComboboxPopup,
    /// Dialog surface.
    Dialog,
}

impl OverlayBehavior {
    /// Returns the default dismiss policy for this behavior.
    #[must_use]
    pub fn dismiss_policy(self) -> DismissPolicy {
        match self {
            Self::Tooltip => DismissPolicy {
                anchor_blur: true,
                ..DismissPolicy::default()
            },
            Self::HoverCard => DismissPolicy {
                pointer_down_outside: true,
                anchor_blur: true,
                ..DismissPolicy::default()
            },
            Self::Popover
            | Self::Menu
            | Self::Submenu
            | Self::ContextMenu
            | Self::ComboboxPopup => DismissPolicy {
                escape_key: true,
                pointer_down_outside: true,
                focus_outside: true,
                anchor_blur: true,
                close_descendants_on_parent_close: true,
                pointer_up_outside: false,
            },
            Self::Dialog => DismissPolicy {
                escape_key: true,
                close_descendants_on_parent_close: true,
                ..DismissPolicy::default()
            },
        }
    }

    /// Returns the default focus policy for this behavior.
    #[must_use]
    pub fn focus_policy(self) -> FocusPolicy {
        match self {
            Self::Menu | Self::Submenu | Self::ContextMenu | Self::ComboboxPopup => {
                FocusPolicy::Contain
            }
            Self::Dialog => FocusPolicy::ContainAndRestore,
            Self::Tooltip | Self::HoverCard | Self::Popover => FocusPolicy::None,
        }
    }

    pub(crate) fn supports_grace_to_child(self) -> bool {
        matches!(self, Self::Menu | Self::Submenu | Self::ContextMenu)
    }

    pub(crate) fn supports_grace_from_parent(self) -> bool {
        matches!(self, Self::Submenu)
    }
}

/// Dismissal triggers for an overlay.
///
/// Store this in [`OverlayEntry::dismiss`]. Event resolution maps matching
/// [`OverlayEvent`] values to close operations; hosts still decide when to
/// apply those operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DismissPolicy {
    /// Whether Escape should close this overlay.
    pub escape_key: bool,
    /// Whether pointer down outside should close this overlay.
    pub pointer_down_outside: bool,
    /// Whether pointer up outside should close this overlay.
    pub pointer_up_outside: bool,
    /// Whether focus moving outside should close this overlay.
    pub focus_outside: bool,
    /// Whether blur of the attached [`AnchorId`] should close this overlay.
    pub anchor_blur: bool,
    /// Whether closing this overlay through [`OverlayOp::Close`] also closes
    /// descendants.
    pub close_descendants_on_parent_close: bool,
}

impl Default for DismissPolicy {
    fn default() -> Self {
        Self {
            escape_key: false,
            pointer_down_outside: false,
            pointer_up_outside: false,
            focus_outside: false,
            anchor_blur: false,
            close_descendants_on_parent_close: true,
        }
    }
}

/// Focus behavior for an overlay.
///
/// Store this in [`OverlayEntry::focus`]. Frame derivation turns containing
/// policies into [`FocusScopeFrame`](crate::FocusScopeFrame) values. This
/// crate only describes focus scopes; the host still moves real focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FocusPolicy {
    /// No focus scope metadata.
    None,
    /// Host should restore the previous focus when the overlay closes.
    RestoreOnClose,
    /// Host should contain focus inside the overlay while it is open.
    Contain,
    /// Host should contain focus and restore previous focus on close.
    ContainAndRestore,
}

impl FocusPolicy {
    pub(crate) fn contain(self) -> bool {
        matches!(self, Self::Contain | Self::ContainAndRestore)
    }

    pub(crate) fn restore_on_close(self) -> bool {
        matches!(self, Self::RestoreOnClose | Self::ContainAndRestore)
    }

    pub(crate) fn scope_id(self, overlay: OverlayId) -> Option<FocusScopeId> {
        if matches!(self, Self::None) {
            None
        } else {
            Some(FocusScopeId(overlay.0))
        }
    }
}

/// One open overlay in an [`OverlayStack`].
///
/// Hosts open overlays by applying [`OverlayOp::Open`] with an entry. The entry
/// stores semantic relationships and policies; geometry is supplied later
/// through [`OverlayGeometry`](crate::OverlayGeometry) during frame building.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OverlayEntry {
    /// Stable caller-owned overlay id.
    pub id: OverlayId,
    /// Optional parent overlay id for nested popups and submenus.
    pub parent: Option<OverlayId>,
    /// Optional anchor id used by anchor blur and focus resolution.
    pub anchor: Option<AnchorId>,
    /// Semantic layer used for deterministic z-order.
    pub layer: OverlayLayer,
    /// Whether the overlay creates and blocks through an underlay.
    pub modality: Modality,
    /// Dismissal policy used by event resolution.
    pub dismiss: DismissPolicy,
    /// Focus scope policy exposed in derived frames.
    pub focus: FocusPolicy,
    /// Behavioral class for defaults and grace-region derivation.
    pub behavior: OverlayBehavior,
}

impl OverlayEntry {
    /// Creates an overlay entry with behavior-appropriate default policies.
    #[must_use]
    pub fn new(id: OverlayId, layer: OverlayLayer, behavior: OverlayBehavior) -> Self {
        Self {
            id,
            parent: None,
            anchor: None,
            layer,
            modality: Modality::NonModal,
            dismiss: behavior.dismiss_policy(),
            focus: behavior.focus_policy(),
            behavior,
        }
    }

    /// Sets the parent overlay id.
    #[must_use]
    pub fn with_parent(mut self, parent: OverlayId) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Sets the anchor id.
    #[must_use]
    pub fn with_anchor(mut self, anchor: AnchorId) -> Self {
        self.anchor = Some(anchor);
        self
    }

    /// Sets overlay modality.
    #[must_use]
    pub fn with_modality(mut self, modality: Modality) -> Self {
        self.modality = modality;
        self
    }

    /// Replaces the default dismiss policy.
    #[must_use]
    pub fn with_dismiss(mut self, dismiss: DismissPolicy) -> Self {
        self.dismiss = dismiss;
        self
    }

    /// Replaces the default focus policy.
    #[must_use]
    pub fn with_focus(mut self, focus: FocusPolicy) -> Self {
        self.focus = focus;
        self
    }
}

/// Mutating operation for an [`OverlayStack`].
///
/// Event resolution returns these in [`OverlayEventResult::ops`]. Hosts can
/// inspect, filter, or apply them with [`OverlayStack::apply`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OverlayOp {
    /// Open a new overlay.
    Open {
        /// Entry to insert.
        entry: OverlayEntry,
    },
    /// Close one overlay, respecting its descendant-close policy.
    Close {
        /// Overlay to close.
        overlay: OverlayId,
    },
    /// Close an overlay and all descendants.
    CloseSubtree {
        /// Root overlay to close.
        overlay: OverlayId,
    },
    /// Close only descendants of an overlay.
    CloseDescendants {
        /// Parent whose descendants should close.
        overlay: OverlayId,
    },
    /// Close all overlays in a layer.
    CloseLayer {
        /// Layer to close.
        layer: OverlayLayer,
    },
    /// Close every overlay.
    CloseAll,
    /// Move an overlay to the front of its layer among unrelated overlays.
    BringToFront {
        /// Overlay to move.
        overlay: OverlayId,
    },
    /// Change an overlay's parent relationship.
    SetParent {
        /// Overlay to reparent.
        overlay: OverlayId,
        /// New parent, or `None` to detach.
        parent: Option<OverlayId>,
    },
}

/// Error returned when an [`OverlayOp`] cannot be applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OverlayError {
    /// Operation referenced an overlay that is not open.
    InvalidOverlayId,
    /// Operation referenced a parent that is not open or cannot parent the overlay.
    InvalidParent,
    /// Operation would create a parent cycle.
    Cycle,
    /// Operation would insert an id that is already open.
    DuplicateOverlayId,
}

/// Deterministic collection of currently open overlays.
///
/// Hosts mutate the stack with [`OverlayOp`] values and derive render/event
/// metadata with [`build_overlay_frame`](crate::build_overlay_frame). The
/// entries are stored in stable open/order-front order; layer and parent
/// relationships are interpreted during frame building.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct OverlayStack {
    /// Revision advanced after every successfully applied operation.
    revision: Revision,
    /// Open entries in deterministic stack order.
    entries: Vec<OverlayEntry>,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for OverlayStack {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct WireOverlayStack {
            revision: Revision,
            entries: Vec<OverlayEntry>,
        }

        let wire = WireOverlayStack::deserialize(deserializer)?;
        validate_entries(&wire.entries).map_err(|error| {
            serde::de::Error::custom(match error {
                OverlayError::InvalidOverlayId => "overlay stack contains an invalid overlay id",
                OverlayError::InvalidParent => "overlay stack contains an invalid parent",
                OverlayError::Cycle => "overlay stack contains a parent cycle",
                OverlayError::DuplicateOverlayId => "overlay stack contains a duplicate overlay id",
            })
        })?;
        Ok(Self {
            revision: wire.revision,
            entries: wire.entries,
        })
    }
}

impl OverlayStack {
    /// Creates an empty overlay stack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current stack revision.
    #[must_use]
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// Returns all open entries in stack order.
    #[must_use]
    pub fn entries(&self) -> &[OverlayEntry] {
        &self.entries
    }

    /// Returns an entry by id.
    #[must_use]
    pub fn entry(&self, overlay: OverlayId) -> Option<&OverlayEntry> {
        self.entries.iter().find(|entry| entry.id == overlay)
    }

    /// Returns the parent id of an overlay.
    #[must_use]
    pub fn parent_of(&self, overlay: OverlayId) -> Option<OverlayId> {
        self.entry(overlay).and_then(|entry| entry.parent)
    }

    /// Returns direct child entries of an overlay in stack order.
    pub fn children_of(&self, overlay: OverlayId) -> impl Iterator<Item = &OverlayEntry> + '_ {
        self.entries
            .iter()
            .filter(move |entry| entry.parent == Some(overlay))
    }

    /// Returns descendant ids of an overlay in stack order.
    #[must_use]
    pub fn descendants_of(&self, overlay: OverlayId) -> Vec<OverlayId> {
        let mut descendants = Vec::new();
        for entry in &self.entries {
            if self.is_descendant_of(entry.id, overlay) {
                descendants.push(entry.id);
            }
        }
        descendants
    }

    /// Returns whether `overlay` is a descendant of `ancestor`.
    #[must_use]
    pub fn is_descendant_of(&self, overlay: OverlayId, ancestor: OverlayId) -> bool {
        let mut parent = self.parent_of(overlay);
        for _ in 0..self.entries.len() {
            let Some(current) = parent else {
                return false;
            };
            if current == ancestor {
                return true;
            }
            parent = self.parent_of(current);
        }
        false
    }

    /// Applies one operation and advances the revision on success.
    pub fn apply(&mut self, op: OverlayOp) -> Result<(), OverlayError> {
        match op {
            OverlayOp::Open { entry } => self.open(entry)?,
            OverlayOp::Close { overlay } => self.close(overlay)?,
            OverlayOp::CloseSubtree { overlay } => self.close_subtree(overlay)?,
            OverlayOp::CloseDescendants { overlay } => self.close_descendants(overlay)?,
            OverlayOp::CloseLayer { layer } => self.close_layer(layer),
            OverlayOp::CloseAll => self.entries.clear(),
            OverlayOp::BringToFront { overlay } => self.bring_to_front(overlay)?,
            OverlayOp::SetParent { overlay, parent } => self.set_parent(overlay, parent)?,
        }
        self.revision.advance();
        Ok(())
    }

    /// Resolves an event, applies the produced operations, and returns them.
    pub fn handle_event(
        &mut self,
        frame: &OverlayFrame,
        event: OverlayEvent,
    ) -> Result<OverlayEventResult, OverlayError> {
        let result = resolve_event(self, frame, event);
        for op in result.ops.iter().cloned() {
            self.apply(op)?;
        }
        Ok(result)
    }

    fn open(&mut self, entry: OverlayEntry) -> Result<(), OverlayError> {
        if self.entry(entry.id).is_some() {
            return Err(OverlayError::DuplicateOverlayId);
        }
        match entry.parent {
            Some(parent) if parent == entry.id || self.entry(parent).is_none() => {
                return Err(OverlayError::InvalidParent);
            }
            _ => {}
        }
        self.entries.push(entry);
        Ok(())
    }

    fn close(&mut self, overlay: OverlayId) -> Result<(), OverlayError> {
        let Some(index) = self.index_of(overlay) else {
            return Err(OverlayError::InvalidOverlayId);
        };
        if self.entries[index]
            .dismiss
            .close_descendants_on_parent_close
        {
            self.close_subtree(overlay)
        } else {
            self.entries.remove(index);
            for entry in &mut self.entries {
                if entry.parent == Some(overlay) {
                    entry.parent = None;
                }
            }
            Ok(())
        }
    }

    fn close_subtree(&mut self, overlay: OverlayId) -> Result<(), OverlayError> {
        if self.entry(overlay).is_none() {
            return Err(OverlayError::InvalidOverlayId);
        }
        let mut removed = self.descendants_of(overlay);
        removed.push(overlay);
        self.entries.retain(|entry| !removed.contains(&entry.id));
        Ok(())
    }

    fn close_descendants(&mut self, overlay: OverlayId) -> Result<(), OverlayError> {
        if self.entry(overlay).is_none() {
            return Err(OverlayError::InvalidOverlayId);
        }
        let removed = self.descendants_of(overlay);
        self.entries.retain(|entry| !removed.contains(&entry.id));
        Ok(())
    }

    fn close_layer(&mut self, layer: OverlayLayer) {
        let roots: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| entry.layer == layer)
            .map(|entry| entry.id)
            .collect();
        let mut removed = Vec::new();
        for root in roots {
            removed.push(root);
            removed.extend(self.descendants_of(root));
        }
        self.entries.retain(|entry| !removed.contains(&entry.id));
    }

    fn bring_to_front(&mut self, overlay: OverlayId) -> Result<(), OverlayError> {
        let Some(index) = self.index_of(overlay) else {
            return Err(OverlayError::InvalidOverlayId);
        };
        let entry = self.entries.remove(index);
        self.entries.push(entry);
        Ok(())
    }

    fn set_parent(
        &mut self,
        overlay: OverlayId,
        parent: Option<OverlayId>,
    ) -> Result<(), OverlayError> {
        let Some(index) = self.index_of(overlay) else {
            return Err(OverlayError::InvalidOverlayId);
        };
        if parent == Some(overlay) {
            return Err(OverlayError::Cycle);
        }
        if let Some(parent) = parent {
            if self.entry(parent).is_none() {
                return Err(OverlayError::InvalidParent);
            }
            if self.is_descendant_of(parent, overlay) {
                return Err(OverlayError::Cycle);
            }
        }
        self.entries[index].parent = parent;
        Ok(())
    }

    fn index_of(&self, overlay: OverlayId) -> Option<usize> {
        self.entries.iter().position(|entry| entry.id == overlay)
    }
}

#[cfg(any(test, feature = "serde"))]
fn validate_entries(entries: &[OverlayEntry]) -> Result<(), OverlayError> {
    for (index, entry) in entries.iter().enumerate() {
        if entries[..index]
            .iter()
            .any(|previous| previous.id == entry.id)
        {
            return Err(OverlayError::DuplicateOverlayId);
        }
    }

    for entry in entries {
        if let Some(parent) = entry.parent {
            if parent == entry.id {
                return Err(OverlayError::Cycle);
            }
            if !entries.iter().any(|candidate| candidate.id == parent) {
                return Err(OverlayError::InvalidParent);
            }
        }
    }

    for entry in entries {
        let mut parent = entry.parent;
        for _ in 0..entries.len() {
            let Some(current) = parent else {
                break;
            };
            parent = entries
                .iter()
                .find(|candidate| candidate.id == current)
                .and_then(|candidate| candidate.parent);
        }
        if parent.is_some() {
            return Err(OverlayError::Cycle);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        DismissPolicy, Modality, OverlayBehavior, OverlayEntry, OverlayError, OverlayLayer,
        OverlayOp, OverlayStack, validate_entries,
    };
    use crate::OverlayId;

    fn entry(id: u64) -> OverlayEntry {
        OverlayEntry::new(
            OverlayId(id),
            OverlayLayer::Popover,
            OverlayBehavior::Popover,
        )
    }

    #[test]
    fn open_rejects_duplicate_ids() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open { entry: entry(1) })
            .expect("first open should succeed");

        let error = stack
            .apply(OverlayOp::Open { entry: entry(1) })
            .expect_err("duplicate open should fail");

        assert_eq!(
            error,
            OverlayError::DuplicateOverlayId,
            "duplicate id should be rejected",
        );
        assert_eq!(stack.revision().0, 1, "failed operation should not revise",);
    }

    #[test]
    fn close_subtree_removes_descendants() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open { entry: entry(1) })
            .expect("parent should open");
        stack
            .apply(OverlayOp::Open {
                entry: entry(2).with_parent(OverlayId(1)),
            })
            .expect("child should open");
        stack
            .apply(OverlayOp::Open {
                entry: entry(3).with_parent(OverlayId(2)),
            })
            .expect("grandchild should open");

        stack
            .apply(OverlayOp::CloseSubtree {
                overlay: OverlayId(1),
            })
            .expect("subtree close should succeed");

        assert!(
            stack.entries().is_empty(),
            "subtree close should remove all descendants",
        );
    }

    #[test]
    fn close_can_detach_children_when_policy_allows() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open {
                entry: entry(1).with_dismiss(DismissPolicy {
                    close_descendants_on_parent_close: false,
                    ..DismissPolicy::default()
                }),
            })
            .expect("parent should open");
        stack
            .apply(OverlayOp::Open {
                entry: entry(2).with_parent(OverlayId(1)),
            })
            .expect("child should open");

        stack
            .apply(OverlayOp::Close {
                overlay: OverlayId(1),
            })
            .expect("close should succeed");

        assert_eq!(stack.entries().len(), 1, "child should remain open");
        assert_eq!(
            stack.entries()[0].parent,
            None,
            "remaining child should be detached",
        );
    }

    #[test]
    fn set_parent_rejects_cycles() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open { entry: entry(1) })
            .expect("parent should open");
        stack
            .apply(OverlayOp::Open {
                entry: entry(2).with_parent(OverlayId(1)),
            })
            .expect("child should open");

        let error = stack
            .apply(OverlayOp::SetParent {
                overlay: OverlayId(1),
                parent: Some(OverlayId(2)),
            })
            .expect_err("cycle should fail");

        assert_eq!(error, OverlayError::Cycle, "cycle should be rejected");
    }

    #[test]
    fn bring_to_front_changes_entry_order() {
        let mut stack = OverlayStack::new();
        stack
            .apply(OverlayOp::Open { entry: entry(1) })
            .expect("first should open");
        stack
            .apply(OverlayOp::Open {
                entry: entry(2).with_modality(Modality::Modal),
            })
            .expect("second should open");

        stack
            .apply(OverlayOp::BringToFront {
                overlay: OverlayId(1),
            })
            .expect("bring to front should succeed");

        assert_eq!(
            stack.entries()[1].id,
            OverlayId(1),
            "overlay should move to the end of stack order",
        );
    }

    #[test]
    fn validate_entries_rejects_duplicate_ids() {
        let entries = alloc::vec![entry(1), entry(1)];

        let error = validate_entries(&entries).expect_err("duplicate ids should fail");

        assert_eq!(
            error,
            OverlayError::DuplicateOverlayId,
            "validation should reject duplicate overlay ids",
        );
    }

    #[test]
    fn validate_entries_rejects_missing_parents() {
        let entries = alloc::vec![entry(1).with_parent(OverlayId(99))];

        let error = validate_entries(&entries).expect_err("missing parent should fail");

        assert_eq!(
            error,
            OverlayError::InvalidParent,
            "validation should reject missing parents",
        );
    }

    #[test]
    fn validate_entries_rejects_parent_cycles() {
        let entries = alloc::vec![
            entry(1).with_parent(OverlayId(2)),
            entry(2).with_parent(OverlayId(1)),
        ];

        let error = validate_entries(&entries).expect_err("cycle should fail");

        assert_eq!(
            error,
            OverlayError::Cycle,
            "validation should reject parent cycles",
        );
    }

    #[test]
    fn invalid_stack_descendant_query_is_bounded() {
        let stack = OverlayStack {
            revision: crate::Revision(0),
            entries: alloc::vec![
                entry(1).with_parent(OverlayId(2)),
                entry(2).with_parent(OverlayId(1)),
            ],
        };

        assert!(
            !stack.is_descendant_of(OverlayId(1), OverlayId(3)),
            "invalid cycles should not make descendant queries loop",
        );
    }
}
