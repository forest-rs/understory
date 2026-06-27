// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use crate::interaction::op_for_dock_proposal;
use crate::util::{major_length, major_size, split_min_major};
use crate::{
    Axis, DockProposal, DockTarget, LayoutFrame, Placement, Proposal, ResizeOptions,
    ResizeProposal, TileError, TileNode, TileOp, TileTree,
};

/// Pane behavior capabilities.
///
/// Store this in [`DockPolicyData`] when validating proposals. Current
/// validation uses the move, tab, split, edge, and zone fields for supported
/// dock proposals; close, float, and pin capabilities are reserved for
/// operations that do not yet commit successfully.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaneCapabilities {
    /// Whether pane close operations are allowed.
    ///
    /// Reserved for close-policy validation.
    pub closable: bool,
    /// Whether pane move operations are allowed.
    pub movable: bool,
    /// Whether pane float operations are allowed.
    ///
    /// Reserved until floating surfaces can be committed.
    pub floatable: bool,
    /// Whether pane pinning is allowed by future APIs.
    pub pinnable: bool,
    /// Whether the pane may join tab groups.
    pub tabbable: bool,
    /// Whether the pane may be used as a split target.
    pub split_target: bool,
    /// Edges allowed for edge docking.
    pub allowed_edges: EdgeSet,
    /// Zones allowed for docking.
    pub allowed_zones: ZoneSet,
}

/// Bitset of allowed dock edges.
///
/// Use these constants in [`PaneCapabilities::allowed_edges`] to describe which
/// edge split targets a pane may dock into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EdgeSet(
    /// Edge bitset.
    pub u8,
);

/// Bitset of allowed dock zones.
///
/// Use these constants in [`PaneCapabilities::allowed_zones`] to describe which
/// broad docking zones a pane may use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ZoneSet(
    /// Zone bitset.
    pub u8,
);

/// Data-driven docking policy.
///
/// Construct this and pass it to [`validate_proposal`] before committing a drag,
/// resize, or command-generated proposal. The core uses data rather than
/// callbacks so validation remains deterministic and testable.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DockPolicyData {
    /// Default capabilities used by proposal validation.
    ///
    /// Per-pane policy data is not modeled yet, so these capabilities apply to
    /// every validated proposal.
    pub default_pane_capabilities: PaneCapabilities,
    /// Whether the layout is locked against mutation.
    pub locked_layout: bool,
}

/// A proposal paired with the operation it lowers to.
///
/// Returned by [`validate_proposal`]. Pass it to [`commit_proposal`] to apply
/// the already-validated operation without re-lowering the proposal.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ValidatedProposal {
    /// Original proposal.
    pub proposal: Proposal,
    /// Operation to commit.
    pub op: TileOp,
}

/// Inputs for proposal validation.
///
/// Construct this with [`ProposalValidationInput::new`] before passing it to
/// [`validate_proposal`]. Dock proposals only need a tree and policy. Resize
/// proposals should additionally use [`ProposalValidationInput::with_frame`] so
/// validation can check the proposed shares against the solved split geometry
/// that produced the active resize interaction.
#[derive(Clone, Debug)]
pub struct ProposalValidationInput<'a> {
    /// Tree the proposal applies to.
    pub tree: &'a TileTree,
    /// Proposal to validate.
    pub proposal: Proposal,
    /// Policy data used for capability checks.
    pub policy: &'a DockPolicyData,
    /// Solved frame for geometry-sensitive validation.
    pub frame: Option<&'a LayoutFrame>,
    /// Resize geometry constraints.
    pub resize_options: ResizeOptions,
}

impl Default for PaneCapabilities {
    fn default() -> Self {
        Self {
            closable: true,
            movable: true,
            floatable: true,
            pinnable: true,
            tabbable: true,
            split_target: true,
            allowed_edges: EdgeSet::ALL,
            allowed_zones: ZoneSet::ALL,
        }
    }
}

impl<'a> ProposalValidationInput<'a> {
    /// Creates validation input for dock-like proposals.
    ///
    /// Use [`ProposalValidationInput::with_frame`] before validating resize
    /// proposals that came from pointer interaction.
    #[must_use]
    pub fn new(tree: &'a TileTree, proposal: Proposal, policy: &'a DockPolicyData) -> Self {
        Self {
            tree,
            proposal,
            policy,
            frame: None,
            resize_options: ResizeOptions::default(),
        }
    }

