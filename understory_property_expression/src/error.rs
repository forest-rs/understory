// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::any::TypeId;

use understory_property::PropertyId;

use crate::{ExprResourceKey, FunctionId, FunctionSignature};

/// Error produced while building an expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprBuildError {
    /// The requested function is not registered.
    MissingFunction {
        /// The function id that could not be found.
        function: FunctionId,
    },
    /// A function argument had the wrong expression type.
    ArgumentTypeMismatch {
        /// The function being called.
        function: FunctionId,
        /// The zero-based argument index.
        index: usize,
        /// The type expected by the registered function signature.
        expected: TypeId,
        /// The type produced by the supplied expression.
        actual: TypeId,
    },
    /// The function return type does not match the requested expression type.
    ReturnTypeMismatch {
        /// The function being called.
        function: FunctionId,
        /// The requested expression result type.
        expected: TypeId,
        /// The return type declared by the registered function.
        actual: TypeId,
    },
}

/// Error produced while evaluating an expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprError {
    /// A theme resource was required but not present.
    MissingResource {
        /// The missing resource key.
        key: ExprResourceKey,
    },
    /// A dependency property was required but not present.
    MissingProperty {
        /// The missing property id.
        property: PropertyId,
    },
    /// A function call referenced an unregistered function.
    MissingFunction {
        /// The missing function id.
        function: FunctionId,
    },
    /// A runtime value had a different type than the expression expected.
    TypeMismatch {
        /// The type expected by the expression.
        expected: TypeId,
        /// The actual runtime value type.
        actual: TypeId,
    },
    /// A function id was registered with a signature different from the one the
    /// expression was built against.
    FunctionSignatureMismatch {
        /// The mismatched function id.
        function: FunctionId,
        /// The signature stored in the expression.
        expected: FunctionSignature,
        /// The signature currently registered for the function id.
        actual: FunctionSignature,
    },
    /// Evaluation tried to read a property that is already being resolved.
    Cycle {
        /// The property that closed the cycle.
        property: PropertyId,
        /// The in-flight property stack at the point the cycle was detected.
        stack: Vec<PropertyId>,
    },
    /// A registered function reported an evaluation failure.
    Function {
        /// The function that failed.
        function: FunctionId,
    },
}

/// Error produced while registering a function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FunctionRegistrationError {
    /// The function id is already registered.
    DuplicateFunction {
        /// The duplicate function id.
        function: FunctionId,
    },
}
