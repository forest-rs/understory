// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::fmt;

use understory_property::{Property, PropertyId};

use crate::{ErasedExpr, Expr, ExprDeps};

/// Borrowed reference to one expression default entry.
#[derive(Copy, Clone, Debug)]
pub struct ExpressionDefaultRef<'a> {
    property: PropertyId,
    expression: &'a ErasedExpr,
}

impl<'a> ExpressionDefaultRef<'a> {
    /// Returns the property whose default is supplied by this expression.
    #[must_use]
    pub const fn property(self) -> PropertyId {
        self.property
    }

    /// Returns the erased default expression.
    #[must_use]
    pub const fn expression(self) -> &'a ErasedExpr {
        self.expression
    }

    /// Returns the expression's static dependencies.
    #[must_use]
    pub fn deps(self) -> &'a ExprDeps {
        self.expression.deps()
    }
}

/// Expression defaults keyed by dependency property.
#[derive(Clone, Default)]
pub struct ExpressionDefaults {
    entries: Vec<(PropertyId, ErasedExpr)>,
}

impl ExpressionDefaults {
    /// Creates an empty default-expression registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers or replaces the default expression for a property.
    pub fn set<T: Clone + 'static>(&mut self, property: Property<T>, expr: Expr<T>) {
        let id = property.id();
        match self
            .entries
            .binary_search_by_key(&id, |(property, _)| *property)
        {
            Ok(index) => self.entries[index].1 = expr.into_erased(),
            Err(index) => self.entries.insert(index, (id, expr.into_erased())),
        }
    }

    /// Returns the erased default expression for a property id.
    #[must_use]
    pub fn get(&self, property: PropertyId) -> Option<&ErasedExpr> {
        self.entries
            .binary_search_by_key(&property, |(id, _)| *id)
            .ok()
            .map(|index| &self.entries[index].1)
    }

    /// Returns the dependencies for a property's default expression.
    #[must_use]
    pub fn expression_deps(&self, property: PropertyId) -> Option<&ExprDeps> {
        self.get(property).map(ErasedExpr::deps)
    }

    /// Returns the registered expression defaults in property-id order.
    pub fn expression_entries(&self) -> impl Iterator<Item = ExpressionDefaultRef<'_>> {
        self.entries
            .iter()
            .map(|(property, expression)| ExpressionDefaultRef {
                property: *property,
                expression,
            })
    }

    /// Returns `true` if no expression defaults are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of registered expression defaults.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl fmt::Debug for ExpressionDefaults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExpressionDefaults")
            .field("len", &self.entries.len())
            .finish()
    }
}
