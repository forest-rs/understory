// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use understory_property::PropertyId;

use crate::ExprError;

/// Helper stack for detecting recursive property-expression reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InFlightProperties {
    stack: Vec<PropertyId>,
}

impl InFlightProperties {
    /// Creates an empty in-flight stack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes a property, returning a cycle error if it is already in flight.
    pub fn push(&mut self, property: PropertyId) -> Result<(), ExprError> {
        if self.stack.contains(&property) {
            return Err(ExprError::Cycle {
                property,
                stack: self.stack.clone(),
            });
        }
        self.stack.push(property);
        Ok(())
    }

    /// Pops the most recent in-flight property.
    pub fn pop(&mut self) {
        self.stack.pop();
    }

    /// Returns the current in-flight stack.
    #[must_use]
    pub fn stack(&self) -> &[PropertyId] {
        &self.stack
    }

    /// Returns whether the property is already in flight.
    #[must_use]
    pub fn contains(&self, property: PropertyId) -> bool {
        self.stack.contains(&property)
    }
}
