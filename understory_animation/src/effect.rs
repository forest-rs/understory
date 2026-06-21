// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::cmp::Ordering;

use understory_motion::AnimatableValue;

/// How an effect sample combines with the value beneath it in a target stack.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositeOperation {
    /// Replace the accumulated value with this effect's sample.
    Replace,
    /// Add this effect's sample to the accumulated value.
    Add,
    /// Accumulate this effect's sample into the accumulated value.
    Accumulate,
}

/// One keyframe value at a normalized effect offset.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe<T> {
    /// Normalized keyframe offset.
    pub offset: f64,
    /// Value at `offset`.
    pub value: T,
}

impl<T> Keyframe<T> {
    /// Creates a keyframe.
    #[must_use]
    pub const fn new(offset: f64, value: T) -> Self {
        Self { offset, value }
    }
}

/// Typed keyframe effect.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyframeEffect<T> {
    keyframes: Vec<Keyframe<T>>,
    composite: CompositeOperation,
}

impl<T> KeyframeEffect<T> {
    /// Creates a replacement keyframe effect from keyframes.
    ///
    /// Keyframes are stored in ascending offset order.
    #[must_use]
    pub fn new(mut keyframes: Vec<Keyframe<T>>) -> Self {
        debug_assert!(
            keyframes.iter().all(|keyframe| keyframe.offset.is_finite()),
            "keyframe offsets must be finite"
        );
        keyframes.sort_by(|a, b| a.offset.partial_cmp(&b.offset).unwrap_or(Ordering::Equal));
        Self {
            keyframes,
            composite: CompositeOperation::Replace,
        }
    }

    /// Creates an effect from evenly-spaced values.
    #[must_use]
    pub fn from_values(values: Vec<T>) -> Self {
        let count = values.len();
        if count <= 1 {
            return Self::new(
                values
                    .into_iter()
                    .map(|value| Keyframe::new(0.0, value))
                    .collect(),
            );
        }

        let last = (count - 1) as f64;
        Self::new(
            values
                .into_iter()
                .enumerate()
                .map(|(index, value)| Keyframe::new(index as f64 / last, value))
                .collect(),
        )
    }

    /// Sets the composite operation for this effect.
    #[must_use]
    pub const fn with_composite(mut self, composite: CompositeOperation) -> Self {
        self.composite = composite;
        self
    }

    /// Returns this effect's composite operation.
    #[must_use]
    pub const fn composite(&self) -> CompositeOperation {
        self.composite
    }

    /// Returns this effect's keyframes.
    #[must_use]
    pub fn keyframes(&self) -> &[Keyframe<T>] {
        &self.keyframes
    }
}

impl<T: AnimatableValue> KeyframeEffect<T> {
    /// Samples this effect at normalized progress `progress`.
    #[must_use]
    pub fn sample_at(&self, progress: f64) -> Option<T> {
        let [first] = self.keyframes.as_slice() else {
            let progress = progress.clamp(0.0, 1.0);
            return self.sample_many(progress);
        };
        Some(first.value.clone())
    }

    fn sample_many(&self, progress: f64) -> Option<T> {
        let first = self.keyframes.first()?;
        if progress <= first.offset {
            return Some(first.value.clone());
        }

        let last = self.keyframes.last()?;
        if progress >= last.offset {
            return Some(last.value.clone());
        }

        for pair in self.keyframes.windows(2) {
            let from = &pair[0];
            let to = &pair[1];
            if progress < from.offset || progress > to.offset {
                continue;
            }
            let span = to.offset - from.offset;
            if span <= f64::EPSILON {
                return Some(to.value.clone());
            }
            let local = (progress - from.offset) / span;
            return Some(from.value.interpolate(&to.value, local));
        }

        Some(last.value.clone())
    }
}
