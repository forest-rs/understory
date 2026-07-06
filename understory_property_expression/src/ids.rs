// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

/// Index of a node inside one expression arena.
///
/// `ExprId` values are local to an [`ErasedExpr`](crate::ErasedExpr). They are
/// not stable provenance identifiers outside that expression.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExprId(usize);

impl ExprId {
    /// Creates a node id from an arena index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the arena index for this node id.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

impl fmt::Debug for ExprId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ExprId").field(&self.0).finish()
    }
}

/// A key for looking up expression resources in a host theme.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExprResourceKey(u16);

impl ExprResourceKey {
    /// Creates a resource key from its compact index.
    #[must_use]
    pub const fn new(index: u16) -> Self {
        Self(index)
    }

    /// Returns the underlying resource index.
    #[must_use]
    pub const fn index(self) -> u16 {
        self.0
    }
}

impl fmt::Debug for ExprResourceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ExprResourceKey").field(&self.0).finish()
    }
}

/// A stable semantic identifier for a registered expression function.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunctionId(u16);

impl FunctionId {
    /// Creates a function id from its compact index.
    #[must_use]
    pub const fn new(index: u16) -> Self {
        Self(index)
    }

    /// Returns the underlying function index.
    #[must_use]
    pub const fn index(self) -> u16 {
        self.0
    }
}

impl fmt::Debug for FunctionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("FunctionId").field(&self.0).finish()
    }
}
