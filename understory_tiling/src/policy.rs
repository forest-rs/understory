// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::interaction::op_for_dock_proposal;
use crate::{Proposal, TileError, TileOp, TileTree};

/// Pane behavior capabilities.
///
/// Store this in [`DockPolicyData`] when validating proposals. The current MVP
/// mostly exposes the shape; future validation can use these flags to accept or
/// reject close, move, tab, split, and float requests.
#[derive(Clone, Copy, Debug)]
pub struct PaneCapabilities {
    /// Whether pane close operations are allowed.
    pub closable: bool,
    /// Whether pane move operations are allowed.
    pub movable: bool,
    /// Whether pane float operations are allowed.
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
pub struct EdgeSet(
    /// Edge bitset.
    pub u8,
);

/// Bitset of allowed dock zones.
///
/// Use these constants in [`PaneCapabilities::allowed_zones`] to describe which
/// broad docking zones a pane may use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
pub struct DockPolicyData {
    /// Default capabilities used for panes without per-pane data.
    pub default_pane_capabilities: PaneCapabilities,
    /// Whether the layout is locked against mutation.
    pub locked_layout: bool,
}

/// A proposal paired with the operation it lowers to.
///
/// Returned by [`validate_proposal`]. Pass it to [`commit_proposal`] to apply
/// the already-validated operation without re-lowering the proposal.
#[derive(Clone, Debug)]
pub struct ValidatedProposal {
    /// Original proposal.
    pub proposal: Proposal,
    /// Operation to commit.
    pub op: TileOp,
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
}

/// Validates a proposal and lowers it to a semantic operation.
pub fn validate_proposal(
    _tree: &TileTree,
    proposal: Proposal,
    policy: &DockPolicyData,
) -> Result<ValidatedProposal, TileError> {
    if policy.locked_layout {
        return Err(TileError::PolicyRejected);
    }
    let op = match proposal.clone() {
        Proposal::Dock(dock) => op_for_dock_proposal(dock)?,
        Proposal::Resize(resize) => TileOp::SetSplitShares {
            split: resize.split,
            shares: resize.new_shares,
        },
    };
    Ok(ValidatedProposal { proposal, op })
}

/// Commits a validated proposal.
pub fn commit_proposal(
    tree: &mut TileTree,
    proposal: ValidatedProposal,
) -> Result<TileOp, TileError> {
    tree.apply(proposal.op.clone())?;
    Ok(proposal.op)
}
