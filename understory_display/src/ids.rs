// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Stable identifiers for retained display items and semantic provenance.

/// Stable identifier for one display item.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId(u32);

impl ItemId {
    /// Creates an item id from a zero-based index.
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// Returns the underlying index.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
}

/// Optional semantic/provenance identifier carried by one display item.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticId(u32);

impl SemanticId {
    /// Creates a semantic identifier from a host-defined numeric value.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the underlying host-defined value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}
