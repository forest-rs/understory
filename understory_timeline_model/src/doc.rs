// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Range;

use crate::{
    FlowId, LaneId, MarkerId, SpanId, TimelineError, TimelineFlow, TimelineItemKey, TimelineLane,
    TimelineMarker, TimelineResult, TimelineSelection, TimelineSpan,
};

/// A validated headless time-domain document with plain edit/query operations.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineDoc<Label = String> {
    lanes: Vec<TimelineLane<Label>>,
    spans: Vec<TimelineSpan<Label>>,
    flows: Vec<TimelineFlow>,
    markers: Vec<TimelineMarker<Label>>,
    playhead: f64,
    selection: Option<TimelineSelection>,
}

impl<Label> Default for TimelineDoc<Label> {
    fn default() -> Self {
        Self::with_capacity(0, 0, 0, 0)
    }
}

impl<Label> TimelineDoc<Label> {
    /// Creates an empty document with storage reserved for expected record counts.
    ///
    /// This is the preferred construction path for large traces when the caller
    /// already knows the rough number of lanes, spans, markers, and flows.
    #[must_use]
    pub fn with_capacity(
        lane_capacity: usize,
        span_capacity: usize,
        marker_capacity: usize,
        flow_capacity: usize,
    ) -> Self {
        Self {
            lanes: Vec::with_capacity(lane_capacity),
            spans: Vec::with_capacity(span_capacity),
            flows: Vec::with_capacity(flow_capacity),
            markers: Vec::with_capacity(marker_capacity),
            playhead: 0.0,
            selection: None,
        }
    }

    /// Reserves additional storage for records that will be appended later.
    pub fn reserve(
        &mut self,
        additional_lanes: usize,
        additional_spans: usize,
        additional_markers: usize,
        additional_flows: usize,
    ) {
        self.lanes.reserve(additional_lanes);
        self.spans.reserve(additional_spans);
        self.markers.reserve(additional_markers);
        self.flows.reserve(additional_flows);
    }

    /// Creates a validated document with lanes and spans. Other state starts empty.
    pub fn try_new(
        lanes: impl IntoIterator<Item = TimelineLane<Label>>,
        spans: impl IntoIterator<Item = TimelineSpan<Label>>,
    ) -> TimelineResult<Self> {
        Self::try_from_parts(lanes, spans, [], [])
    }

    /// Creates a validated document from all content records.
    ///
    /// The playhead starts at `0.0` and no selection is active.
    pub fn try_from_parts(
        lanes: impl IntoIterator<Item = TimelineLane<Label>>,
        spans: impl IntoIterator<Item = TimelineSpan<Label>>,
        markers: impl IntoIterator<Item = TimelineMarker<Label>>,
        flows: impl IntoIterator<Item = TimelineFlow>,
    ) -> TimelineResult<Self> {
        let lanes: Vec<_> = lanes.into_iter().collect();
        let spans: Vec<_> = spans.into_iter().collect();
        let markers: Vec<_> = markers.into_iter().collect();
        let flows: Vec<_> = flows.into_iter().collect();
        validate_content(&lanes, &spans, &markers, &flows)?;
        Ok(Self {
            lanes,
            spans,
            flows,
            markers,
            playhead: 0.0,
            selection: None,
        })
    }

    /// Replaces lanes, spans, markers, and flows transactionally.
    ///
    /// If validation fails, existing content is left unchanged. Time-range
    /// selections survive replacement. Span and marker selections survive only
    /// when the replacement contains the selected stable item key.
    pub fn replace_content(
        &mut self,
        lanes: impl IntoIterator<Item = TimelineLane<Label>>,
        spans: impl IntoIterator<Item = TimelineSpan<Label>>,
        markers: impl IntoIterator<Item = TimelineMarker<Label>>,
        flows: impl IntoIterator<Item = TimelineFlow>,
    ) -> TimelineResult<()> {
        let lanes: Vec<_> = lanes.into_iter().collect();
        let spans: Vec<_> = spans.into_iter().collect();
        let markers: Vec<_> = markers.into_iter().collect();
        let flows: Vec<_> = flows.into_iter().collect();
        validate_content(&lanes, &spans, &markers, &flows)?;

        self.selection = reconcile_selection(self.selection, &spans, &markers);
        self.lanes = lanes;
        self.spans = spans;
        self.markers = markers;
        self.flows = flows;
        Ok(())
    }

