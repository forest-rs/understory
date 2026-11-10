// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{Axis, PaneId, TabBarPlacement, TileId};

/// Saved placement of a pane closed with
/// [`TileTree::close_pane_with_bookmark`](crate::TileTree::close_pane_with_bookmark).
///
/// The host can persist this separately from the pane's content and use it to
/// reopen the pane in the current workspace.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaneBookmark {
    pub(crate) pane: PaneId,
    pub(crate) tabs: Option<TabBookmark>,
    pub(crate) split: Option<SplitBookmark>,
    pub(crate) collapsed_extent: Option<f64>,
}

impl PaneBookmark {
    /// Application-owned pane identifier to reopen.
    #[must_use]
    pub const fn pane(&self) -> PaneId {
        self.pane
    }
}

/// Whether reopening recovered the original semantic placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ReopenPlacement {
    /// Reopened at a surviving gap or reconstructed its original split slot.
    Exact,
    /// Used a surviving container, neighbor, or root fallback.
    Approximate,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct TabBookmark {
    pub(crate) group: TileId,
    pub(crate) index: usize,
    pub(crate) placement: TabBarPlacement,
    pub(crate) next: Option<PaneId>,
    pub(crate) previous: Option<PaneId>,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct SplitBookmark {
    pub(crate) parent: TileId,
    pub(crate) axis: Axis,
    pub(crate) index: usize,
    pub(crate) binary: bool,
    pub(crate) ratio: f64,
    pub(crate) minimum: f64,
    pub(crate) next: Option<Neighbor>,
    pub(crate) previous: Option<Neighbor>,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Neighbor {
    pub(crate) tile: TileId,
    pub(crate) pane: PaneId,
}
