// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Document-local index into a timeline document's lane list.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LaneId(usize);

impl LaneId {
    /// Creates an identifier from a zero-based lane index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the underlying zero-based lane index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Document-local index into a timeline document's span list.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpanId(usize);

impl SpanId {
    /// Creates an identifier from a zero-based span index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the underlying zero-based span index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Document-local index into a timeline document's marker list.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MarkerId(usize);

impl MarkerId {
    /// Creates an identifier from a zero-based marker index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the underlying zero-based marker index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Document-local index into a timeline document's flow list.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FlowId(usize);

impl FlowId {
    /// Creates an identifier from a zero-based flow index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Returns the underlying zero-based flow index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Stable caller-defined identity for a timeline content record.
///
/// Item keys are intended for identity that survives document replacement:
/// selected frames, diffing, retained view state, and adapter-specific lookup.
/// The raw value `0` is reserved as [`Self::ANONYMOUS`]; use it for simple
/// records that do not need durable identity. Non-anonymous keys are validated
/// as unique across lanes, spans, markers, and flows.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimelineItemKey(u64);

impl TimelineItemKey {
    /// Anonymous item identity.
    ///
    /// Anonymous keys are ignored by uniqueness validation and should not be
    /// used for selection that needs to survive content replacement.
    pub const ANONYMOUS: Self = Self(0);

    /// Creates an item key from a caller-defined raw value.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the raw caller-defined key value.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Returns whether this is [`Self::ANONYMOUS`].
    #[must_use]
    pub const fn is_anonymous(self) -> bool {
        self.0 == Self::ANONYMOUS.0
    }
}