    /// Appends a validated lane and returns its document-local identifier.
    pub fn push_lane(&mut self, lane: TimelineLane<Label>) -> TimelineResult<LaneId> {
        self.validate_available_item_key(lane.key)?;
        let id = LaneId::new(self.lanes.len());
        self.lanes.push(lane);
        Ok(id)
    }

    /// Appends a validated span and returns its document-local identifier.
    pub fn push_span(&mut self, span: TimelineSpan<Label>) -> TimelineResult<SpanId> {
        self.validate_available_item_key(span.key)?;
        let id = SpanId::new(self.spans.len());
        validate_span(id, &span, self.lanes.len())?;
        self.spans.push(span);
        Ok(id)
    }

    /// Appends a validated marker and returns its document-local identifier.
    pub fn push_marker(&mut self, marker: TimelineMarker<Label>) -> TimelineResult<MarkerId> {
        self.validate_available_item_key(marker.key)?;
        let id = MarkerId::new(self.markers.len());
        validate_marker(id, &marker, self.lanes.len())?;
        self.markers.push(marker);
        Ok(id)
    }

    /// Appends a validated flow and returns its document-local identifier.
    pub fn push_flow(&mut self, flow: TimelineFlow) -> TimelineResult<FlowId> {
        self.validate_available_item_key(flow.key)?;
        let id = FlowId::new(self.flows.len());
        validate_flow(id, flow, self.spans.len())?;
        self.flows.push(flow);
        Ok(id)
    }

    /// Replaces the document's flow records transactionally.
    ///
    /// If validation fails, the existing flow records are left unchanged.
    pub fn set_flows(
        &mut self,
        flows: impl IntoIterator<Item = TimelineFlow>,
    ) -> TimelineResult<()> {
        let flows: Vec<_> = flows.into_iter().collect();
        validate_unique_item_keys(&self.lanes, &self.spans, &self.markers, &flows)?;
        validate_flows(&flows, self.spans.len())?;
        self.flows = flows;
        Ok(())
    }

    /// Replaces the document's marker records transactionally.
    ///
    /// If validation fails, the existing marker records are left unchanged.
    pub fn set_markers(
        &mut self,
        markers: impl IntoIterator<Item = TimelineMarker<Label>>,
    ) -> TimelineResult<()> {
        let markers: Vec<_> = markers.into_iter().collect();
        validate_unique_item_keys(&self.lanes, &self.spans, &markers, &self.flows)?;
        validate_markers(&markers, self.lanes.len())?;
        self.selection = reconcile_selection(self.selection, &self.spans, &markers);
        self.markers = markers;
        Ok(())
    }

    fn validate_available_item_key(&self, key: TimelineItemKey) -> TimelineResult<()> {
        if key.is_anonymous() || !self.contains_item_key(key) {
            Ok(())
        } else {
            Err(TimelineError::DuplicateItemKey { key })
        }
    }

    fn contains_item_key(&self, key: TimelineItemKey) -> bool {
        self.lanes.iter().any(|lane| lane.key == key)
            || self.spans.iter().any(|span| span.key == key)
            || self.markers.iter().any(|marker| marker.key == key)
            || self.flows.iter().any(|flow| flow.key == key)
    }

    /// Returns all lanes.
    #[must_use]
    pub fn lanes(&self) -> &[TimelineLane<Label>] {
        &self.lanes
    }