    /// Adds a solved frame for geometry-sensitive validation.
    ///
    /// Call this for resize proposals so validation can reject shares that
    /// would move a solved split handle past the configured minimum pane size.
    #[must_use]
    pub fn with_frame(mut self, frame: &'a LayoutFrame) -> Self {
        self.frame = Some(frame);
        self
    }

    /// Adds resize options for geometry-sensitive validation.
    ///
    /// Use this with [`ProposalValidationInput::with_frame`] when the host uses
    /// resize constraints other than [`ResizeOptions::default`].
    #[must_use]
    pub const fn with_resize_options(mut self, options: ResizeOptions) -> Self {
        self.resize_options = options;
        self
    }
}

impl EdgeSet {
    /// Left edge.
    pub const LEFT: Self = Self(0b0001);
    /// Right edge.
    pub const RIGHT: Self = Self(0b0010);
    /// Top edge.
    pub const TOP: Self = Self(0b0100);
    /// Bottom edge.
    pub const BOTTOM: Self = Self(0b1000);
    /// All edges.
    pub const ALL: Self = Self(0b1111);

    /// Returns whether every edge in `other` is present.
    ///
    /// Use this when validating generated [`DockTarget::Split`] targets against
    /// [`PaneCapabilities::allowed_edges`].
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl ZoneSet {
    /// Split zone.
    pub const SPLIT: Self = Self(0b0001);
    /// Tab-into zone.
    pub const TAB: Self = Self(0b0010);
    /// Floating zone.
    pub const FLOAT: Self = Self(0b0100);
    /// All zones.
    pub const ALL: Self = Self(0b0111);

    /// Returns whether every zone in `other` is present.
    ///
    /// Use this when validating a proposal against
    /// [`PaneCapabilities::allowed_zones`].
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// Validates a proposal and lowers it to a semantic operation.
///
/// Pass proposals from [`update_drag`](crate::update_drag) or
/// [`update_resize`](crate::update_resize) here before committing them when the
/// host wants data-driven policy checks. This function rejects locked layouts,
/// capability-disallowed targets, and structurally invalid operations by
/// applying the lowered operation to a clone of the tree. The live tree is not
/// mutated.
pub fn validate_proposal(
    input: ProposalValidationInput<'_>,
) -> Result<ValidatedProposal, TileError> {
    let ProposalValidationInput {
        tree,
        proposal,
        policy,
        frame,
        resize_options,
    } = input;
    if policy.locked_layout {
        return Err(TileError::PolicyRejected);
    }
    let op = match proposal.clone() {
        Proposal::Dock(dock) => op_for_dock_proposal(dock)?,
        Proposal::Resize(resize) => {
            let frame = frame.ok_or(TileError::InvalidOperation)?;
            validate_resize_geometry(tree, frame, &resize, &resize_options)?;
            TileOp::SetSplitShares {
                split: resize.split,
                shares: resize.new_shares,
            }
        }
    };
    let mut probe = tree.clone();
    probe.apply(op.clone())?;
    validate_policy(&proposal, policy)?;
    Ok(ValidatedProposal { proposal, op })
}

/// Commits a validated proposal.
///
/// Pass the value returned by [`validate_proposal`] here when the tree is still
/// expected to accept the operation. Policy is not re-run; the tree may still
/// reject the operation if the caller changed it after validation.
pub fn commit_proposal(
    tree: &mut TileTree,
    proposal: ValidatedProposal,
) -> Result<TileOp, TileError> {
    tree.apply(proposal.op.clone())?;
    Ok(proposal.op)
}

fn validate_policy(proposal: &Proposal, policy: &DockPolicyData) -> Result<(), TileError> {
    match proposal {
        Proposal::Dock(dock) => validate_dock_policy(dock, &policy.default_pane_capabilities),
        Proposal::Resize(_) => Ok(()),
    }
}

fn validate_dock_policy(
    proposal: &DockProposal,
    capabilities: &PaneCapabilities,
) -> Result<(), TileError> {
    match proposal {
        DockProposal::MovePane { target, .. } => {
            require(capabilities.movable)?;
            validate_target_policy(*target, capabilities)
        }
        DockProposal::MoveTabGroup { target, .. } => validate_target_policy(*target, capabilities),
        DockProposal::ReorderTab { .. } => require(capabilities.movable && capabilities.tabbable),
        DockProposal::FloatPane { .. } => {
            require(capabilities.floatable && capabilities.allowed_zones.contains(ZoneSet::FLOAT))
        }
    }
}

fn validate_target_policy(
    target: DockTarget,
    capabilities: &PaneCapabilities,
) -> Result<(), TileError> {
    match target {
        DockTarget::Root | DockTarget::Replace { .. } => {
            require(capabilities.movable && capabilities.allowed_zones.contains(ZoneSet::SPLIT))
        }
        DockTarget::Split {
            axis, placement, ..
        } => require(
            capabilities.movable
                && capabilities.split_target
                && capabilities.allowed_zones.contains(ZoneSet::SPLIT)
                && capabilities
                    .allowed_edges
                    .contains(edge_for_split(axis, placement)),
        ),
        DockTarget::TabInto { .. } => require(
            capabilities.movable
                && capabilities.tabbable
                && capabilities.allowed_zones.contains(ZoneSet::TAB),
        ),
        DockTarget::Float { .. } => require(
            capabilities.movable
                && capabilities.floatable
                && capabilities.allowed_zones.contains(ZoneSet::FLOAT),
        ),
    }
}

fn edge_for_split(axis: Axis, placement: Placement) -> EdgeSet {
    match (axis, placement) {
        (Axis::Horizontal, Placement::Before) => EdgeSet::LEFT,
        (Axis::Horizontal, Placement::After) => EdgeSet::RIGHT,
        (Axis::Vertical, Placement::Before) => EdgeSet::TOP,
        (Axis::Vertical, Placement::After) => EdgeSet::BOTTOM,
    }
}

fn require(allowed: bool) -> Result<(), TileError> {
    if allowed {
        Ok(())
    } else {
        Err(TileError::PolicyRejected)
    }
}

fn validate_resize_geometry(
    tree: &TileTree,
    frame: &LayoutFrame,
    proposal: &ResizeProposal,
    options: &ResizeOptions,
) -> Result<(), TileError> {
    let Some(TileNode::Split(split)) = tree.node(proposal.split) else {
        return Err(TileError::InvalidTileId);
    };
    if proposal.handle + 1 >= split.children.len()
        || proposal.new_shares.len() != split.children.len()
        || proposal
            .new_shares
            .iter()
            .any(|share| !share.is_finite() || *share <= 0.0)
        || !proposal.delta.is_finite()
    {
        return Err(TileError::InvalidOperation);
    }

    let mut children = frame
        .split_children
        .iter()
        .filter(|child| child.split == proposal.split)
        .copied()
        .collect::<Vec<_>>();
    children.sort_by_key(|child| child.index);
    if children.len() != split.children.len() {
        return Err(TileError::InvalidOperation);
    }
    for child in &children {
        if split.children.get(child.index).copied() != Some(child.child) {
            return Err(TileError::InvalidOperation);
        }
    }

    let min_major = split_min_major(
        split.children.len(),
        &split.constraints,
        major_size(options.min_pane_size, split.axis),
    );
    let mut old_total = 0.0;
    let mut new_total = 0.0;
    for child in &children {
        let old = major_length(child.rect, split.axis);
        let new = proposal.new_shares[child.index];
        if !old.is_finite() || old <= 0.0 || new + VALIDATION_EPSILON < min_major[child.index] {
            return Err(TileError::InvalidOperation);
        }
        if child.index != proposal.handle && child.index != proposal.handle + 1 && !near(old, new) {
            return Err(TileError::InvalidOperation);
        }
        old_total += old;
        new_total += new;
    }

    if !near(old_total, new_total) {
        return Err(TileError::InvalidOperation);
    }
    let old_left = major_length(children[proposal.handle].rect, split.axis);
    let old_right = major_length(children[proposal.handle + 1].rect, split.axis);
    let new_left = proposal.new_shares[proposal.handle];
    let new_right = proposal.new_shares[proposal.handle + 1];
    if !near(new_left - old_left, proposal.delta) || !near(old_right - new_right, proposal.delta) {
        return Err(TileError::InvalidOperation);
    }

    Ok(())
}

const VALIDATION_EPSILON: f64 = 1.0e-6;

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= VALIDATION_EPSILON
}
