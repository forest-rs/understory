// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Revision counter utilities.

/// Monotonic revision counter used by node-graph state objects.
///
/// Revisions let [`GraphComputed`](crate::GraphComputed) cheaply notice that a
/// document, projection, or session changed since the last rebuild. They are
/// not global timestamps and should only be compared within the object that
/// produced them.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(u64);

impl Revision {
    /// Creates a zero revision.
    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Returns the raw revision value.
    ///
    /// This is mainly useful for diagnostics or host caches that store their own
    /// last-seen revision.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Bumps the revision and returns the new value.
    ///
    /// Revisions wrap on overflow. They are intended for change detection across
    /// ordinary UI lifetimes, not for persistent version histories.
    pub fn bump(&mut self) -> Self {
        self.0 = self.0.wrapping_add(1);
        *self
    }
}
