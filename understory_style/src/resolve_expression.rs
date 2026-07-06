// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::any::TypeId;

use understory_property::{ErasedValue, Property, PropertyId};
use understory_property_expression::{
    ErasedExpr, ExprError, ExprEvalCx, ExprResourceKey, InFlightProperties,
};

use super::{ExprResolveOptions, ResolveCx, ResolveParentLookup, ResolveSubject};
use crate::{MatchState, ResourceKey, StyleCascade};

impl<'a, K, F> ResolveCx<'a, K, F>
where
    K: Copy + Eq + 'a,
    F: ResolveParentLookup<'a, K>,
{
    pub(super) fn eval_expression<'cx, 's, T>(
        &'cx self,
        subject: ResolveSubject<'s, K>,
        property: Property<T>,
        style: Option<(&'cx StyleCascade, MatchState)>,
        expr: ExprResolveOptions<'cx>,
        expression: &ErasedExpr,
        in_flight: &mut InFlightProperties,
    ) -> Result<T, ExprError>
    where
        T: Clone + 'static,
    {
        let value = self.eval_expression_erased(
            subject,
            property.id(),
            style,
            expr,
            expression,
            TypeId::of::<T>(),
            in_flight,
        )?;
        value
            .downcast_ref::<T>()
            .cloned()
            .ok_or_else(|| ExprError::TypeMismatch {
                expected: TypeId::of::<T>(),
                actual: value.type_id(),
            })
    }

    pub(super) fn eval_default_expression<'cx, 's, T>(
        &'cx self,
        subject: ResolveSubject<'s, K>,
        property: Property<T>,
        style: Option<(&'cx StyleCascade, MatchState)>,
        expr: ExprResolveOptions<'cx>,
        default_expr: &ErasedExpr,
        in_flight: &mut InFlightProperties,
    ) -> Result<T, ExprError>
    where
        T: Clone + 'static,
    {
        let expected = self
            .registry
            .get(property.id())
            .ok_or(ExprError::MissingProperty {
                property: property.id(),
            })?
            .type_id();
        let value = self.eval_expression_erased(
            subject,
            property.id(),
            style,
            expr,
            default_expr,
            expected,
            in_flight,
        )?;
        value
            .downcast_ref::<T>()
            .cloned()
            .ok_or_else(|| ExprError::TypeMismatch {
                expected: TypeId::of::<T>(),
                actual: value.type_id(),
            })
    }

    pub(super) fn eval_default_expression_erased<'cx, 's>(
        &'cx self,
        subject: ResolveSubject<'s, K>,
        property: PropertyId,
        style: Option<(&'cx StyleCascade, MatchState)>,
        expr: ExprResolveOptions<'cx>,
        default_expr: &ErasedExpr,
        in_flight: &mut InFlightProperties,
    ) -> Result<ErasedValue, ExprError> {
        let expected = self
            .registry
            .get(property)
            .ok_or(ExprError::MissingProperty { property })?
            .type_id();
        self.eval_expression_erased(
            subject,
            property,
            style,
            expr,
            default_expr,
            expected,
            in_flight,
        )
    }

    pub(super) fn eval_expression_erased<'cx, 's>(
        &'cx self,
        subject: ResolveSubject<'s, K>,
        property: PropertyId,
        style: Option<(&'cx StyleCascade, MatchState)>,
        expr: ExprResolveOptions<'cx>,
        expression: &ErasedExpr,
        expected: TypeId,
        in_flight: &mut InFlightProperties,
    ) -> Result<ErasedValue, ExprError> {
        if expression.type_id() != expected {
            return Err(ExprError::TypeMismatch {
                expected,
                actual: expression.type_id(),
            });
        }

        in_flight.push(property)?;
        let mut cx = ResolveExprEvalCx {
            resolve: self,
            subject,
            style,
            expr,
            in_flight,
        };
        let result = expression.eval_erased(&mut cx).and_then(|value| {
            if value.type_id() == expected {
                Ok(value)
            } else {
                Err(ExprError::TypeMismatch {
                    expected,
                    actual: value.type_id(),
                })
            }
        });
        cx.in_flight.pop();
        result
    }

    fn resolve_erased_property<'cx, 's>(
        &'cx self,
        subject: ResolveSubject<'s, K>,
        property: PropertyId,
        style: Option<(&'cx StyleCascade, MatchState)>,
        expr: ExprResolveOptions<'cx>,
        in_flight: &mut InFlightProperties,
    ) -> Result<ErasedValue, ExprError> {
        if let Some(value) = subject.store.animation_erased(property) {
            return Ok(value.clone());
        }
        if let Some(value) = subject.store.local_winner_erased(property) {
            return Ok(value.clone());
        }
        if let Some((style, state)) = style
            && let Some(value) =
                self.style_erased_resolved_value(subject, style, state, property, expr, in_flight)?
        {
            return Ok(value);
        }
        if self.registry.inherits(property)
            && let Some(value) = self.inherited_erased_value(
                subject.parent_key,
                property,
                style.map(|s| s.0),
                expr,
                in_flight,
            )?
        {
            return Ok(value);
        }
        if let Some(default_expr) = expr.defaults.get(property) {
            return self.eval_default_expression_erased(
                subject,
                property,
                style,
                expr,
                default_expr,
                in_flight,
            );
        }
        self.registry
            .default_value_erased(property)
            .ok_or(ExprError::MissingProperty { property })
    }
}

struct ResolveExprEvalCx<'cx, 's, 'a, K, F>
where
    K: Copy + Eq + 'a,
    F: ResolveParentLookup<'a, K>,
{
    resolve: &'cx ResolveCx<'a, K, F>,
    subject: ResolveSubject<'s, K>,
    style: Option<(&'cx StyleCascade, MatchState)>,
    expr: ExprResolveOptions<'cx>,
    in_flight: &'cx mut InFlightProperties,
}

impl<'cx, 's, 'a, K, F> ExprEvalCx for ResolveExprEvalCx<'cx, 's, 'a, K, F>
where
    K: Copy + Eq + 'a,
    F: ResolveParentLookup<'a, K>,
{
    fn get_property(&mut self, property: PropertyId) -> Result<ErasedValue, ExprError> {
        self.resolve.resolve_erased_property(
            self.subject,
            property,
            self.style,
            self.expr,
            self.in_flight,
        )
    }

    fn get_resource(&mut self, resource: ExprResourceKey) -> Result<ErasedValue, ExprError> {
        let key = ResourceKey::from(resource);
        self.resolve
            .theme
            .get_erased(key)
            .ok_or(ExprError::MissingResource { key: resource })
    }

    fn functions(&self) -> &understory_property_expression::FunctionRegistry {
        self.expr.functions
    }
}
