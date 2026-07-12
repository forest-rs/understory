// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use crate::interaction::op_for_dock_proposal;
use crate::util::{major_length, major_size, split_min_major};
use crate::{
    Axis, DockProposal, DockTarget, InteractionOptions, InteractionUpdate, LayoutFrame, Placement,
    Proposal, ResizeOptions, ResizeProposal, Revision, TileError, TileNode, TileOp, TileTree,
};

/// Global permissions for docking operations.
///
/// Store this in [`DockPolicy`] when validating proposals. These permissions
/// apply uniformly to proposals; application-specific
/// per-pane policy remains outside the renderer-independent core.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DockPermissions {
    /// Whether move operations are allowed.
    pub allow_move: bool,
    /// Whether tab insertion and reordering are allowed.
    pub allow_tab: bool,
    /// Whether split docking is allowed.
    pub allow_split: bool,
    /// Edges allowed for edge docking.
    pub allowed_edges: EdgeSet,
    /// Zones allowed for docking.
    pub allowed_zones: ZoneSet,
}

/// Bitset of allowed dock edges.
///
/// Use these constants in [`DockPermissions::allowed_edges`] to describe which
/// edge split targets a pane may dock into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EdgeSet(
    /// Edge bitset.
    pub u8,
);

/// Bitset of allowed dock zones.
///
/// Use these constants in [`DockPermissions::allowed_zones`] to describe which
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
pub struct DockPolicy {
    /// Global permissions used by proposal validation.
    pub permissions: DockPermissions,
    /// Whether the layout is locked against mutation.
    pub locked_layout: bool,
}

/// A proposal paired with the operation it lowers to.
///
/// Returned by [`validate_proposal`]. Pass it to [`commit_proposal`] to apply
/// the already-validated operation without re-lowering the proposal.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ValidatedProposal {
    /// Original proposal.
    proposal: Proposal,
    /// Operation to commit.
    op: TileOp,
    /// Exact tree revision against which this proposal was validated.
    revision: Revision,
}

impl ValidatedProposal {
    /// Returns the original validated proposal.
    #[must_use]
    pub const fn proposal(&self) -> &Proposal {
        &self.proposal
    }

    /// Returns the semantic operation that will be committed.
    #[must_use]
    pub const fn operation(&self) -> &TileOp {
        &self.op
    }

    /// Returns the exact tree revision against which validation ran.
    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }
}

/// Inputs for proposal validation.
///
/// Construct this with [`ProposalValidationInput::new`] before passing it to
/// [`validate_proposal`]. Dock proposals only need a tree and policy. Resize
/// proposals should additionally use [`ProposalValidationInput::with_frame`] so
/// validation can check proposed major lengths against solved split geometry
/// that produced the active resize interaction. Pointer-generated proposals can
/// also use [`ProposalValidationInput::with_base_revision`] so validation
/// rejects stale interactions.
#[derive(Clone, Debug)]
pub struct ProposalValidationInput<'a> {
    /// Tree the proposal applies to.
    pub tree: &'a TileTree,
    /// Proposal to validate.
    pub proposal: Proposal,
    /// Policy data used for capability checks.
    pub policy: &'a DockPolicy,
    /// Solved frame for geometry-sensitive validation.
    pub frame: Option<&'a LayoutFrame>,
    /// Resize geometry constraints.
    pub resize_options: ResizeOptions,
    /// Expected tree revision for stale interaction checks.
    pub base_revision: Option<Revision>,
}

