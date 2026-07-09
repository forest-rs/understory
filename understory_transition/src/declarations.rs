// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::any::TypeId;
use core::fmt;

use peniko::Color;
use understory_motion::{ColorInterpolation, Interpolate, TimingFunction};
use understory_property::{ErasedValue, Property, PropertyId};
use understory_timing::TimerDuration;

/// Erased pairwise interpolation captured for one value type.
///
/// The [`ColorInterpolation`] argument is consulted only by color-aware
/// captures; scalar captures ignore it.
pub(crate) type LerpFn =
    fn(&ErasedValue, &ErasedValue, f64, ColorInterpolation) -> Option<ErasedValue>;

/// Erased equality comparison captured for one value type.
pub(crate) type EqFn = fn(&ErasedValue, &ErasedValue) -> bool;

/// A stable semantic identifier for a host-owned motion resource.
///
/// This crate does not know what the token means; hosts map it onto their
/// theme or resource system when they implement [`TransitionResources`],
/// following the `understory_property_expression::ExprResourceKey` precedent.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransitionResourceKey(u16);

impl TransitionResourceKey {
    /// Creates a resource key from its compact index.
    #[must_use]
    pub const fn new(index: u16) -> Self {
        Self(index)
    }

    /// Returns the underlying resource index.
    #[must_use]
    pub const fn index(self) -> u16 {
        self.0
    }
}

impl fmt::Debug for TransitionResourceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TransitionResourceKey")
            .field(&self.0)
            .finish()
    }
}

/// A declaration field that is either a literal or deferred to a resource.
///
/// Deferred fields are resolved when a transition **starts** (including a
/// retarget, which starts a replacement transition), so a theme swap retunes
/// motion for every subsequent transition without rebuilding styles or
/// declaration sets. This matches CSS Transitions, which resolves transition
/// parameters from the after-change style at the moment a transition begins.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LateBound<T> {
    /// A literal value.
    Value(T),
    /// Deferred to a host resource, looked up at transition start.
    Resource(TransitionResourceKey),
}

/// Host adapter that resolves [`TransitionResourceKey`] tokens.
///
/// Every method defaults to `None`; a missing or mistyped resource makes the
/// affected field fall back to its initial value (zero duration/delay, linear
/// easing, default color interpolation), mirroring how CSS treats a value
/// that fails to resolve.
pub trait TransitionResources {
    /// Resolves a duration or delay token.
    fn duration(&self, key: TransitionResourceKey) -> Option<TimerDuration> {
        let _ = key;
        None
    }

    /// Resolves an easing token.
    fn timing_function(&self, key: TransitionResourceKey) -> Option<TimingFunction> {
        let _ = key;
        None
    }

    /// Resolves a color-interpolation policy token.
    fn color_interpolation(&self, key: TransitionResourceKey) -> Option<ColorInterpolation> {
        let _ = key;
        None
    }
}

/// A [`TransitionResources`] that resolves nothing.
///
/// Use this when every declaration field is a literal.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct NoTransitionResources;

impl TransitionResources for NoTransitionResources {}

/// Duration, delay, and easing literals for one property's transition.
///
/// Durations use the shared [`TimerDuration`] tick unit; hosts conventionally
/// use nanoseconds, and [`understory_motion::millis`] converts from
/// milliseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionTiming {
    duration: TimerDuration,
    delay: TimerDuration,
    timing: TimingFunction,
}

impl TransitionTiming {
    /// Creates a timing declaration with the given duration, no delay, and
    /// linear easing.
    #[must_use]
    pub const fn new(duration: TimerDuration) -> Self {
        Self {
            duration,
            delay: 0,
            timing: TimingFunction::LINEAR,
        }
    }

    /// Sets the delay before interpolation begins.
    #[must_use]
    pub const fn with_delay(mut self, delay: TimerDuration) -> Self {
        self.delay = delay;
        self
    }

    /// Sets the easing function.
    #[must_use]
    pub const fn with_timing(mut self, timing: TimingFunction) -> Self {
        self.timing = timing;
        self
    }

    /// Returns the interpolation duration.
    #[must_use]
    pub const fn duration(&self) -> TimerDuration {
        self.duration
    }

    /// Returns the delay before interpolation begins.
    #[must_use]
    pub const fn delay(&self) -> TimerDuration {
        self.delay
    }

