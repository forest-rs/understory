// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::any::TypeId;

use crate::endpoint::{BindingId, EndpointKey};
use crate::report::BindingReport;

/// Error produced while registering or evaluating bindings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingError<K> {
    /// The binding would directly bind an endpoint to itself.
    SelfBinding {
        /// The endpoint used as both source and target.
        endpoint: EndpointKey<K>,
    },
    /// Adding the binding would introduce a cycle in the binding dependency graph.
    Cycle {
        /// The binding that would depend on another binding.
        dependent: BindingId,
        /// The binding that would be depended on.
        dependency: BindingId,
    },
    /// The binding set has reached the maximum representable binding id.
    TooManyBindings,
    /// Another active binding already writes this target endpoint.
    TargetAlreadyBound {
        /// The target endpoint that already has a writer.
        target: EndpointKey<K>,
        /// The existing writer binding.
        existing: BindingId,
    },
    /// The host did not provide a source value for the endpoint.
    ///
    /// [`BindingSet::drain`] treats this as a non-fatal pending state: it
    /// skips the binding and counts it in
    /// [`BindingReport::skipped_missing_source`]. Other callers of a binding
    /// evaluator may still observe this error directly.
    ///
    /// [`BindingSet::drain`]: crate::BindingSet::drain
    /// [`BindingReport::skipped_missing_source`]: crate::BindingReport::skipped_missing_source
    MissingSource {
        /// The binding that failed to read its source.
        binding: BindingId,
        /// The source endpoint that could not be read.
        endpoint: EndpointKey<K>,
    },
    /// The host returned a source value with a different runtime type.
    SourceTypeMismatch {
        /// The binding that read the wrong value type.
        binding: BindingId,
        /// The source endpoint that returned the wrong value type.
        endpoint: EndpointKey<K>,
        /// The type expected by the binding declaration.
        expected: TypeId,
        /// The type returned by the host.
        actual: TypeId,
    },
}

/// Error returned when draining dirty bindings stops before the set is clean.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingDrainError<K> {
    error: BindingError<K>,
    report: BindingReport,
}

impl<K> BindingDrainError<K> {
    /// Creates a drain error from the binding error and partial report.
    #[must_use]
    pub(crate) const fn new(error: BindingError<K>, report: BindingReport) -> Self {
        Self { error, report }
    }

    /// Returns the binding error that stopped the drain.
    #[must_use]
    pub const fn error(&self) -> &BindingError<K> {
        &self.error
    }

    /// Returns the report for writes completed before the error.
    ///
    /// Those writes are not rolled back. Hosts should still apply the affected
    /// channels from this report to their application-level invalidation state.
    #[must_use]
    pub const fn report(&self) -> BindingReport {
        self.report
    }

    /// Consumes the error and returns the binding error and partial report.
    #[must_use]
    pub fn into_parts(self) -> (BindingError<K>, BindingReport) {
        (self.error, self.report)
    }
}
