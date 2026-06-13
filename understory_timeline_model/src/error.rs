// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

use crate::{FlowId, LaneId, MarkerId, SpanId, TimelineItemKey};

/// Result type returned by timeline validation and edit operations.
pub type TimelineResult<T> = Result<T, TimelineError>;

/// Validation or edit error for timeline documents.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TimelineError {
    /// Two or more content records used the same non-anonymous item key.
    DuplicateItemKey {
        /// Duplicated item key.
        key: TimelineItemKey,
    },
    /// A span start or end time was not finite.
    NonFiniteSpanTime {
        /// Span being validated.
        span: SpanId,
        /// Span start time.
        start: f64,
        /// Span end time.
        end: f64,
    },
    /// A span start time was greater than its end time.
    ReversedSpan {
        /// Span being validated.
        span: SpanId,
        /// Span start time.
        start: f64,
        /// Span end time.
        end: f64,
    },
    /// A span referenced a lane that does not exist.
    UnknownSpanLane {
        /// Span being validated.
        span: SpanId,
        /// Missing lane reference.
        lane: LaneId,
    },
    /// A marker time was not finite.
    NonFiniteMarkerTime {
        /// Marker being validated.
        marker: MarkerId,
        /// Marker time.
        time: f64,
    },
    /// A marker referenced a lane that does not exist.
    UnknownMarkerLane {
        /// Marker being validated.
        marker: MarkerId,
        /// Missing lane reference.
        lane: LaneId,
    },
    /// A span identifier does not exist in the document.
    UnknownSpan {
        /// Missing span reference.
        span: SpanId,
    },
    /// A stable span key does not exist in the document.
    UnknownSpanKey {
        /// Missing stable span key.
        key: TimelineItemKey,
    },
    /// A stable marker key does not exist in the document.
    UnknownMarkerKey {
        /// Missing stable marker key.
        key: TimelineItemKey,
    },
    /// A flow endpoint referenced a span that does not exist.
    UnknownFlowEndpoint {
        /// Flow being validated.
        flow: FlowId,
        /// Missing span reference.
        span: SpanId,
    },
    /// The playhead time was not finite.
    NonFinitePlayhead {
        /// Invalid playhead time.
        playhead: f64,
    },
    /// A selection endpoint was not finite.
    NonFiniteSelection {
        /// Selection start endpoint.
        start: f64,
        /// Selection end endpoint.
        end: f64,
    },
    /// An edit delta was not finite.
    NonFiniteDelta {
        /// Invalid delta value.
        delta: f64,
    },
    /// A minimum duration was negative or not finite.
    InvalidMinimumDuration {
        /// Invalid minimum duration.
        min_duration: f64,
    },
    /// An edit would produce a non-finite span time.
    NonFiniteEditedSpan {
        /// Span being edited.
        span: SpanId,
        /// Edited start time.
        start: f64,
        /// Edited end time.
        end: f64,
    },
    /// An edit would produce a non-finite selection endpoint.
    NonFiniteEditedSelection {
        /// Edited selection start time.
        start: f64,
        /// Edited selection end time.
        end: f64,
    },
}

impl fmt::Display for TimelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::DuplicateItemKey { key } => {
                write!(f, "duplicate timeline item key {}", key.raw())
            }
            Self::NonFiniteSpanTime { span, start, end } => write!(
                f,
                "span {} has non-finite time bounds {start:?}..{end:?}",
                span.index()
            ),
            Self::ReversedSpan { span, start, end } => write!(
                f,
                "span {} starts after it ends: {start:?}..{end:?}",
                span.index()
            ),
            Self::UnknownSpanLane { span, lane } => write!(
                f,
                "span {} references missing lane {}",
                span.index(),
                lane.index()
            ),
            Self::NonFiniteMarkerTime { marker, time } => {
                write!(f, "marker {} has non-finite time {time:?}", marker.index())
            }
            Self::UnknownMarkerLane { marker, lane } => write!(
                f,
                "marker {} references missing lane {}",
                marker.index(),
                lane.index()
            ),
            Self::UnknownSpan { span } => {
                write!(f, "missing span {}", span.index())
            }
            Self::UnknownSpanKey { key } => {
                write!(f, "missing span with item key {}", key.raw())
            }
            Self::UnknownMarkerKey { key } => {
                write!(f, "missing marker with item key {}", key.raw())
            }
            Self::UnknownFlowEndpoint { flow, span } => write!(
                f,
                "flow {} references missing span {}",
                flow.index(),
                span.index()
            ),
            Self::NonFinitePlayhead { playhead } => {
                write!(f, "playhead is non-finite: {playhead:?}")
            }
            Self::NonFiniteSelection { start, end } => {
                write!(f, "selection has non-finite endpoint: {start:?}..{end:?}")
            }
            Self::NonFiniteDelta { delta } => {
                write!(f, "edit delta is non-finite: {delta:?}")
            }
            Self::InvalidMinimumDuration { min_duration } => write!(
                f,
                "minimum duration must be finite and non-negative, got {min_duration:?}"
            ),
            Self::NonFiniteEditedSpan { span, start, end } => write!(
                f,
                "editing span {} would produce non-finite bounds {start:?}..{end:?}",
                span.index()
            ),
            Self::NonFiniteEditedSelection { start, end } => write!(
                f,
                "editing selection would produce non-finite bounds {start:?}..{end:?}"
            ),
        }
    }
}

impl core::error::Error for TimelineError {}