    /// Returns all lanes with their typed identifiers.
    pub fn lanes_with_ids(&self) -> impl Iterator<Item = (LaneId, &TimelineLane<Label>)> + '_ {
        self.lanes
            .iter()
            .enumerate()
            .map(|(index, lane)| (LaneId::new(index), lane))
    }

    /// Returns a lane by identifier.
    #[must_use]
    pub fn lane(&self, lane: LaneId) -> Option<&TimelineLane<Label>> {
        self.lanes.get(lane.index())
    }

    /// Returns all spans.
    #[must_use]
    pub fn spans(&self) -> &[TimelineSpan<Label>] {
        &self.spans
    }

    /// Returns all spans with their typed identifiers.
    pub fn spans_with_ids(&self) -> impl Iterator<Item = (SpanId, &TimelineSpan<Label>)> + '_ {
        self.spans
            .iter()
            .enumerate()
            .map(|(index, span)| (SpanId::new(index), span))
    }

    /// Returns a span by identifier.
    #[must_use]
    pub fn span(&self, span: SpanId) -> Option<&TimelineSpan<Label>> {
        self.spans.get(span.index())
    }

    /// Returns all flows.
    #[must_use]
    pub fn flows(&self) -> &[TimelineFlow] {
        &self.flows
    }

    /// Returns all flows with their typed identifiers.
    pub fn flows_with_ids(&self) -> impl Iterator<Item = (FlowId, &TimelineFlow)> + '_ {
        self.flows
            .iter()
            .enumerate()
            .map(|(index, flow)| (FlowId::new(index), flow))
    }

    /// Returns a flow by identifier.
    #[must_use]
    pub fn flow(&self, flow: FlowId) -> Option<&TimelineFlow> {
        self.flows.get(flow.index())
    }

    /// Returns all markers.
    #[must_use]
    pub fn markers(&self) -> &[TimelineMarker<Label>] {
        &self.markers
    }

    /// Returns all markers with their typed identifiers.
    pub fn markers_with_ids(
        &self,
    ) -> impl Iterator<Item = (MarkerId, &TimelineMarker<Label>)> + '_ {
        self.markers
            .iter()
            .enumerate()
            .map(|(index, marker)| (MarkerId::new(index), marker))
    }

    /// Returns a marker by identifier.
    #[must_use]
    pub fn marker(&self, marker: MarkerId) -> Option<&TimelineMarker<Label>> {
        self.markers.get(marker.index())
    }

    /// Returns the current playhead time.
    #[must_use]
    pub fn playhead(&self) -> f64 {
        self.playhead
    }

    /// Sets the playhead time.
    pub fn set_playhead(&mut self, playhead: f64) -> TimelineResult<()> {
        if !playhead.is_finite() {
            return Err(TimelineError::NonFinitePlayhead { playhead });
        }
        self.playhead = playhead;
        Ok(())
    }

    /// Returns the current selection, if any.
    #[must_use]
    pub fn selection(&self) -> Option<TimelineSelection> {
        self.selection
    }

    /// Sets a normalized time-range selection.
    pub fn set_selection(&mut self, start: f64, end: f64) -> TimelineResult<()> {
        self.set_time_selection(start, end)
    }

    /// Sets a normalized time-range selection.
    pub fn set_time_selection(&mut self, start: f64, end: f64) -> TimelineResult<()> {
        self.selection = Some(TimelineSelection::time_range(start, end)?);
        Ok(())
    }

    /// Selects a span by stable item key.
    ///
    /// Anonymous keys cannot be selected because they are not durable identity.
    pub fn select_span(&mut self, key: TimelineItemKey) -> TimelineResult<()> {
        if self
            .spans
            .iter()
            .any(|span| span.key == key && !key.is_anonymous())
        {
            self.selection = Some(TimelineSelection::Span(key));
            Ok(())
        } else {
            Err(TimelineError::UnknownSpanKey { key })
        }
    }

    /// Selects a marker by stable item key.
    ///
    /// Anonymous keys cannot be selected because they are not durable identity.
    pub fn select_marker(&mut self, key: TimelineItemKey) -> TimelineResult<()> {
        if self
            .markers
            .iter()
            .any(|marker| marker.key == key && !key.is_anonymous())
        {
            self.selection = Some(TimelineSelection::Marker(key));
            Ok(())
        } else {
            Err(TimelineError::UnknownMarkerKey { key })
        }
    }

    /// Clears the current selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Moves a span by a delta in timeline units.
    pub fn move_span_by(&mut self, span: SpanId, delta: f64) -> TimelineResult<()> {
        validate_delta(delta)?;
        let span_ref = self
            .spans
            .get_mut(span.index())
            .ok_or(TimelineError::UnknownSpan { span })?;
        let start = span_ref.start + delta;
        let end = span_ref.end + delta;
        if !start.is_finite() || !end.is_finite() {
            return Err(TimelineError::NonFiniteEditedSpan { span, start, end });
        }
        span_ref.start = start;
        span_ref.end = end;
        Ok(())
    }

    /// Resizes a span's start edge, clamping to `end - min_duration`.
    pub fn resize_span_start_by(
        &mut self,
        span: SpanId,
        delta: f64,
        min_duration: f64,
    ) -> TimelineResult<()> {
        validate_delta(delta)?;
        validate_min_duration(min_duration)?;
        let span_ref = self
            .spans
            .get_mut(span.index())
            .ok_or(TimelineError::UnknownSpan { span })?;
        let start = (span_ref.start + delta).min(span_ref.end - min_duration);
        if !start.is_finite() {
            return Err(TimelineError::NonFiniteEditedSpan {
                span,
                start,
                end: span_ref.end,
            });
        }
        span_ref.start = start;
        Ok(())
    }

    /// Resizes a span's end edge, clamping to `start + min_duration`.
    pub fn resize_span_end_by(
        &mut self,
        span: SpanId,
        delta: f64,
        min_duration: f64,
    ) -> TimelineResult<()> {
        validate_delta(delta)?;
        validate_min_duration(min_duration)?;
        let span_ref = self
            .spans
            .get_mut(span.index())
            .ok_or(TimelineError::UnknownSpan { span })?;
        let end = (span_ref.end + delta).max(span_ref.start + min_duration);
        if !end.is_finite() {
            return Err(TimelineError::NonFiniteEditedSpan {
                span,
                start: span_ref.start,
                end,
            });
        }
        span_ref.end = end;
        Ok(())
    }

    /// Moves the current selection by a delta in timeline units.
    ///
    /// Returns `Ok(false)` when no time-range selection is active.
    pub fn move_selection_by(&mut self, delta: f64) -> TimelineResult<bool> {
        let Some(TimelineSelection::TimeRange(selection)) = self.selection else {
            validate_delta(delta)?;
            return Ok(false);
        };
        self.selection = Some(TimelineSelection::TimeRange(selection.translate_by(delta)?));
        Ok(true)
    }

    /// Resizes the selection's start edge, clamping to `end - min_duration`.
    ///
    /// Returns `Ok(false)` when no time-range selection is active.
    pub fn resize_selection_start_by(
        &mut self,
        delta: f64,
        min_duration: f64,
    ) -> TimelineResult<bool> {
        validate_delta(delta)?;
        validate_min_duration(min_duration)?;
        let Some(TimelineSelection::TimeRange(selection)) = self.selection else {
            return Ok(false);
        };
        let start = (selection.start() + delta).min(selection.end() - min_duration);
        self.selection = Some(TimelineSelection::TimeRange(selection.with_start(start)?));
        Ok(true)
    }

    /// Resizes the selection's end edge, clamping to `start + min_duration`.
    ///
    /// Returns `Ok(false)` when no time-range selection is active.
    pub fn resize_selection_end_by(
        &mut self,
        delta: f64,
        min_duration: f64,
    ) -> TimelineResult<bool> {
        validate_delta(delta)?;
        validate_min_duration(min_duration)?;
        let Some(TimelineSelection::TimeRange(selection)) = self.selection else {
            return Ok(false);
        };
        let end = (selection.end() + delta).max(selection.start() + min_duration);
        self.selection = Some(TimelineSelection::TimeRange(selection.with_end(end)?));
        Ok(true)
    }

    /// Returns the exact content bounds across spans and markers.
    ///
    /// The playhead and selection are intentionally excluded.
    #[must_use]
    pub fn content_bounds(&self) -> Option<Range<f64>> {
        let mut bounds = None;

        for span in &self.spans {
            bounds = Some(include_bounds(bounds, span.start));
            bounds = Some(include_bounds(bounds, span.end));
        }
        for marker in &self.markers {
            bounds = Some(include_bounds(bounds, marker.time));
        }

        bounds
    }

    /// Returns spans whose time intervals intersect a non-empty query range.
    ///
    /// Spans use inclusive start and exclusive end semantics. Zero-duration
    /// spans are treated as point spans and are returned when their time falls
    /// inside `range`. The query is a linear scan and does not allocate.
    pub fn spans_intersecting(
        &self,
        range: Range<f64>,
    ) -> impl Iterator<Item = (SpanId, &TimelineSpan<Label>)> + '_ {
        let query_start = range.start;
        let query_end = range.end;
        self.spans
            .iter()
            .enumerate()
            .filter_map(move |(index, span)| {
                span_intersects_range(span.start, span.end, query_start, query_end)
                    .then_some((SpanId::new(index), span))
            })
    }

    /// Returns markers whose times fall inside a non-empty query range.
    ///
    /// The range start is inclusive and the range end is exclusive. The query is
    /// a linear scan and does not allocate.
    pub fn markers_in(
        &self,
        range: Range<f64>,
    ) -> impl Iterator<Item = (MarkerId, &TimelineMarker<Label>)> + '_ {
        let query_start = range.start;
        let query_end = range.end;
        self.markers
            .iter()
            .enumerate()
            .filter_map(move |(index, marker)| {
                (query_start < query_end && marker.time >= query_start && marker.time < query_end)
                    .then_some((MarkerId::new(index), marker))
            })
    }

    /// Returns borrowed timeline content filtered to a non-empty query range.
    #[must_use]
    pub fn content_in(&self, range: Range<f64>) -> TimelineRangeContent<'_, Label> {
        TimelineRangeContent { doc: self, range }
    }
}

