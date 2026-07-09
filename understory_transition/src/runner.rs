// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::any::TypeId;

use understory_motion::{ColorInterpolation, TimingFunction, normalized_progress};
use understory_property::{ErasedValue, PropertyId, PropertyStore};
use understory_timing::{TimerDuration, TimerInstant};

use crate::declarations::{EqFn, LerpFn, TransitionDeclarations, TransitionResources};

/// Type mismatch between a transition declaration and a supplied value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionTypeMismatch {
    /// The value type captured by the transition declaration.
    pub expected: TypeId,
    /// The value type carried by the supplied [`ErasedValue`].
    pub actual: TypeId,
}

/// What [`TransitionRunner::set_target`] decided to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOutcome {
    /// Nothing to do: the runner was idle and the endpoint values are equal,
    /// or an in-flight transition already heads to the same target.
    Unchanged,
    /// No transition runs; the base value shows immediately. If an in-flight
    /// transition was interrupted, the returned [`TargetUpdate`] carries a
    /// [`TransitionWriteOp::Clear`] write.
    Snapped,
    /// A new transition started from the runner's idle state.
    Started,
    /// An in-flight transition was replaced, continuing from its current
    /// animated value.
    Retargeted,
}

/// The result of reporting a changed transition target.
///
/// This is `#[must_use]` because the immediate write is part of the state
/// change. Apply the write returned by [`Self::into_immediate_write`] before
/// presenting the update; it is not re-emitted by a same-instant
/// [`TransitionRunner::sample`] call.
#[must_use = "apply the immediate transition write before presenting the update"]
#[derive(Debug, Clone)]
pub struct TargetUpdate<K, G = PropertyId> {
    outcome: TargetOutcome,
    immediate_write: Option<TransitionWrite<K, G>>,
}

impl<K, G> TargetUpdate<K, G> {
    fn new(outcome: TargetOutcome, immediate_write: Option<TransitionWrite<K, G>>) -> Self {
        Self {
            outcome,
            immediate_write,
        }
    }

    /// Returns what [`TransitionRunner::set_target`] decided to do.
    #[must_use]
    pub fn outcome(&self) -> TargetOutcome {
        self.outcome
    }

    /// Returns the write that must be applied before presenting this update.
    ///
    /// `None` means no transition write is needed: the target was unchanged,
    /// snapped from idle, or otherwise had no animation-slot state to clear.
    #[must_use]
    pub fn immediate_write(&self) -> Option<&TransitionWrite<K, G>> {
        self.immediate_write.as_ref()
    }

    /// Consumes the update, returning its immediate write.
    #[must_use = "apply this write before presenting the update"]
    pub fn into_immediate_write(self) -> Option<TransitionWrite<K, G>> {
        self.immediate_write
    }

    /// Consumes the update, returning both parts.
    #[must_use = "apply the returned write before presenting the update"]
    pub fn into_parts(self) -> (TargetOutcome, Option<TransitionWrite<K, G>>) {
        (self.outcome, self.immediate_write)
    }
}

/// One animation-slot write for `(owner, target)`.
#[must_use = "apply this transition write before presenting the update"]
#[derive(Debug, Clone)]
pub struct TransitionWrite<K, G = PropertyId> {
    owner: K,
    target: G,
    op: TransitionWriteOp,
}

/// The operation a [`TransitionWrite`] performs on the animation slot.
#[derive(Debug, Clone)]
pub enum TransitionWriteOp {
    /// Write this sampled value into the animation slot.
    Set(ErasedValue),
    /// Clear the animation slot so the base/styled value shows through.
    Clear,
}

impl<K, G> TransitionWrite<K, G> {
    fn set(owner: K, target: G, value: ErasedValue) -> Self {
        Self {
            owner,
            target,
            op: TransitionWriteOp::Set(value),
        }
    }

    fn clear(owner: K, target: G) -> Self {
        Self {
            owner,
            target,
            op: TransitionWriteOp::Clear,
        }
    }
}

