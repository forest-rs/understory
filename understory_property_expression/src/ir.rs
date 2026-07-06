// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::any::TypeId;
use core::marker::PhantomData;

use smallvec::SmallVec;
use understory_property::{ErasedValue, Property, PropertyId};

use crate::{
    ExprBuildError, ExprDeps, ExprError, ExprId, ExprResourceKey, FunctionId, FunctionRegistry,
    FunctionSignature,
};

/// Host-provided evaluation context for expression reads.
pub trait ExprEvalCx {
    /// Reads a dependency property from the current subject.
    fn get_property(&mut self, property: PropertyId) -> Result<ErasedValue, ExprError>;

    /// Reads a theme resource value.
    fn get_resource(&mut self, resource: ExprResourceKey) -> Result<ErasedValue, ExprError>;

    /// Returns the function registry used by call nodes.
    fn functions(&self) -> &FunctionRegistry;
}

/// A typed expression.
#[derive(Clone, Debug)]
pub struct Expr<T> {
    inner: ErasedExpr,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Clone + 'static> Expr<T> {
    /// Creates an expression from a literal value.
    #[must_use]
    pub fn literal(value: T) -> Self {
        Self {
            inner: ErasedExpr::single(
                ExprNode::Literal(ErasedValue::new(value)),
                TypeId::of::<T>(),
            ),
            _marker: PhantomData,
        }
    }

    /// Creates an expression that reads a dependency property.
    #[must_use]
    pub fn property(property: Property<T>) -> Self {
        let mut deps = ExprDeps::new();
        deps.add_property(property.id());
        Self {
            inner: ErasedExpr::single_with_deps(
                ExprNode::Property(property.id()),
                TypeId::of::<T>(),
                deps,
            ),
            _marker: PhantomData,
        }
    }

    /// Creates an expression that reads a theme resource.
    #[must_use]
    pub fn token(resource: ExprResourceKey) -> Self {
        let mut deps = ExprDeps::new();
        deps.add_resource(resource);
        Self {
            inner: ErasedExpr::single_with_deps(
                ExprNode::Resource(resource),
                TypeId::of::<T>(),
                deps,
            ),
            _marker: PhantomData,
        }
    }

    /// Creates a lazy conditional expression.
    #[must_use]
    pub fn conditional(condition: Expr<bool>, then_expr: Self, else_expr: Self) -> Self {
        let mut nodes = Vec::new();
        let mut deps = ExprDeps::new();
        let condition = append_expr(&mut nodes, condition.into_erased(), &mut deps);
        let then_expr = append_expr(&mut nodes, then_expr.into_erased(), &mut deps);
        let else_expr = append_expr(&mut nodes, else_expr.into_erased(), &mut deps);
        nodes.push(ExprNode::Conditional {
            condition,
            then_expr,
            else_expr,
        });
        Self {
            inner: ErasedExpr {
                nodes,
                ty: TypeId::of::<T>(),
                deps,
            },
            _marker: PhantomData,
        }
    }

    /// Builds a zero-argument function call expression.
    pub fn call0(
        functions: &FunctionRegistry,
        function: FunctionId,
    ) -> Result<Self, ExprBuildError> {
        Self::call_erased(functions, function, Vec::new())
    }

    /// Builds a one-argument function call expression.
    pub fn call1<A: Clone + 'static>(
        functions: &FunctionRegistry,
        function: FunctionId,
        arg: Expr<A>,
    ) -> Result<Self, ExprBuildError> {
        Self::call_erased(functions, function, alloc::vec![arg.into_erased()])
    }

    /// Builds a two-argument function call expression.
    pub fn call2<A: Clone + 'static, B: Clone + 'static>(
        functions: &FunctionRegistry,
        function: FunctionId,
        first: Expr<A>,
        second: Expr<B>,
    ) -> Result<Self, ExprBuildError> {
        Self::call_erased(
            functions,
            function,
            alloc::vec![first.into_erased(), second.into_erased()],
        )
    }

    /// Builds a three-argument function call expression.
    pub fn call3<A: Clone + 'static, B: Clone + 'static, C: Clone + 'static>(
        functions: &FunctionRegistry,
        function: FunctionId,
        first: Expr<A>,
        second: Expr<B>,
        third: Expr<C>,
    ) -> Result<Self, ExprBuildError> {
        Self::call_erased(
            functions,
            function,
            alloc::vec![
                first.into_erased(),
                second.into_erased(),
                third.into_erased()
            ],
        )
    }

    /// Builds a function call from erased argument expressions.
    pub fn call_erased(
        functions: &FunctionRegistry,
        function: FunctionId,
        args: Vec<ErasedExpr>,
    ) -> Result<Self, ExprBuildError> {
        let arg_types = args.iter().map(ErasedExpr::type_id).collect::<Vec<_>>();
        let signature = functions.validate_call::<T>(function, &arg_types)?;
        Ok(Self::call_with_signature(function, signature, args))
    }

    pub(crate) fn call_with_signature(
        function: FunctionId,
        signature: FunctionSignature,
        args: Vec<ErasedExpr>,
    ) -> Self {
        debug_assert_eq!(
            signature.return_type(),
            TypeId::of::<T>(),
            "call signature return type must match expression result type"
        );
        let mut nodes = Vec::new();
        let mut deps = ExprDeps::new();
        let args = args
            .into_iter()
            .map(|arg| append_expr(&mut nodes, arg, &mut deps))
            .collect();
        nodes.push(ExprNode::Call {
            function,
            signature,
            args,
        });
        Self {
            inner: ErasedExpr {
                nodes,
                ty: TypeId::of::<T>(),
                deps,
            },
            _marker: PhantomData,
        }
    }

    /// Evaluates the expression to its typed value.
    pub fn eval(&self, cx: &mut dyn ExprEvalCx) -> Result<T, ExprError> {
        let value = self.inner.eval_erased(cx)?;
        value
            .downcast_ref::<T>()
            .cloned()
            .ok_or_else(|| ExprError::TypeMismatch {
                expected: TypeId::of::<T>(),
                actual: value.type_id(),
            })
    }

    /// Returns the expression's static dependencies.
    #[must_use]
    pub fn deps(&self) -> &ExprDeps {
        self.inner.deps()
    }

    /// Returns the expression's erased form.
    #[must_use]
    pub fn as_erased(&self) -> &ErasedExpr {
        &self.inner
    }

    /// Consumes the typed expression and returns its erased form.
    #[must_use]
    pub fn into_erased(self) -> ErasedExpr {
        self.inner
    }
}

