// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;

use crate::{LaneId, SpanId, TimelineItemKey};

/// A timeline lane.
///
/// The label type defaults to [`String`] through [`TimelineLane::new`]. Large
/// traces can use [`TimelineLane::from_label`] with a compact symbol id or
/// interned handle instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimelineLane<Label = String> {
    /// Stable caller-defined identity for this lane.
    pub key: TimelineItemKey,
    /// User-visible lane label.
    pub label: Label,
}

impl TimelineLane {
    /// Creates a lane with a user-visible label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            key: TimelineItemKey::ANONYMOUS,
            label: label.into(),
        }
    }
}

impl<Label> TimelineLane<Label> {
    /// Creates a lane from an already-resolved label value.
    ///
    /// Use this when labels are compact ids, interned strings, or other
    /// application-defined handles.
    #[must_use]
    pub const fn from_label(label: Label) -> Self {
        Self {
            key: TimelineItemKey::ANONYMOUS,
            label,
        }
    }

    /// Sets the stable caller-defined identity for this lane.
    #[must_use]
    pub const fn with_key(mut self, key: TimelineItemKey) -> Self {
        self.key = key;
        self
    }
}

/// A span item occupying a continuous time interval in one lane.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineSpan<Label = String> {
    /// Stable caller-defined identity for this span.
    pub key: TimelineItemKey,
    /// User-visible span label.
    pub label: Label,
    /// Inclusive start time in caller-defined scalar units.
    pub start: f64,
    /// Exclusive end time in caller-defined scalar units.
    pub end: f64,
    /// Owning lane.
    pub lane: LaneId,
    /// Visual nesting depth or structural sub-level within the lane.
    pub depth: usize,
}

impl TimelineSpan {
    /// Creates a span with zero nesting depth.
    #[must_use]
    pub fn new(label: impl Into<String>, start: f64, end: f64, lane: LaneId) -> Self {
        Self {
            key: TimelineItemKey::ANONYMOUS,
            label: label.into(),
            start,
            end,
            lane,
            depth: 0,
        }
    }
}

impl<Label> TimelineSpan<Label> {
    /// Creates a span from an already-resolved label value.
    ///
    /// Use this when labels are compact ids, interned strings, or other
    /// application-defined handles.
    #[must_use]
    pub const fn from_label(label: Label, start: f64, end: f64, lane: LaneId) -> Self {
        Self {
            key: TimelineItemKey::ANONYMOUS,
            label,
            start,
            end,
            lane,
            depth: 0,
        }
    }

    /// Sets the stable caller-defined identity for this span.
    #[must_use]
    pub const fn with_key(mut self, key: TimelineItemKey) -> Self {
        self.key = key;
        self
    }

    /// Sets the visual nesting depth or structural sub-level.
    #[must_use]
    pub fn with_depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    /// Returns the span duration.
    #[must_use]
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
}

/// An instantaneous marker at a single time position.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineMarker<Label = String> {
    /// Stable caller-defined identity for this marker.
    pub key: TimelineItemKey,
    /// User-visible marker label.
    pub label: Label,
    /// Marker time in caller-defined scalar units.
    pub time: f64,
    /// Optional associated lane. `None` means global/all-lane.
    pub lane: Option<LaneId>,
}

impl TimelineMarker {
    /// Creates a global marker with no lane association.
    #[must_use]
    pub fn new_global(label: impl Into<String>, time: f64) -> Self {
        Self {
            key: TimelineItemKey::ANONYMOUS,
            label: label.into(),
            time,
            lane: None,
        }
    }

    /// Creates a marker associated with a lane.
    #[must_use]
    pub fn new(label: impl Into<String>, time: f64, lane: LaneId) -> Self {
        Self {
            key: TimelineItemKey::ANONYMOUS,
            label: label.into(),
            time,
            lane: Some(lane),
        }
    }
}

impl<Label> TimelineMarker<Label> {
    /// Creates a global marker from an already-resolved label value.
    ///
    /// Use this when labels are compact ids, interned strings, or other
    /// application-defined handles.
    #[must_use]
    pub const fn global_from_label(label: Label, time: f64) -> Self {
        Self {
            key: TimelineItemKey::ANONYMOUS,
            label,
            time,
            lane: None,
        }
    }

    /// Creates a lane-associated marker from an already-resolved label value.
    ///
    /// Use this when labels are compact ids, interned strings, or other
    /// application-defined handles.
    #[must_use]
    pub const fn from_label(label: Label, time: f64, lane: LaneId) -> Self {
        Self {
            key: TimelineItemKey::ANONYMOUS,
            label,
            time,
            lane: Some(lane),
        }
    }

    /// Sets the stable caller-defined identity for this marker.
    #[must_use]
    pub const fn with_key(mut self, key: TimelineItemKey) -> Self {
        self.key = key;
        self
    }
}

/// A relationship between two spans, such as an async handoff or causal flow.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TimelineFlow {
    /// Stable caller-defined identity for this flow.
    pub key: TimelineItemKey,
    /// Source span.
    pub from_span: SpanId,
    /// Destination span.
    pub to_span: SpanId,
}

impl TimelineFlow {
    /// Creates a relationship between two spans.
    #[must_use]
    pub const fn new(from_span: SpanId, to_span: SpanId) -> Self {
        Self {
            key: TimelineItemKey::ANONYMOUS,
            from_span,
            to_span,
        }
    }

    /// Sets the stable caller-defined identity for this flow.
    #[must_use]
    pub const fn with_key(mut self, key: TimelineItemKey) -> Self {
        self.key = key;
        self
    }
}
