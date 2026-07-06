// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::any::TypeId;
use core::fmt;

use understory_property::ErasedValue;

use crate::{ExprBuildError, ExprError, FunctionId, FunctionRegistrationError};

type EvalFn = dyn Fn(&[ErasedValue]) -> Result<ErasedValue, ExprError>;

/// Type signature for an expression function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionSignature {
    arg_types: Vec<TypeId>,
    return_type: TypeId,
}

impl FunctionSignature {
    /// Creates a signature from erased argument and return type ids.
    #[must_use]
    pub fn new(arg_types: Vec<TypeId>, return_type: TypeId) -> Self {
        Self {
            arg_types,
            return_type,
        }
    }

    /// Creates a zero-argument function signature.
    #[must_use]
    pub fn of0<R: 'static>() -> Self {
        Self::new(Vec::new(), TypeId::of::<R>())
    }

    /// Creates a one-argument function signature.
    #[must_use]
    pub fn of1<A: 'static, R: 'static>() -> Self {
        Self::new(vec![TypeId::of::<A>()], TypeId::of::<R>())
    }

    /// Creates a two-argument function signature.
    #[must_use]
    pub fn of2<A: 'static, B: 'static, R: 'static>() -> Self {
        Self::new(
            vec![TypeId::of::<A>(), TypeId::of::<B>()],
            TypeId::of::<R>(),
        )
    }

    /// Creates a three-argument function signature.
    #[must_use]
    pub fn of3<A: 'static, B: 'static, C: 'static, R: 'static>() -> Self {
        Self::new(
            vec![TypeId::of::<A>(), TypeId::of::<B>(), TypeId::of::<C>()],
            TypeId::of::<R>(),
        )
    }

    /// Returns the function argument types.
    #[must_use]
    pub fn arg_types(&self) -> &[TypeId] {
        &self.arg_types
    }

    /// Returns the function return type.
    #[must_use]
    pub fn return_type(&self) -> TypeId {
        self.return_type
    }
}

struct FunctionEntry {
    signature: FunctionSignature,
    eval: Box<EvalFn>,
}

/// Registry of expression functions available during build and evaluation.
#[derive(Default)]
pub struct FunctionRegistry {
    entries: Vec<(FunctionId, FunctionEntry)>,
}

impl FunctionRegistry {
    /// Creates an empty function registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a registry containing the built-in functions.
    #[must_use]
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        crate::register_builtins(&mut registry);
        registry
    }

    /// Registers a function with an erased evaluator.
    pub fn register_erased<F>(
        &mut self,
        function: FunctionId,
        signature: FunctionSignature,
        eval: F,
    ) -> Result<(), FunctionRegistrationError>
    where
        F: Fn(&[ErasedValue]) -> Result<ErasedValue, ExprError> + 'static,
    {
        match self.entries.binary_search_by_key(&function, |(id, _)| *id) {
            Ok(_) => Err(FunctionRegistrationError::DuplicateFunction { function }),
            Err(index) => {
                self.entries.insert(
                    index,
                    (
                        function,
                        FunctionEntry {
                            signature,
                            eval: Box::new(eval),
                        },
                    ),
                );
                Ok(())
            }
        }
    }

    /// Returns the signature for a registered function.
    #[must_use]
    pub fn signature(&self, function: FunctionId) -> Option<&FunctionSignature> {
        self.entry(function).map(|entry| &entry.signature)
    }

    pub(crate) fn validate_call<T: 'static>(
        &self,
        function: FunctionId,
        arg_types: &[TypeId],
    ) -> Result<FunctionSignature, ExprBuildError> {
        let signature = self
            .signature(function)
            .ok_or(ExprBuildError::MissingFunction { function })?;
        for (index, (expected, actual)) in signature
            .arg_types()
            .iter()
            .copied()
            .zip(arg_types.iter().copied())
            .enumerate()
        {
            if expected != actual {
                return Err(ExprBuildError::ArgumentTypeMismatch {
                    function,
                    index,
                    expected,
                    actual,
                });
            }
        }
        if signature.arg_types().len() != arg_types.len() {
            let index = signature.arg_types().len().min(arg_types.len());
            let expected = signature
                .arg_types()
                .get(index)
                .copied()
                .unwrap_or(TypeId::of::<()>());
            let actual = arg_types.get(index).copied().unwrap_or(TypeId::of::<()>());
            return Err(ExprBuildError::ArgumentTypeMismatch {
                function,
                index,
                expected,
                actual,
            });
        }
        let expected_return = TypeId::of::<T>();
        if signature.return_type() != expected_return {
            return Err(ExprBuildError::ReturnTypeMismatch {
                function,
                expected: expected_return,
                actual: signature.return_type(),
            });
        }
        Ok(signature.clone())
    }

    pub(crate) fn evaluate(
        &self,
        function: FunctionId,
        expected_signature: &FunctionSignature,
        args: &[ErasedValue],
    ) -> Result<ErasedValue, ExprError> {
        let entry = self
            .entry(function)
            .ok_or(ExprError::MissingFunction { function })?;
        if &entry.signature != expected_signature {
            return Err(ExprError::FunctionSignatureMismatch {
                function,
                expected: expected_signature.clone(),
                actual: entry.signature.clone(),
            });
        }
        (entry.eval)(args)
    }

    fn entry(&self, function: FunctionId) -> Option<&FunctionEntry> {
        self.entries
            .binary_search_by_key(&function, |(id, _)| *id)
            .ok()
            .map(|index| &self.entries[index].1)
    }
}

impl fmt::Debug for FunctionRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FunctionRegistry")
            .field("len", &self.entries.len())
            .finish()
    }
}
