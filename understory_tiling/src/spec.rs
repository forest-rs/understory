// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::fmt;

use crate::{Axis, PaneId, SplitConstraints, TabBarPlacement};

/// Recursive, ID-free description consumed to construct a [`TileTree`](crate::TileTree).
///
/// Use the convenience constructors for common cases, or construct variants
/// directly when split constraints or tab metadata need to be explicit.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TileSpec {
    /// A single pane leaf.
    Pane(PaneId),
    /// A semantic tab group.
    Tabs {
        /// Panes in semantic tab order.
        panes: Vec<PaneId>,
        /// Active tab index.
        active: usize,
        /// Tab bar placement.
        placement: TabBarPlacement,
    },
    /// An ordered split of child specifications.
    Split {
        /// Split axis.
        axis: Axis,
        /// Children in semantic traversal order.
        children: Vec<Self>,
        /// Positive relative share for each child.
        shares: Vec<f64>,
        /// Per-child split constraints.
        constraints: SplitConstraints,
    },
}

impl TileSpec {
    /// Creates a single pane leaf.
    #[must_use]
    pub const fn pane(pane: PaneId) -> Self {
        Self::Pane(pane)
    }

    /// Creates a top-placed tab group with the first pane active.
    pub fn tabs(panes: impl IntoIterator<Item = PaneId>) -> Self {
        Self::Tabs {
            panes: panes.into_iter().collect(),
            active: 0,
            placement: TabBarPlacement::Top,
        }
    }

    /// Creates a tab group with explicit active index and placement.
    pub fn tabs_with(
        panes: impl IntoIterator<Item = PaneId>,
        active: usize,
        placement: TabBarPlacement,
    ) -> Self {
        Self::Tabs {
            panes: panes.into_iter().collect(),
            active,
            placement,
        }
    }

    /// Creates a split with explicit child shares and default constraints.
    pub fn split(
        axis: Axis,
        children: impl IntoIterator<Item = Self>,
        shares: impl IntoIterator<Item = f64>,
    ) -> Self {
        Self::Split {
            axis,
            children: children.into_iter().collect(),
            shares: shares.into_iter().collect(),
            constraints: SplitConstraints::default(),
        }
    }

    /// Creates an equal-share split with default constraints.
    pub fn equal_split(axis: Axis, children: impl IntoIterator<Item = Self>) -> Self {
        let children: Vec<_> = children.into_iter().collect();
        let shares = alloc::vec![1.0; children.len()];
        Self::Split {
            axis,
            children,
            shares,
            constraints: SplitConstraints::default(),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), TileSpecError> {
        validate_spec(self, &mut Vec::new())
    }
}

fn validate_spec(spec: &TileSpec, panes: &mut Vec<PaneId>) -> Result<(), TileSpecError> {
    match spec {
        TileSpec::Pane(pane) => validate_pane(*pane, panes),
        TileSpec::Tabs {
            panes: tab_panes,
            active,
            ..
        } => {
            if tab_panes.is_empty() {
                return Err(TileSpecError::EmptyTabs);
            }
            if *active >= tab_panes.len() {
                return Err(TileSpecError::InvalidActiveTab {
                    active: *active,
                    panes: tab_panes.len(),
                });
            }
            for pane in tab_panes {
                validate_pane(*pane, panes)?;
            }
            Ok(())
        }
        TileSpec::Split {
            children,
            shares,
            constraints,
            ..
        } => {
            if children.len() < 2 {
                return Err(TileSpecError::TooFewSplitChildren {
                    children: children.len(),
                });
            }
            if children.len() != shares.len() {
                return Err(TileSpecError::ShareCountMismatch {
                    children: children.len(),
                    shares: shares.len(),
                });
            }
            if let Some(index) = shares
                .iter()
                .position(|share| !share.is_finite() || *share <= 0.0)
            {
                return Err(TileSpecError::InvalidShare { index });
            }
            if constraints.min_major.len() > children.len() {
                return Err(TileSpecError::TooManySplitConstraints {
                    children: children.len(),
                    constraints: constraints.min_major.len(),
                });
            }
            if let Some(index) = constraints
                .min_major
                .iter()
                .position(|minimum| !minimum.is_finite() || *minimum < 0.0)
            {
                return Err(TileSpecError::InvalidSplitConstraint { index });
            }
            for child in children {
                validate_spec(child, panes)?;
            }
            Ok(())
        }
    }
}

fn validate_pane(pane: PaneId, panes: &mut Vec<PaneId>) -> Result<(), TileSpecError> {
    if panes.contains(&pane) {
        return Err(TileSpecError::DuplicatePane(pane));
    }
    panes.push(pane);
    Ok(())
}

/// Error returned when an ID-free [`TileSpec`] is structurally invalid.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TileSpecError {
    /// A tab group contains no panes.
    EmptyTabs,
    /// A split contains fewer than two children.
    TooFewSplitChildren {
        /// Number of supplied children.
        children: usize,
    },
    /// The number of shares does not match the number of split children.
    ShareCountMismatch {
        /// Number of supplied children.
        children: usize,
        /// Number of supplied shares.
        shares: usize,
    },
    /// A split share is non-finite or not strictly positive.
    InvalidShare {
        /// Index of the invalid share.
        index: usize,
    },
    /// More per-child constraints were supplied than the split has children.
    TooManySplitConstraints {
        /// Number of supplied children.
        children: usize,
        /// Number of supplied minimum sizes.
        constraints: usize,
    },
    /// A per-child minimum size is non-finite or negative.
    InvalidSplitConstraint {
        /// Index of the invalid minimum size.
        index: usize,
    },
    /// A tab group's active index is outside its pane list.
    InvalidActiveTab {
        /// Supplied active index.
        active: usize,
        /// Number of panes in the group.
        panes: usize,
    },
    /// A pane id appears more than once in the specification.
    DuplicatePane(PaneId),
}

impl fmt::Display for TileSpecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTabs => formatter.write_str("tab group must contain at least one pane"),
            Self::TooFewSplitChildren { children } => {
                write!(
                    formatter,
                    "split must contain at least two children, got {children}"
                )
            }
            Self::ShareCountMismatch { children, shares } => write!(
                formatter,
                "split has {children} children but {shares} shares"
            ),
            Self::InvalidShare { index } => {
                write!(formatter, "invalid split share at index {index}")
            }
            Self::TooManySplitConstraints {
                children,
                constraints,
            } => write!(
                formatter,
                "split has {children} children but {constraints} constraints"
            ),
            Self::InvalidSplitConstraint { index } => {
                write!(formatter, "invalid split constraint at index {index}")
            }
            Self::InvalidActiveTab { active, panes } => write!(
                formatter,
                "active tab index {active} is invalid for {panes} panes"
            ),
            Self::DuplicatePane(pane) => write!(formatter, "duplicate {pane}"),
        }
    }
}

impl core::error::Error for TileSpecError {}