/// A type-erased expression.
#[derive(Clone, Debug)]
pub struct ErasedExpr {
    nodes: Vec<ExprNode>,
    ty: TypeId,
    deps: ExprDeps,
}

impl ErasedExpr {
    fn single(node: ExprNode, ty: TypeId) -> Self {
        Self::single_with_deps(node, ty, ExprDeps::new())
    }

    fn single_with_deps(node: ExprNode, ty: TypeId, deps: ExprDeps) -> Self {
        Self {
            nodes: alloc::vec![node],
            ty,
            deps,
        }
    }

    /// Returns the expression's result type id.
    #[must_use]
    pub fn type_id(&self) -> TypeId {
        self.ty
    }

    /// Returns the expression's static dependencies.
    #[must_use]
    pub fn deps(&self) -> &ExprDeps {
        &self.deps
    }

    /// Returns the root node id.
    #[must_use]
    pub fn root(&self) -> ExprId {
        ExprId::new(self.nodes.len() - 1)
    }

    /// Returns the expression arena nodes in evaluation order.
    #[must_use]
    pub fn nodes(&self) -> &[ExprNode] {
        &self.nodes
    }

    /// Evaluates the expression to an erased value.
    pub fn eval_erased(&self, cx: &mut dyn ExprEvalCx) -> Result<ErasedValue, ExprError> {
        self.eval_node(self.root(), cx)
    }

    fn eval_node(&self, id: ExprId, cx: &mut dyn ExprEvalCx) -> Result<ErasedValue, ExprError> {
        match &self.nodes[id.index()] {
            ExprNode::Literal(value) => Ok(value.clone()),
            ExprNode::Property(property) => cx.get_property(*property),
            ExprNode::Resource(resource) => cx.get_resource(*resource),
            ExprNode::Conditional {
                condition,
                then_expr,
                else_expr,
            } => {
                let condition_value = self.eval_node(*condition, cx)?;
                let condition =
                    condition_value
                        .downcast_ref::<bool>()
                        .copied()
                        .ok_or_else(|| ExprError::TypeMismatch {
                            expected: TypeId::of::<bool>(),
                            actual: condition_value.type_id(),
                        })?;
                self.eval_node(if condition { *then_expr } else { *else_expr }, cx)
            }
            ExprNode::Call {
                function,
                signature,
                args,
            } => {
                let args = args
                    .iter()
                    .map(|arg| self.eval_node(*arg, cx))
                    .collect::<Result<Vec<_>, _>>()?;
                cx.functions().evaluate(*function, signature, &args)
            }
        }
    }
}

/// One node in an expression arena.
#[derive(Clone, Debug)]
pub enum ExprNode {
    /// A literal erased value.
    Literal(ErasedValue),
    /// A dependency-property read on the current subject.
    Property(PropertyId),
    /// A theme-resource read.
    Resource(ExprResourceKey),
    /// A lazy conditional expression.
    Conditional {
        /// The condition node, expected to evaluate to `bool`.
        condition: ExprId,
        /// The branch evaluated when `condition` is `true`.
        then_expr: ExprId,
        /// The branch evaluated when `condition` is `false`.
        else_expr: ExprId,
    },
    /// A call into the evaluation context's function registry.
    Call {
        /// The function id.
        function: FunctionId,
        /// The signature captured when the expression was built.
        signature: FunctionSignature,
        /// Argument node ids.
        args: SmallVec<[ExprId; 4]>,
    },
}

fn append_expr(nodes: &mut Vec<ExprNode>, expr: ErasedExpr, deps: &mut ExprDeps) -> ExprId {
    let base = nodes.len();
    deps.extend(&expr.deps);
    let root = ExprId::new(base + expr.root().index());
    for node in expr.nodes {
        nodes.push(offset_node(node, base));
    }
    root
}

fn offset_id(id: ExprId, base: usize) -> ExprId {
    ExprId::new(base + id.index())
}

fn offset_node(node: ExprNode, base: usize) -> ExprNode {
    match node {
        ExprNode::Literal(value) => ExprNode::Literal(value),
        ExprNode::Property(property) => ExprNode::Property(property),
        ExprNode::Resource(resource) => ExprNode::Resource(resource),
        ExprNode::Conditional {
            condition,
            then_expr,
            else_expr,
        } => ExprNode::Conditional {
            condition: offset_id(condition, base),
            then_expr: offset_id(then_expr, base),
            else_expr: offset_id(else_expr, base),
        },
        ExprNode::Call {
            function,
            signature,
            args,
        } => ExprNode::Call {
            function,
            signature,
            args: args.into_iter().map(|arg| offset_id(arg, base)).collect(),
        },
    }
}
