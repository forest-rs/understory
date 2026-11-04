// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Revision-covered changes produced by scoped document and projection edits.

#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as Shared;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc as Shared;
use alloc::vec::Vec;

use crate::{GraphElementId, Revision};

/// Allocation-backed instance identity without a process-global counter.
/// Retaining a token prevents address reuse while a cache or receipt refers to it.
#[derive(Debug)]
pub(crate) struct SourceIdentity(Shared<()>);

impl Default for SourceIdentity {
    fn default() -> Self {
        Self(Shared::new(()))
    }
}

impl Clone for SourceIdentity {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl SourceIdentity {
    pub(crate) fn token(&self) -> SourceToken {
        SourceToken(Shared::clone(&self.0))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SourceToken(Shared<()>);

impl PartialEq for SourceToken {
    fn eq(&self, other: &Self) -> bool {
        Shared::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for SourceToken {}

/// Source whose mutations a change receipt covers.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ChangeSource {
    /// A graph document.
    Document,
    /// A graph projection.
    Projection,
}

/// Complete changes from one scoped edit, tied to its source instance and revisions.
///
/// Obtain this from [`GraphDoc::edit`](crate::GraphDoc::edit) or
/// [`GraphProjection::edit`](crate::GraphProjection::edit), then pass it to
/// [`GraphInvalidation::record`](crate::GraphInvalidation::record) for each
/// computed cache that should consume it. Receipts are reusable and cloneable;
/// sources retain no mutation journal. Skipped, reordered or foreign receipts
/// cannot certify a revision transition and cause a full rebuild instead.
#[derive(Clone, Debug)]
pub struct GraphChanges {
    pub(crate) source: ChangeSource,
    pub(crate) identity: SourceToken,
    pub(crate) before: Revision,
    pub(crate) after: Revision,
    pub(crate) targets: Vec<GraphElementId>,
}

impl GraphChanges {
    /// Source kind changed by the edit.
    #[must_use]
    pub const fn source(&self) -> ChangeSource {
        self.source
    }
    /// Revision immediately before the scoped edit.
    #[must_use]
    pub const fn before(&self) -> Revision {
        self.before
    }
    /// Revision after all mutations in the scoped edit.
    #[must_use]
    pub const fn after(&self) -> Revision {
        self.after
    }
    /// Changed entities, including retired ports and edges needed for cache cleanup.
    pub fn targets(&self) -> impl Iterator<Item = GraphElementId> + '_ {
        self.targets.iter().copied()
    }
    /// Whether the edit left its source revision unchanged.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.before == self.after
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Coverage {
    identity: SourceToken,
    before: Revision,
    after: Revision,
    complete: bool,
}

impl Coverage {
    pub(crate) fn append(coverage: &mut Option<Self>, changes: &GraphChanges) {
        match coverage {
            Some(current) => {
                current.complete &=
                    current.identity == changes.identity && current.after == changes.before;
                current.after = changes.after;
            }
            None => {
                *coverage = Some(Self {
                    identity: changes.identity.clone(),
                    before: changes.before,
                    after: changes.after,
                    complete: true,
                });
            }
        }
    }
    pub(crate) fn covers(&self, identity: &SourceToken, before: Revision, after: Revision) -> bool {
        self.complete && self.identity == *identity && self.before == before && self.after == after
    }
}