impl<K: Copy, G: Copy> TransitionWrite<K, G> {
    /// Returns the owner key this write targets.
    #[must_use]
    pub fn owner(&self) -> K {
        self.owner
    }

    /// Returns the target key this write targets.
    #[must_use]
    pub fn target(&self) -> G {
        self.target
    }

    /// Returns the operation to perform.
    #[must_use]
    pub fn op(&self) -> &TransitionWriteOp {
        &self.op
    }

    /// Consumes the write, returning its operation.
    #[must_use]
    pub fn into_op(self) -> TransitionWriteOp {
        self.op
    }
}

impl<K: Copy + Eq> TransitionWrite<K, PropertyId> {
    /// Applies this write to the animation slot of `store`.
    ///
    /// This is the dependency-property convenience for the common case where
    /// targets are [`PropertyId`]s. The store must be the one owned by
    /// [`Self::owner`]; hosts with many objects route each write by its owner
    /// key first.
    pub fn apply(self, store: &mut PropertyStore<K>) {
        debug_assert!(
            store.owner() == self.owner,
            "transition write applied to a store with a different owner"
        );
        match self.op {
            TransitionWriteOp::Set(value) => store.set_animation_erased(self.target, value),
            TransitionWriteOp::Clear => {
                store.clear_animation_erased(self.target);
            }
        }
    }
}

/// The phase of an in-flight transition, as reported by
/// [`TransitionRunner::active`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionPhase {
    /// The delay has not elapsed; the transition holds its `from` value.
    Delaying,
    /// Interpolation is running toward the target.
    Running,
    /// Interpolation is running, and this transition was created as a
    /// reversal of the one it replaced (its duration was scaled by the
    /// reversing shortening factor).
    Reversing,
}

/// A read-only snapshot of one in-flight transition at a sample instant.
///
/// Produced by [`TransitionRunner::active`] for diagnostics, inspectors, and
/// tests, in the spirit of the workspace's provenance and report types.
#[derive(Debug)]
pub struct ActiveTransitionInfo<'a, K, G> {
    owner: K,
    target: G,
    phase: TransitionPhase,
    progress: f64,
    from: &'a ErasedValue,
    to: &'a ErasedValue,
    current: ErasedValue,
    reversing_shortening: f64,
    start: TimerInstant,
    duration: TimerDuration,
}

impl<K: Copy, G: Copy> ActiveTransitionInfo<'_, K, G> {
    /// Returns the owner key.
    #[must_use]
    pub fn owner(&self) -> K {
        self.owner
    }

    /// Returns the target key.
    #[must_use]
    pub fn target(&self) -> G {
        self.target
    }

    /// Returns the phase at the snapshot instant.
    #[must_use]
    pub fn phase(&self) -> TransitionPhase {
        self.phase
    }

    /// Returns normalized time progress in `0.0..=1.0` (zero while
    /// delaying), before easing is applied.
    #[must_use]
    pub fn progress(&self) -> f64 {
        self.progress
    }

    /// Returns the value the transition started from.
    #[must_use]
    pub fn from(&self) -> &ErasedValue {
        self.from
    }

    /// Returns the value the transition heads toward.
    #[must_use]
    pub fn to(&self) -> &ErasedValue {
        self.to
    }

    /// Returns the eased value at the snapshot instant.
    #[must_use]
    pub fn current(&self) -> &ErasedValue {
        &self.current
    }

    /// Returns the composed reversing shortening factor in `0.0..=1.0`
    /// (`1.0` for transitions that are not reversals).
    #[must_use]
    pub fn reversing_shortening(&self) -> f64 {
        self.reversing_shortening
    }

    /// Returns the first instant of interpolation (creation time plus delay).
    #[must_use]
    pub fn start(&self) -> TimerInstant {
        self.start
    }

    /// Returns the possibly-shortened interpolation duration.
    #[must_use]
    pub fn duration(&self) -> TimerDuration {
        self.duration
    }

    /// Returns the instant the transition reaches its target.
    #[must_use]
    pub fn end_time(&self) -> TimerInstant {
        self.start.saturating_add(self.duration)
    }
}