    /// Returns the easing function.
    #[must_use]
    pub const fn timing(&self) -> TimingFunction {
        self.timing
    }

    /// Returns `delay + duration`.
    ///
    /// A transition only runs when this is non-zero; otherwise the target
    /// value applies immediately.
    #[must_use]
    pub const fn combined_duration(&self) -> TimerDuration {
        self.delay.saturating_add(self.duration)
    }
}

/// Distinguishes how the interpolation was captured, so declarations compare
/// meaningfully without comparing function pointers.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum CaptureKind {
    /// Captured through a type's [`Interpolate`] implementation.
    Interpolate,
    /// Captured through [`ColorInterpolation`], honoring the declaration's
    /// color-interpolation field.
    Color,
}

/// One per-target transition declaration.
///
/// The value type is fixed when the declaration is created (through a typed
/// [`Property<T>`] handle or an explicit type parameter) and recorded as a
/// [`TypeId`]; the runner checks supplied endpoint values against it before
/// starting a transition. Timing and color-interpolation fields may be
/// literals or [`LateBound::Resource`] tokens resolved at transition start.
#[derive(Clone, Copy, Debug)]
pub struct TransitionDeclaration<G = PropertyId> {
    target: G,
    duration: LateBound<TimerDuration>,
    delay: LateBound<TimerDuration>,
    timing: LateBound<TimingFunction>,
    color_interpolation: LateBound<ColorInterpolation>,
    value_type: TypeId,
    kind: CaptureKind,
    lerp: LerpFn,
    eq: EqFn,
}

impl TransitionDeclaration<PropertyId> {
    /// Creates a declaration for a dependency property, capturing its value
    /// type's [`Interpolate`] implementation.
    #[must_use]
    pub fn of<T: Interpolate + PartialEq + 'static>(
        property: Property<T>,
        timing: TransitionTiming,
    ) -> Self {
        Self::for_target::<T>(property.id(), timing)
    }

    /// Creates a declaration for a color-valued dependency property whose
    /// interpolation honors the declaration's color-interpolation field.
    #[must_use]
    pub fn of_color(property: Property<Color>, timing: TransitionTiming) -> Self {
        Self::for_color_target(property.id(), timing)
    }
}

