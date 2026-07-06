// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

use understory_property::Property;

use crate::{Expr, ExpressionDefaults, FunctionRegistry};

/// App-facing bundle of expression defaults and function registrations.
///
/// Most embedders should create one `ExpressionLayer`, register any default
/// expressions on it, and pass a reference to expression-aware style
/// resolution.
pub struct ExpressionLayer {
    defaults: ExpressionDefaults,
    functions: FunctionRegistry,
}

impl ExpressionLayer {
    /// Creates an empty expression layer with built-in functions registered.
    #[must_use]
    pub fn new() -> Self {
        Self {
            defaults: ExpressionDefaults::new(),
            functions: FunctionRegistry::with_builtins(),
        }
    }

    /// Registers or replaces the default expression for a property.
    pub fn set_default<T: Clone + 'static>(&mut self, property: Property<T>, expr: Expr<T>) {
        self.defaults.set(property, expr);
    }

    /// Returns the property default expression registry.
    #[must_use]
    pub fn defaults(&self) -> &ExpressionDefaults {
        &self.defaults
    }

    /// Returns the function registry used during expression evaluation.
    #[must_use]
    pub fn functions(&self) -> &FunctionRegistry {
        &self.functions
    }

    /// Returns mutable access to the function registry.
    ///
    /// Embedders can use this to register application-specific functions in
    /// addition to the built-ins installed by [`Self::new`].
    #[must_use]
    pub fn functions_mut(&mut self) -> &mut FunctionRegistry {
        &mut self.functions
    }
}

impl Default for ExpressionLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ExpressionLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExpressionLayer")
            .field("defaults", &self.defaults)
            .field("functions", &self.functions)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use understory_property::{Property, PropertyId};

    use crate::{ExpressionLayer, builtins, expr};

    const WIDTH: Property<f64> = Property::from_id(PropertyId::new(1));

    #[test]
    fn new_installs_builtins() {
        let layer = ExpressionLayer::new();

        assert!(layer.functions().signature(builtins::ADD).is_some());
    }

    #[test]
    fn set_default_stores_expression() {
        let mut layer = ExpressionLayer::new();

        layer.set_default(WIDTH, expr::lit(42.0));

        assert!(layer.defaults().get(WIDTH.id()).is_some());
    }
}