/// One in-flight transition.
#[derive(Debug)]
struct ActiveTransition<K, G> {
    owner: K,
    target: G,
    from: ErasedValue,
    to: ErasedValue,
    /// The value compared against a new target to detect a reversal; the end
    /// value of the transition this one reversed, or `from` otherwise.
    reversing_adjusted_start: ErasedValue,
    /// CSS reversing shortening factor in `0.0..=1.0`.
    reversing_shortening: f64,
    /// Whether this transition was created as a reversal.
    reversed: bool,
    /// First instant of interpolation (creation time plus delay).
    start: TimerInstant,
    duration: TimerDuration,
    timing: TimingFunction,
    /// Color-interpolation policy resolved at transition start; consulted
    /// only by color-aware captures.
    color_interpolation: ColorInterpolation,
    value_type: TypeId,
    lerp: LerpFn,
    eq: EqFn,
    /// Instant of the most recent returned write. Used to make the immediate
    /// write from `set_target` idempotent with a same-instant `sample`.
    last_write: Option<TimerInstant>,
}

impl<K, G> ActiveTransition<K, G> {
    /// Returns normalized time progress at `now`; zero during the delay.
    fn time_progress(&self, now: TimerInstant) -> f64 {
        if now < self.start {
            return 0.0;
        }
        normalized_progress(now, self.start, self.duration)
    }

    /// Returns eased progress at `now`; `0.0`-input easing during the delay.
    fn eased_progress(&self, now: TimerInstant) -> f64 {
        self.timing.sample(self.time_progress(now))
    }

    fn current_value(&self, now: TimerInstant) -> ErasedValue {
        if now < self.start {
            return self.from.clone();
        }
        (self.lerp)(
            &self.from,
            &self.to,
            self.eased_progress(now),
            self.color_interpolation,
        )
        .expect("transition endpoints share the declared value type")
    }

    fn phase(&self, now: TimerInstant) -> TransitionPhase {
        if now < self.start {
            TransitionPhase::Delaying
        } else if self.reversed {
            TransitionPhase::Reversing
        } else {
            TransitionPhase::Running
        }
    }

    fn is_complete(&self, now: TimerInstant) -> bool {
        now >= self.start && now.saturating_sub(self.start) >= self.duration
    }

    fn end_time(&self) -> TimerInstant {
        self.start.saturating_add(self.duration)
    }
}

/// Retained transition state for many `(owner, target)` pairs.
///
/// The runner owns the retargeting/interruption state machine. Hosts report
/// resolved-value changes through [`Self::set_target`] and drive sampling
/// with host-provided monotonic time through [`Self::sample`]; the runner
/// never reads a clock and never schedules work.
///
/// `K` identifies the owning object and `G` identifies the animated target on
/// that object. `G` defaults to [`PropertyId`] for dependency properties, but
/// hosts may use their own key type for presentation-only targets that are
/// deliberately not dependency properties (layout-derived offsets, additive
/// visual nudges).
#[derive(Debug)]
pub struct TransitionRunner<K, G = PropertyId> {
    /// In-flight transitions, sorted by `(owner, target)`.
    active: Vec<ActiveTransition<K, G>>,
}

impl<K, G> Default for TransitionRunner<K, G> {
    fn default() -> Self {
        Self { active: Vec::new() }
    }
}