impl<G> TransitionDeclaration<G> {
    /// Creates a declaration for an arbitrary host target key, capturing
    /// `T`'s [`Interpolate`] implementation.
    #[must_use]
    pub fn for_target<T: Interpolate + PartialEq + 'static>(
        target: G,
        timing: TransitionTiming,
    ) -> Self {
        Self {
            target,
            duration: LateBound::Value(timing.duration()),
            delay: LateBound::Value(timing.delay()),
            timing: LateBound::Value(timing.timing()),
            color_interpolation: LateBound::Value(ColorInterpolation::default()),
            value_type: TypeId::of::<T>(),
            kind: CaptureKind::Interpolate,
            lerp: lerp_erased::<T>,
            eq: eq_erased::<T>,
        }
    }

    /// Creates a color declaration for an arbitrary host target key.
    ///
    /// Unlike [`Self::for_target::<Color>`](Self::for_target), this capture
    /// interpolates through the declaration's color-interpolation field
    /// (literal or resource-deferred) instead of the default policy.
    #[must_use]
    pub fn for_color_target(target: G, timing: TransitionTiming) -> Self {
        Self {
            target,
            duration: LateBound::Value(timing.duration()),
            delay: LateBound::Value(timing.delay()),
            timing: LateBound::Value(timing.timing()),
            color_interpolation: LateBound::Value(ColorInterpolation::default()),
            value_type: TypeId::of::<Color>(),
            kind: CaptureKind::Color,
            lerp: lerp_color,
            eq: eq_erased::<Color>,
        }
    }

    /// Defers the duration to a host resource.
    #[must_use]
    pub const fn with_duration_resource(mut self, key: TransitionResourceKey) -> Self {
        self.duration = LateBound::Resource(key);
        self
    }

    /// Defers the delay to a host resource.
    #[must_use]
    pub const fn with_delay_resource(mut self, key: TransitionResourceKey) -> Self {
        self.delay = LateBound::Resource(key);
        self
    }

    /// Defers the easing function to a host resource.
    #[must_use]
    pub const fn with_timing_resource(mut self, key: TransitionResourceKey) -> Self {
        self.timing = LateBound::Resource(key);
        self
    }

    /// Sets a literal color-interpolation policy.
    ///
    /// Only color captures ([`TransitionDeclaration::of_color`] /
    /// [`Self::for_color_target`]) consult this field.
    #[must_use]
    pub const fn with_color_interpolation(mut self, interpolation: ColorInterpolation) -> Self {
        self.color_interpolation = LateBound::Value(interpolation);
        self
    }

    /// Defers the color-interpolation policy to a host resource.
    #[must_use]
    pub const fn with_color_interpolation_resource(mut self, key: TransitionResourceKey) -> Self {
        self.color_interpolation = LateBound::Resource(key);
        self
    }

    /// Returns the declared duration field.
    #[must_use]
    pub const fn duration(&self) -> LateBound<TimerDuration> {
        self.duration
    }

    /// Returns the declared delay field.
    #[must_use]
    pub const fn delay(&self) -> LateBound<TimerDuration> {
        self.delay
    }

    /// Returns the declared easing field.
    #[must_use]
    pub const fn timing(&self) -> LateBound<TimingFunction> {
        self.timing
    }

    /// Returns the declared color-interpolation field.
    #[must_use]
    pub const fn color_interpolation(&self) -> LateBound<ColorInterpolation> {
        self.color_interpolation
    }

    /// Returns the [`TypeId`] of the value type this declaration interpolates.
    #[must_use]
    pub const fn value_type(&self) -> TypeId {
        self.value_type
    }

    /// Resolves the timing fields against host resources, as done at
    /// transition start.
    ///
    /// A missing resource falls back to the field's initial value: zero
    /// duration or delay, linear easing.
    #[must_use]
    pub fn resolve_timing<R: TransitionResources + ?Sized>(
        &self,
        resources: &R,
    ) -> TransitionTiming {
        let duration = match self.duration {
            LateBound::Value(value) => value,
            LateBound::Resource(key) => resources.duration(key).unwrap_or(0),
        };
        let delay = match self.delay {
            LateBound::Value(value) => value,
            LateBound::Resource(key) => resources.duration(key).unwrap_or(0),
        };
        let timing = match self.timing {
            LateBound::Value(value) => value,
            LateBound::Resource(key) => resources
                .timing_function(key)
                .unwrap_or(TimingFunction::LINEAR),
        };
        TransitionTiming::new(duration)
            .with_delay(delay)
            .with_timing(timing)
    }

    /// Resolves the color-interpolation field against host resources, as done
    /// at transition start. A missing resource falls back to the default
    /// policy.
    #[must_use]
    pub fn resolve_color_interpolation<R: TransitionResources + ?Sized>(
        &self,
        resources: &R,
    ) -> ColorInterpolation {
        match self.color_interpolation {
            LateBound::Value(value) => value,
            LateBound::Resource(key) => resources.color_interpolation(key).unwrap_or_default(),
        }
    }

    pub(crate) const fn lerp(&self) -> LerpFn {
        self.lerp
    }

    pub(crate) const fn eq(&self) -> EqFn {
        self.eq
    }
}

impl<G: Copy> TransitionDeclaration<G> {
    /// Returns the target key this declaration applies to.
    #[must_use]
    pub fn target(&self) -> G {
        self.target
    }
}

impl<G: PartialEq> PartialEq for TransitionDeclaration<G> {
    /// Declarations compare by their authored facts: target, timing and
    /// color-interpolation fields, value type, and capture kind. The captured
    /// function pointers are implied by the value type and capture kind, so
    /// they do not participate (function-pointer identity is unreliable).
    fn eq(&self, other: &Self) -> bool {
        self.target == other.target
            && self.duration == other.duration
            && self.delay == other.delay
            && self.timing == other.timing
            && self.color_interpolation == other.color_interpolation
            && self.value_type == other.value_type
            && self.kind == other.kind
    }
}

fn lerp_erased<T: Interpolate + 'static>(
    from: &ErasedValue,
    to: &ErasedValue,
    progress: f64,
    _interpolation: ColorInterpolation,
) -> Option<ErasedValue> {
    let from = from.downcast_ref::<T>()?;
    let to = to.downcast_ref::<T>()?;
    Some(ErasedValue::new(from.interpolate(to, progress)))
}