/// Borrowed timeline content filtered to a time range.
///
/// This is a lightweight view returned by [`TimelineDoc::content_in`]. It does
/// not allocate; each accessor performs the same linear scan as the direct
/// range-query methods.
#[derive(Clone, Debug)]
pub struct TimelineRangeContent<'a, Label = String> {
    doc: &'a TimelineDoc<Label>,
    range: Range<f64>,
}

impl<'a, Label> TimelineRangeContent<'a, Label> {
    /// Returns spans whose time intervals intersect this content range.
    pub fn spans(&self) -> impl Iterator<Item = (SpanId, &'a TimelineSpan<Label>)> + '_ {
        self.doc
            .spans_intersecting(self.range.start..self.range.end)
    }

    /// Returns markers whose times fall inside this content range.
    pub fn markers(&self) -> impl Iterator<Item = (MarkerId, &'a TimelineMarker<Label>)> + '_ {
        self.doc.markers_in(self.range.start..self.range.end)
    }
}

fn include_bounds(bounds: Option<Range<f64>>, value: f64) -> Range<f64> {
    match bounds {
        Some(bounds) => bounds.start.min(value)..bounds.end.max(value),
        None => value..value,
    }
}

fn reconcile_selection<Label>(
    selection: Option<TimelineSelection>,
    spans: &[TimelineSpan<Label>],
    markers: &[TimelineMarker<Label>],
) -> Option<TimelineSelection> {
    match selection {
        Some(TimelineSelection::TimeRange(_)) | None => selection,
        Some(TimelineSelection::Span(key)) => spans
            .iter()
            .any(|span| span.key == key)
            .then_some(TimelineSelection::Span(key)),
        Some(TimelineSelection::Marker(key)) => markers
            .iter()
            .any(|marker| marker.key == key)
            .then_some(TimelineSelection::Marker(key)),
    }
}

fn span_intersects_range(span_start: f64, span_end: f64, query_start: f64, query_end: f64) -> bool {
    if query_start >= query_end {
        return false;
    }
    if span_start == span_end {
        span_start >= query_start && span_start < query_end
    } else {
        span_start < query_end && span_end > query_start
    }
}

fn validate_content<Label>(
    lanes: &[TimelineLane<Label>],
    spans: &[TimelineSpan<Label>],
    markers: &[TimelineMarker<Label>],
    flows: &[TimelineFlow],
) -> TimelineResult<()> {
    validate_unique_item_keys(lanes, spans, markers, flows)?;
    validate_spans(spans, lanes.len())?;
    validate_markers(markers, lanes.len())?;
    validate_flows(flows, spans.len())?;
    Ok(())
}

fn validate_unique_item_keys<Label>(
    lanes: &[TimelineLane<Label>],
    spans: &[TimelineSpan<Label>],
    markers: &[TimelineMarker<Label>],
    flows: &[TimelineFlow],
) -> TimelineResult<()> {
    let mut keys = Vec::with_capacity(lanes.len() + spans.len() + markers.len() + flows.len());
    keys.extend(lanes.iter().map(|lane| lane.key));
    keys.extend(spans.iter().map(|span| span.key));
    keys.extend(markers.iter().map(|marker| marker.key));
    keys.extend(flows.iter().map(|flow| flow.key));
    keys.retain(|key| !key.is_anonymous());
    keys.sort_unstable();

    for window in keys.windows(2) {
        if window[0] == window[1] {
            return Err(TimelineError::DuplicateItemKey { key: window[0] });
        }
    }

    Ok(())
}

fn validate_spans<Label>(spans: &[TimelineSpan<Label>], lane_count: usize) -> TimelineResult<()> {
    for (index, span) in spans.iter().enumerate() {
        validate_span(SpanId::new(index), span, lane_count)?;
    }
    Ok(())
}

fn validate_span<Label>(
    span_id: SpanId,
    span: &TimelineSpan<Label>,
    lane_count: usize,
) -> TimelineResult<()> {
    if !span.start.is_finite() || !span.end.is_finite() {
        return Err(TimelineError::NonFiniteSpanTime {
            span: span_id,
            start: span.start,
            end: span.end,
        });
    }
    if span.start > span.end {
        return Err(TimelineError::ReversedSpan {
            span: span_id,
            start: span.start,
            end: span.end,
        });
    }
    if span.lane.index() >= lane_count {
        return Err(TimelineError::UnknownSpanLane {
            span: span_id,
            lane: span.lane,
        });
    }
    Ok(())
}

fn validate_markers<Label>(
    markers: &[TimelineMarker<Label>],
    lane_count: usize,
) -> TimelineResult<()> {
    for (index, marker) in markers.iter().enumerate() {
        validate_marker(MarkerId::new(index), marker, lane_count)?;
    }
    Ok(())
}

fn validate_marker<Label>(
    marker_id: MarkerId,
    marker: &TimelineMarker<Label>,
    lane_count: usize,
) -> TimelineResult<()> {
    if !marker.time.is_finite() {
        return Err(TimelineError::NonFiniteMarkerTime {
            marker: marker_id,
            time: marker.time,
        });
    }
    if let Some(lane) = marker.lane
        && lane.index() >= lane_count
    {
        return Err(TimelineError::UnknownMarkerLane {
            marker: marker_id,
            lane,
        });
    }
    Ok(())
}

fn validate_flows(flows: &[TimelineFlow], span_count: usize) -> TimelineResult<()> {
    for (index, flow) in flows.iter().copied().enumerate() {
        validate_flow(FlowId::new(index), flow, span_count)?;
    }
    Ok(())
}

fn validate_flow(flow_id: FlowId, flow: TimelineFlow, span_count: usize) -> TimelineResult<()> {
    if flow.from_span.index() >= span_count {
        return Err(TimelineError::UnknownFlowEndpoint {
            flow: flow_id,
            span: flow.from_span,
        });
    }
    if flow.to_span.index() >= span_count {
        return Err(TimelineError::UnknownFlowEndpoint {
            flow: flow_id,
            span: flow.to_span,
        });
    }
    Ok(())
}

fn validate_delta(delta: f64) -> TimelineResult<()> {
    if delta.is_finite() {
        Ok(())
    } else {
        Err(TimelineError::NonFiniteDelta { delta })
    }
}

fn validate_min_duration(min_duration: f64) -> TimelineResult<()> {
    if min_duration.is_finite() && min_duration >= 0.0 {
        Ok(())
    } else {
        Err(TimelineError::InvalidMinimumDuration { min_duration })
    }
}