impl<K: Copy + Ord, G: Copy + Ord> TransitionRunner<K, G> {
    /// Creates an empty runner.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reports that the resolved value for `(owner, target)` changed.
    ///
    /// `declarations` is the transition declaration set that currently
    /// applies to the owner — with styled transitions, the value of the
    /// owner's "Transitions" property from the *after-change* resolution.
    /// Late-bound declaration fields are resolved against `resources` now, at
    /// transition start, matching CSS Transitions, which resolves transition
    /// parameters from the after-change style at the moment a transition
    /// begins.
    ///
    /// `before` is the value that was presented until now — the previously
    /// resolved base value. It seeds the transition only when the runner is
    /// idle for this target; if a transition is already in flight, the new
    /// transition starts from the *current animated value* instead, so an
    /// interruption never snaps or restarts from the old base.
    ///
    /// `after` is the newly resolved target. The host is expected to have
    /// already applied it to the base/styled layer; the transition masks the
    /// jump through the animation slot until it completes.
    ///
    /// If the new target reverses an in-flight transition (it equals that
    /// transition's reversing-adjusted start value), the replacement's
    /// duration is scaled by the CSS reversing shortening factor so symmetric
    /// reversals (e.g. hover-off during hover-on) take proportionally less
    /// time.
    ///
    /// The returned [`TargetUpdate`] may carry an immediate write. Apply it
    /// before presenting the update that changed the target. That write is
    /// not emitted again by a same-instant [`Self::sample`] call.
    ///
    /// No transition runs when `declarations` has no entry for `target` or
    /// the resolved combined duration (duration plus delay) is zero.
    ///
    /// # Errors
    ///
    /// Returns [`TransitionTypeMismatch`] when `before` or `after` does not
    /// carry the declaration's value type. The runner is left unchanged.
    pub fn set_target<R: TransitionResources + ?Sized>(
        &mut self,
        owner: K,
        target: G,
        declarations: &TransitionDeclarations<G>,
        resources: &R,
        before: &ErasedValue,
        after: ErasedValue,
        now: TimerInstant,
    ) -> Result<TargetUpdate<K, G>, TransitionTypeMismatch> {
        match self.find(owner, target) {
            Ok(index) => self.retarget_active(index, declarations, resources, after, now),
            Err(index) => self.start_idle(
                index,
                owner,
                target,
                declarations,
                resources,
                before,
                after,
                now,
            ),
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "internal helper mirroring set_target's surface"
    )]
    fn start_idle<R: TransitionResources + ?Sized>(
        &mut self,
        index: usize,
        owner: K,
        target: G,
        declarations: &TransitionDeclarations<G>,
        resources: &R,
        before: &ErasedValue,
        after: ErasedValue,
        now: TimerInstant,
    ) -> Result<TargetUpdate<K, G>, TransitionTypeMismatch> {
        let Some(declaration) = declarations.find(&target) else {
            return Ok(TargetUpdate::new(TargetOutcome::Snapped, None));
        };
        check_type(declaration.value_type(), before)?;
        check_type(declaration.value_type(), &after)?;

        if (declaration.eq())(before, &after) {
            return Ok(TargetUpdate::new(TargetOutcome::Unchanged, None));
        }
        let timing = declaration.resolve_timing(resources);
        if timing.combined_duration() == 0 {
            return Ok(TargetUpdate::new(TargetOutcome::Snapped, None));
        }

        let from = before.clone();
        let immediate_write = TransitionWrite::set(owner, target, from.clone());
        self.active.insert(
            index,
            ActiveTransition {
                owner,
                target,
                from: from.clone(),
                to: after,
                reversing_adjusted_start: from,
                reversing_shortening: 1.0,
                reversed: false,
                start: now.saturating_add(timing.delay()),
                duration: timing.duration(),
                timing: timing.timing(),
                color_interpolation: declaration.resolve_color_interpolation(resources),
                value_type: declaration.value_type(),
                lerp: declaration.lerp(),
                eq: declaration.eq(),
                last_write: Some(now),
            },
        );
        Ok(TargetUpdate::new(
            TargetOutcome::Started,
            Some(immediate_write),
        ))
    }