fn lerp_color(
    from: &ErasedValue,
    to: &ErasedValue,
    progress: f64,
    interpolation: ColorInterpolation,
) -> Option<ErasedValue> {
    let from = from.downcast_ref::<Color>()?;
    let to = to.downcast_ref::<Color>()?;
    Some(ErasedValue::new(
        interpolation.interpolate(*from, *to, progress),
    ))
}

fn eq_erased<T: PartialEq + 'static>(a: &ErasedValue, b: &ErasedValue) -> bool {
    match (a.downcast_ref::<T>(), b.downcast_ref::<T>()) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// An ordered list of per-target transition declarations, usable as a
/// dependency-property value.
///
/// The intended pattern follows CSS: hosts register a "Transitions" property
/// whose value type is `TransitionDeclarations`, author values in style
/// rules, and apply whichever value wins the cascade. The winning value
/// replaces the whole set — declarations are **not merged across rules**,
/// just as `transition` declarations in CSS replace rather than accumulate.
///
/// When the same target is declared more than once, the **last** entry wins,
/// matching how repeated `transition-property` items behave in CSS.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransitionDeclarations<G = PropertyId> {
    entries: Vec<TransitionDeclaration<G>>,
}

impl<G> TransitionDeclarations<G> {
    /// Creates an empty declaration set.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Appends a declaration.
    pub fn push(&mut self, declaration: TransitionDeclaration<G>) {
        self.entries.push(declaration);
    }

    /// Appends a declaration, builder style.
    #[must_use]
    pub fn with(mut self, declaration: TransitionDeclaration<G>) -> Self {
        self.entries.push(declaration);
        self
    }

    /// Returns the number of declarations, counting duplicates.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no transitions are declared.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the declarations in authored order.
    pub fn iter(&self) -> core::slice::Iter<'_, TransitionDeclaration<G>> {
        self.entries.iter()
    }
}

impl<G: PartialEq> TransitionDeclarations<G> {
    /// Returns the winning declaration for `target` — the last matching
    /// entry, if any.
    #[must_use]
    pub fn find(&self, target: &G) -> Option<&TransitionDeclaration<G>> {
        self.entries
            .iter()
            .rev()
            .find(|declaration| declaration.target == *target)
    }
}

impl TransitionDeclarations<PropertyId> {
    /// Declares a transition for a dependency property.
    ///
    /// This is shorthand for pushing [`TransitionDeclaration::of`].
    pub fn declare<T: Interpolate + PartialEq + 'static>(
        &mut self,
        property: Property<T>,
        timing: TransitionTiming,
    ) {
        self.push(TransitionDeclaration::of(property, timing));
    }

    /// Declares a color transition for a dependency property.
    ///
    /// This is shorthand for pushing [`TransitionDeclaration::of_color`].
    pub fn declare_color(&mut self, property: Property<Color>, timing: TransitionTiming) {
        self.push(TransitionDeclaration::of_color(property, timing));
    }
}

impl<'a, G> IntoIterator for &'a TransitionDeclarations<G> {
    type Item = &'a TransitionDeclaration<G>;
    type IntoIter = core::slice::Iter<'a, TransitionDeclaration<G>>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use understory_motion::millis;
    use understory_property::{PropertyMetadataBuilder, PropertyRegistry};