impl Default for DockPermissions {
    fn default() -> Self {
        Self {
            allow_move: true,
            allow_tab: true,
            allow_split: true,
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
    pub fn new(tree: &'a TileTree, proposal: Proposal, policy: &'a DockPolicy) -> Self {
        Self {
            tree,
            proposal,
            policy,
            frame: None,
            resize_options: ResizeOptions::default(),
            base_revision: None,
        }
    }

    /// Creates validation input from a unified interaction update.
    ///
    /// Use this when committing proposals returned by
    /// [`update_interaction`](crate::update_interaction). Add the current
    /// layout frame with [`ProposalValidationInput::with_frame`] before
    /// validating resize proposals.
    #[must_use]
    pub fn from_interaction_update(
        tree: &'a TileTree,
        update: &InteractionUpdate,
        policy: &'a DockPolicy,
    ) -> Option<Self> {
        Some(Self {
            tree,
            proposal: update.proposal.clone()?,
            policy,
            frame: None,
            resize_options: ResizeOptions::default(),
            base_revision: update.base_revision,
        })
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

    /// Adds resize options from unified interaction options.
    ///
    /// Use this with values passed to
    /// [`update_interaction`](crate::update_interaction) so validation applies
    /// the same resize constraints that produced the proposal.
    #[must_use]
    pub const fn with_interaction_options(mut self, options: &InteractionOptions) -> Self {
        self.resize_options = options.resize;
        self
    }

    /// Adds an expected tree revision for stale interaction checks.
    ///
    /// Pointer interactions should validate against the revision captured at
    /// pointer-down. Command-generated proposals can leave this unset.
    #[must_use]
    pub const fn with_base_revision(mut self, revision: Revision) -> Self {
        self.base_revision = Some(revision);
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
    /// [`DockPermissions::allowed_edges`].
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
    /// All zones.
    pub const ALL: Self = Self(0b0011);

    /// Returns whether every zone in `other` is present.
    ///
    /// Use this when validating a proposal against
    /// [`DockPermissions::allowed_zones`].
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// Validates a proposal and lowers it to a semantic operation.
///
/// Pass command-generated proposals here before committing them when the host
/// wants data-driven policy checks. For pointer interaction, prefer
/// [`validate_interaction_update`], which fills in the solved frame,
/// interaction options, and stale-revision guard. This function rejects locked
/// layouts, capability-disallowed targets, and structurally invalid operations
/// by applying the lowered operation to a clone of the tree. The live tree is
/// not mutated.
pub fn validate_proposal(
    input: ProposalValidationInput<'_>,
) -> Result<ValidatedProposal, TileError> {
    let ProposalValidationInput {
        tree,
        proposal,
        policy,
        frame,
        resize_options,
        base_revision,
    } = input;
    if let Some(base_revision) = base_revision
        && base_revision != tree.revision()
    {
        return Err(TileError::StaleInteraction);
    }
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
                shares: resize.new_major_lengths,
            }
        }
    };
    let mut probe = tree.clone();
    probe.apply(op.clone())?;
    validate_policy(&proposal, policy)?;
    Ok(ValidatedProposal {
        proposal,
        op,
        revision: tree.revision(),
    })
}

/// Validates a unified interaction update.
///
/// Use this with updates returned by [`update_interaction`](crate::update_interaction)
/// on pointer-up. It validates the update's proposal against `frame`,
/// `options`, `policy`, and the interaction revision captured at pointer-down,
/// then lowers it to a [`ValidatedProposal`] for [`commit_proposal`].
pub fn validate_interaction_update(
    tree: &TileTree,
    frame: &LayoutFrame,
    update: &InteractionUpdate,
    policy: &DockPolicy,
    options: &InteractionOptions,
) -> Result<ValidatedProposal, TileError> {
    let input = ProposalValidationInput::from_interaction_update(tree, update, policy)
        .ok_or(TileError::InvalidOperation)?
        .with_frame(frame)
        .with_interaction_options(options);
    validate_proposal(input)
}

/// Commits a validated proposal.
///
/// Pass the value returned by [`validate_proposal`] here to apply it to the
/// exact tree revision against which it was validated. Policy is not re-run;
/// if the tree changed after validation, commit returns
/// [`TileError::StaleInteraction`].
pub fn commit_proposal(
    tree: &mut TileTree,
    proposal: ValidatedProposal,
) -> Result<TileOp, TileError> {
    if tree.revision() != proposal.revision {
        return Err(TileError::StaleInteraction);
    }
    tree.apply(proposal.op.clone())?;
    Ok(proposal.op)
}

fn validate_policy(proposal: &Proposal, policy: &DockPolicy) -> Result<(), TileError> {
    match proposal {
        Proposal::Dock(dock) => validate_dock_policy(dock, &policy.permissions),
        Proposal::Resize(_) => Ok(()),
    }
}

fn validate_dock_policy(
    proposal: &DockProposal,
    capabilities: &DockPermissions,
) -> Result<(), TileError> {
    match proposal {
        DockProposal::MovePane { target, .. } => {
            require(capabilities.allow_move)?;
            validate_target_policy(*target, capabilities)
        }
        DockProposal::MoveTabGroup { target, .. } => {
            require(capabilities.allow_move)?;
            validate_target_policy(*target, capabilities)
        }
        DockProposal::ReorderTab { .. } => {
            require(capabilities.allow_move && capabilities.allow_tab)
        }
    }
}

fn validate_target_policy(
    target: DockTarget,
    capabilities: &DockPermissions,
) -> Result<(), TileError> {
    match target {
        DockTarget::Root => {
            require(capabilities.allow_move && capabilities.allowed_zones.contains(ZoneSet::SPLIT))
        }
        DockTarget::Split {
            axis, placement, ..
        } => require(
            capabilities.allow_move
                && capabilities.allow_split
                && capabilities.allowed_zones.contains(ZoneSet::SPLIT)
                && capabilities
                    .allowed_edges
                    .contains(edge_for_split(axis, placement)),
        ),
        DockTarget::TabInto { .. } => require(
            capabilities.allow_move
                && capabilities.allow_tab
                && capabilities.allowed_zones.contains(ZoneSet::TAB),
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
    if frame.revision() != tree.revision() {
        return Err(TileError::StaleInteraction);
    }
    let Some(TileNode::Split(split)) = tree.node(proposal.split) else {
        return Err(TileError::InvalidTileId);
    };
    if proposal.handle + 1 >= split.children.len()
        || proposal.new_major_lengths.len() != split.children.len()
        || proposal
            .new_major_lengths
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
        let new = proposal.new_major_lengths[child.index];
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
    let new_left = proposal.new_major_lengths[proposal.handle];
    let new_right = proposal.new_major_lengths[proposal.handle + 1];
    if !near(new_left - old_left, proposal.delta) || !near(old_right - new_right, proposal.delta) {
        return Err(TileError::InvalidOperation);
    }

    Ok(())
}

const VALIDATION_EPSILON: f64 = 1.0e-6;

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= VALIDATION_EPSILON
}