    fn retarget_active<R: TransitionResources + ?Sized>(
        &mut self,
        index: usize,
        declarations: &TransitionDeclarations<G>,
        resources: &R,
        after: ErasedValue,
        now: TimerInstant,
    ) -> Result<TargetUpdate<K, G>, TransitionTypeMismatch> {
        {
            let active = &self.active[index];
            check_type(active.value_type, &after)?;
            if (active.eq)(&active.to, &after) {
                return Ok(TargetUpdate::new(TargetOutcome::Unchanged, None));
            }
            if let Some(declaration) = declarations.find(&active.target) {
                check_type(declaration.value_type(), &after)?;
            }
        }

        let old = self.active.remove(index);
        let current = old.current_value(now);

        // The new target equals the value on screen: nothing to animate.
        if (old.eq)(&current, &after) {
            return Ok(TargetUpdate::new(
                TargetOutcome::Snapped,
                Some(TransitionWrite::clear(old.owner, old.target)),
            ));
        }

        // The declaration may have been removed since the old transition
        // started; treat that as "no transition" and reveal the base.
        let Some(declaration) = declarations.find(&old.target) else {
            return Ok(TargetUpdate::new(
                TargetOutcome::Snapped,
                Some(TransitionWrite::clear(old.owner, old.target)),
            ));
        };

        // CSS Transitions "faster reversing of interrupted transitions": when
        // the new target reverses the old transition, shorten the replacement
        // by how far the old one actually travelled.
        let reversing = (declaration.eq())(&old.reversing_adjusted_start, &after);
        let shortening = if reversing {
            clamp01(abs(old.eased_progress(now) * old.reversing_shortening
                + (1.0 - old.reversing_shortening)))
        } else {
            1.0
        };

        // Replacement parameters resolve now — this retarget is the start of
        // a new transition.
        let timing = declaration.resolve_timing(resources);
        let duration = scale_duration(timing.duration(), shortening);
        if duration.saturating_add(timing.delay()) == 0 {
            return Ok(TargetUpdate::new(
                TargetOutcome::Snapped,
                Some(TransitionWrite::clear(old.owner, old.target)),
            ));
        }

        let immediate_write = TransitionWrite::set(old.owner, old.target, current.clone());
        self.active.insert(
            index,
            ActiveTransition {
                owner: old.owner,
                target: old.target,
                reversing_adjusted_start: if reversing { old.to } else { current.clone() },
                from: current,
                to: after,
                reversing_shortening: shortening,
                reversed: reversing,
                start: now.saturating_add(timing.delay()),
                duration,
                timing: timing.timing(),
                color_interpolation: declaration.resolve_color_interpolation(resources),
                value_type: declaration.value_type(),
                lerp: declaration.lerp(),
                eq: declaration.eq(),
                last_write: Some(now),
            },
        );
        Ok(TargetUpdate::new(
            TargetOutcome::Retargeted,
            Some(immediate_write),
        ))
    }

    /// Samples every transition at `now`.
    ///
    /// Each in-flight transition yields a [`TransitionWriteOp::Set`] with its
    /// current value (the `from` value while its delay elapses). A completed
    /// transition yields [`TransitionWriteOp::Clear`] and is removed, so the
    /// already-updated base/styled value shows through.
    ///
    /// A write already returned by [`Self::set_target`] for the same `now`
    /// instant is not emitted again here.
    #[must_use = "apply returned transition writes before presenting the sampled frame"]
    pub fn sample(&mut self, now: TimerInstant) -> Vec<TransitionWrite<K, G>> {
        let mut out = Vec::new();
        self.active.retain_mut(|transition| {
            if transition.is_complete(now) {
                out.push(TransitionWrite::clear(transition.owner, transition.target));
                return false;
            }
            if transition.last_write == Some(now) {
                return true;
            }
            out.push(TransitionWrite::set(
                transition.owner,
                transition.target,
                transition.current_value(now),
            ));
            transition.last_write = Some(now);
            true
        });
        out
    }

