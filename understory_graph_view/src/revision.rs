// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Revision counter utilities.

/// Monotonic revision counter used by graph-view state objects.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(u64);

impl Revision {
    /// Creates a zero revision.
    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Returns the raw revision value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Bumps the revision and returns the new value.
    pub fn bump(&mut self) -> Self {
        self.0 = self.0.wrapping_add(1);
        *self
    }
}
