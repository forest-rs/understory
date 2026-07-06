// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use smallvec::{Array, SmallVec};
use understory_property::PropertyId;

use crate::ExprResourceKey;

/// Static dependencies referenced by one expression.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExprDeps {
    /// Dependency properties read by the expression.
    pub properties: SmallVec<[PropertyId; 4]>,
    /// Theme resources read by the expression.
    pub resources: SmallVec<[ExprResourceKey; 4]>,
}

impl ExprDeps {
    /// Creates an empty dependency set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn add_property(&mut self, property: PropertyId) {
        insert_sorted_unique(&mut self.properties, property);
    }

    pub(crate) fn add_resource(&mut self, resource: ExprResourceKey) {
        insert_sorted_unique(&mut self.resources, resource);
    }

    pub(crate) fn extend(&mut self, other: &Self) {
        for property in other.properties.iter().copied() {
            self.add_property(property);
        }
        for resource in other.resources.iter().copied() {
            self.add_resource(resource);
        }
    }
}

fn insert_sorted_unique<A>(items: &mut SmallVec<A>, item: A::Item)
where
    A: Array,
    A::Item: Copy + Ord,
{
    match items.binary_search(&item) {
        Ok(_) => {}
        Err(index) => items.insert(index, item),
    }
}
