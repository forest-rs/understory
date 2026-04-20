// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained display list container.

use alloc::vec::Vec;

use crate::DisplayItem;

/// Retained display items in stable insertion order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayList {
    items: Vec<DisplayItem>,
}

impl DisplayList {
    /// Creates an empty display list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` when the display list has no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the number of retained display items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns the retained items in insertion order.
    #[must_use]
    pub fn items(&self) -> &[DisplayItem] {
        &self.items
    }

    pub(crate) fn push(&mut self, item: DisplayItem) {
        self.items.push(item);
    }
}
