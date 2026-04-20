// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Retained display command-stream container.

use alloc::vec::Vec;

use crate::{DisplayEntry, DisplayItem};

/// Retained display entries in stable insertion order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayList {
    entries: Vec<DisplayEntry>,
}

impl DisplayList {
    /// Creates an empty display list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` when the display list has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of retained display entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns the retained entries in insertion order.
    #[must_use]
    pub fn entries(&self) -> &[DisplayEntry] {
        &self.entries
    }

    /// Returns the retained paint items in insertion order.
    pub fn items(&self) -> impl Iterator<Item = &DisplayItem> {
        self.entries.iter().filter_map(|entry| match entry {
            DisplayEntry::Item(item) => Some(item.as_ref()),
            _ => None,
        })
    }

    pub(crate) fn push(&mut self, entry: DisplayEntry) {
        self.entries.push(entry);
    }
}