    fn setup() -> (PropertyRegistry, Property<f64>, Property<Color>) {
        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0.0_f64).build());
        let fill = registry.register(
            "Fill",
            PropertyMetadataBuilder::new(Color::TRANSPARENT).build(),
        );
        (registry, width, fill)
    }

    #[test]
    fn timing_builder_and_combined_duration() {
        let timing = TransitionTiming::new(millis(100))
            .with_delay(millis(20))
            .with_timing(TimingFunction::LINEAR);

        assert_eq!(timing.duration(), millis(100));
        assert_eq!(timing.delay(), millis(20));
        assert_eq!(timing.timing(), TimingFunction::LINEAR);
        assert_eq!(timing.combined_duration(), millis(120));
    }

    #[test]
    fn declarations_find_returns_last_match() {
        let (_, width, _) = setup();
        let mut declarations = TransitionDeclarations::new();
        declarations.declare(width, TransitionTiming::new(millis(100)));
        declarations.declare(width, TransitionTiming::new(millis(300)));

        let winner = declarations.find(&width.id()).unwrap();
        assert_eq!(
            winner.resolve_timing(&NoTransitionResources).duration(),
            millis(300)
        );
        assert_eq!(declarations.len(), 2);
    }

    #[test]
    fn declarations_compare_by_authored_facts() {
        let (_, width, fill) = setup();

        let a = TransitionDeclarations::new()
            .with(TransitionDeclaration::of(width, TransitionTiming::new(100)));
        let b = TransitionDeclarations::new()
            .with(TransitionDeclaration::of(width, TransitionTiming::new(100)));
        let c = TransitionDeclarations::new()
            .with(TransitionDeclaration::of(width, TransitionTiming::new(200)));

        assert_eq!(a, b);
        assert_ne!(a, c);

        // A color capture and a plain `Interpolate` capture of the same value
        // type are different declarations.
        let plain = TransitionDeclarations::new().with(TransitionDeclaration::of::<Color>(
            fill,
            TransitionTiming::new(100),
        ));
        let color = TransitionDeclarations::new().with(TransitionDeclaration::of_color(
            fill,
            TransitionTiming::new(100),
        ));
        assert_ne!(plain, color);
    }

    #[test]
    fn late_bound_fields_resolve_against_resources() {
        struct Motion;
        impl TransitionResources for Motion {
            fn duration(&self, key: TransitionResourceKey) -> Option<TimerDuration> {
                (key == TransitionResourceKey::new(7)).then_some(millis(400))
            }
            fn timing_function(&self, key: TransitionResourceKey) -> Option<TimingFunction> {
                (key == TransitionResourceKey::new(8))
                    .then_some(TimingFunction::cubic_bezier(0.0, 0.0, 0.58, 1.0))
            }
        }

        let (_, width, _) = setup();
        let declaration = TransitionDeclaration::of(width, TransitionTiming::new(millis(100)))
            .with_duration_resource(TransitionResourceKey::new(7))
            .with_timing_resource(TransitionResourceKey::new(8));

        let resolved = declaration.resolve_timing(&Motion);
        assert_eq!(resolved.duration(), millis(400));
        assert_eq!(resolved.delay(), 0);
        assert_eq!(
            resolved.timing(),
            TimingFunction::cubic_bezier(0.0, 0.0, 0.58, 1.0)
        );

        // Missing resources fall back to initial values.
        let missing = declaration
            .with_duration_resource(TransitionResourceKey::new(99))
            .with_timing_resource(TransitionResourceKey::new(99))
            .resolve_timing(&Motion);
        assert_eq!(missing.duration(), 0);
        assert_eq!(missing.timing(), TimingFunction::LINEAR);

        assert_eq!(
            declaration.resolve_color_interpolation(&NoTransitionResources),
            ColorInterpolation::default()
        );
    }

    #[test]
    fn generic_target_keys_are_supported() {
        #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
        enum VisualKey {
            OffsetX,
            OffsetY,
        }

        let declarations = TransitionDeclarations::new()
            .with(TransitionDeclaration::for_target::<f64>(
                VisualKey::OffsetX,
                TransitionTiming::new(millis(100)),
            ))
            .with(TransitionDeclaration::for_target::<f64>(
                VisualKey::OffsetY,
                TransitionTiming::new(millis(200)),
            ));

        let x = declarations.find(&VisualKey::OffsetX).unwrap();
        assert_eq!(x.target(), VisualKey::OffsetX);
        assert_eq!(
            x.resolve_timing(&NoTransitionResources).duration(),
            millis(100)
        );
        assert_eq!(x.value_type(), TypeId::of::<f64>());
    }

    #[test]
    fn color_capture_honors_declared_interpolation() {
        let (_, _, fill) = setup();
        let declaration = TransitionDeclaration::of_color(fill, TransitionTiming::new(millis(100)))
            .with_color_interpolation(ColorInterpolation::OKLAB);

        let from = ErasedValue::new(Color::BLACK);
        let to = ErasedValue::new(Color::WHITE);
        let mid = (declaration.lerp())(&from, &to, 0.5, ColorInterpolation::OKLAB).unwrap();
        let expected = ColorInterpolation::OKLAB.interpolate(Color::BLACK, Color::WHITE, 0.5);
        assert_eq!(mid.downcast_ref::<Color>(), Some(&expected));
    }
}