    /// Returns read-only snapshots of every in-flight transition at `now`,
    /// ordered by `(owner, target)`.
    pub fn active(
        &self,
        now: TimerInstant,
    ) -> impl Iterator<Item = ActiveTransitionInfo<'_, K, G>> {
        self.active
            .iter()
            .map(move |transition| ActiveTransitionInfo {
                owner: transition.owner,
                target: transition.target,
                phase: transition.phase(now),
                progress: transition.time_progress(now),
                from: &transition.from,
                to: &transition.to,
                current: transition.current_value(now),
                reversing_shortening: transition.reversing_shortening,
                start: transition.start,
                duration: transition.duration,
            })
    }

    /// Cancels the transition for `(owner, target)`.
    ///
    /// Returns a clear write when a transition was in flight.
    #[must_use = "apply the returned clear write before presenting the update"]
    pub fn cancel(&mut self, owner: K, target: G) -> Option<TransitionWrite<K, G>> {
        if let Ok(index) = self.find(owner, target) {
            self.active.remove(index);
            Some(TransitionWrite::clear(owner, target))
        } else {
            None
        }
    }

    /// Cancels every transition for `owner`.
    ///
    /// Returns the clear writes needed to reveal the owner's base values.
    #[must_use = "apply the returned clear writes before presenting the update"]
    pub fn cancel_owner(&mut self, owner: K) -> Vec<TransitionWrite<K, G>> {
        let mut clears = Vec::new();
        self.active.retain(|transition| {
            if transition.owner == owner {
                clears.push(TransitionWrite::clear(transition.owner, transition.target));
                false
            } else {
                true
            }
        });
        clears
    }

    /// Drops every transition for `owner` without emitting clear writes.
    ///
    /// Use this when the owner's store is being destroyed.
    ///
    /// Returns the number of transitions dropped.
    pub fn discard_owner(&mut self, owner: K) -> usize {
        let before = self.active.len();
        self.active.retain(|transition| transition.owner != owner);
        before - self.active.len()
    }

    /// Returns `true` if no transitions are in flight.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.active.is_empty()
    }

    /// Returns the number of in-flight transitions.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// Returns `true` if a transition is in flight for `(owner, target)`.
    #[must_use]
    pub fn has_transition(&self, owner: K, target: G) -> bool {
        self.find(owner, target).is_ok()
    }

    /// Returns the target value of the in-flight transition for
    /// `(owner, target)`, if any.
    #[must_use]
    pub fn target(&self, owner: K, target: G) -> Option<&ErasedValue> {
        self.find(owner, target)
            .ok()
            .map(|index| &self.active[index].to)
    }

    /// Returns the animated value for `(owner, target)` at `now`, if a
    /// transition is in flight.
    #[must_use]
    pub fn current_value(&self, owner: K, target: G, now: TimerInstant) -> Option<ErasedValue> {
        self.find(owner, target)
            .ok()
            .map(|index| self.active[index].current_value(now))
    }

    /// Returns the earliest instant at which an in-flight transition
    /// completes.
    ///
    /// Use [`Self::is_idle`] to decide whether all transition state has been
    /// drained.
    #[must_use]
    pub fn next_end_time(&self) -> Option<TimerInstant> {
        self.active.iter().map(ActiveTransition::end_time).min()
    }

    fn find(&self, owner: K, target: G) -> Result<usize, usize> {
        self.active
            .binary_search_by_key(&(owner, target), |transition| {
                (transition.owner, transition.target)
            })
    }
}

fn check_type(expected: TypeId, value: &ErasedValue) -> Result<(), TransitionTypeMismatch> {
    let actual = value.type_id();
    if actual == expected {
        Ok(())
    } else {
        Err(TransitionTypeMismatch { expected, actual })
    }
}

fn abs(value: f64) -> f64 {
    if value < 0.0 { -value } else { value }
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

/// Scales a duration by a factor in `0.0..=1.0`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "factor is in 0.0..=1.0, so the product fits the original integer range"
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "sub-tick rounding of a shortened duration is imperceptible"
)]
fn scale_duration(duration: TimerDuration, factor: f64) -> TimerDuration {
    if factor >= 1.0 {
        return duration;
    }
    (duration as f64 * factor) as TimerDuration
}
