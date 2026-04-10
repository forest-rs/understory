// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::vec::Vec;

use crate::{
    FlowId, LaneId, MarkerId, SpanId, TimelineDoc, TimelineError, TimelineFlow, TimelineItemKey,
    TimelineLane, TimelineMarker, TimelineSelection, TimelineSpan, TimelineTimeRange,
};

fn one_span_doc() -> TimelineDoc {
    TimelineDoc::try_new(
        [TimelineLane::new("Main")],
        [TimelineSpan::new("Layout", 10.0, 20.0, LaneId::new(0))],
    )
    .unwrap()
}

#[test]
fn selection_is_normalized() {
    let selection = TimelineTimeRange::try_new(8.0, 2.0).unwrap();
    assert_eq!(selection.start(), 2.0);
    assert_eq!(selection.end(), 8.0);
    assert_eq!(selection.range(), 2.0..8.0);
}

#[test]
fn document_rejects_invalid_spans() {
    let lane = TimelineLane::new("Main");
    assert_eq!(
        TimelineDoc::try_new(
            [lane.clone()],
            [TimelineSpan::new("Bad", 20.0, 10.0, LaneId::new(0))]
        ),
        Err(TimelineError::ReversedSpan {
            span: SpanId::new(0),
            start: 20.0,
            end: 10.0,
        })
    );
    let error = TimelineDoc::try_new(
        [lane.clone()],
        [TimelineSpan::new("Bad", f64::NAN, 10.0, LaneId::new(0))],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        TimelineError::NonFiniteSpanTime {
            span,
            start,
            end: 10.0,
        } if span == SpanId::new(0) && start.is_nan()
    ));
    assert_eq!(
        TimelineDoc::try_new(
            [lane],
            [TimelineSpan::new("Bad", 0.0, 10.0, LaneId::new(1))]
        ),
        Err(TimelineError::UnknownSpanLane {
            span: SpanId::new(0),
            lane: LaneId::new(1),
        })
    );
}

#[test]
fn marker_and_flow_replacement_is_transactional() {
    let mut doc = one_span_doc();
    doc.set_markers([TimelineMarker::new_global("Start", 10.0)])
        .unwrap();
    assert_eq!(doc.markers().len(), 1);

    assert_eq!(
        doc.set_markers([TimelineMarker::new("Bad", 12.0, LaneId::new(9))]),
        Err(TimelineError::UnknownMarkerLane {
            marker: MarkerId::new(0),
            lane: LaneId::new(9),
        })
    );
    assert_eq!(doc.markers()[0].label, "Start");

    doc.set_flows([TimelineFlow::new(SpanId::new(0), SpanId::new(0))])
        .unwrap();
    assert_eq!(
        doc.set_flows([TimelineFlow::new(SpanId::new(0), SpanId::new(99))]),
        Err(TimelineError::UnknownFlowEndpoint {
            flow: FlowId::new(0),
            span: SpanId::new(99),
        })
    );
    assert_eq!(doc.flows()[0].to_span, SpanId::new(0));
}

#[test]
fn push_methods_validate_and_return_typed_ids() {
    let mut doc = TimelineDoc::try_new([], []).unwrap();
    let main = doc.push_lane(TimelineLane::new("Main")).unwrap();
    let span = doc
        .push_span(TimelineSpan::new("Layout", 10.0, 20.0, main))
        .unwrap();
    let marker = doc
        .push_marker(TimelineMarker::new("Commit", 15.0, main))
        .unwrap();
    let flow = doc.push_flow(TimelineFlow::new(span, span)).unwrap();

    assert_eq!(main.index(), 0);
    assert_eq!(span.index(), 0);
    assert_eq!(marker.index(), 0);
    assert_eq!(flow.index(), 0);
}

#[test]
fn compact_labels_work_for_large_traces() {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    struct LabelId(u32);

    let mut doc = TimelineDoc::with_capacity(1, 2, 1, 0);
    let main = doc
        .push_lane(TimelineLane::from_label(LabelId(10)).with_key(TimelineItemKey::new(10)))
        .unwrap();
    let span = doc
        .push_span(
            TimelineSpan::from_label(LabelId(20), 0.0, 8.0, main)
                .with_key(TimelineItemKey::new(20)),
        )
        .unwrap();
    let marker = doc
        .push_marker(
            TimelineMarker::from_label(LabelId(30), 4.0, main).with_key(TimelineItemKey::new(30)),
        )
        .unwrap();

    assert_eq!(doc.lane(main).unwrap().label, LabelId(10));
    assert_eq!(doc.span(span).unwrap().label, LabelId(20));
    assert_eq!(doc.marker(marker).unwrap().label, LabelId(30));
}

#[test]
fn duplicate_stable_item_keys_are_rejected() {
    assert_eq!(
        TimelineDoc::try_new(
            [TimelineLane::new("Main").with_key(TimelineItemKey::new(7))],
            [
                TimelineSpan::new("Span", 0.0, 1.0, LaneId::new(0))
                    .with_key(TimelineItemKey::new(7))
            ]
        ),
        Err(TimelineError::DuplicateItemKey {
            key: TimelineItemKey::new(7),
        })
    );
}

#[test]
fn replace_content_is_transactional() {
    let mut doc = one_span_doc();
    doc.set_flows([TimelineFlow::new(SpanId::new(0), SpanId::new(0))])
        .unwrap();

    assert_eq!(
        doc.replace_content(
            [TimelineLane::new("Replacement")],
            [TimelineSpan::new("Only", 0.0, 1.0, LaneId::new(0))],
            [],
            [TimelineFlow::new(SpanId::new(0), SpanId::new(99))]
        ),
        Err(TimelineError::UnknownFlowEndpoint {
            flow: FlowId::new(0),
            span: SpanId::new(99),
        })
    );

    assert_eq!(doc.lanes()[0].label, "Main");
    assert_eq!(doc.spans()[0].label, "Layout");
    assert_eq!(doc.flows()[0].to_span, SpanId::new(0));
}

#[test]
fn selected_items_survive_or_clear_across_content_replacement() {
    let selected_key = TimelineItemKey::new(100);
    let missing_key = TimelineItemKey::new(200);
    let mut doc = TimelineDoc::try_from_parts(
        [TimelineLane::new("Main")],
        [TimelineSpan::new("Selected", 0.0, 1.0, LaneId::new(0)).with_key(selected_key)],
        [TimelineMarker::new_global("Marker", 0.5).with_key(missing_key)],
        [],
    )
    .unwrap();

    doc.select_span(selected_key).unwrap();
    doc.replace_content(
        [TimelineLane::new("Main")],
        [TimelineSpan::new("Selected again", 1.0, 2.0, LaneId::new(0)).with_key(selected_key)],
        [],
        [],
    )
    .unwrap();
    assert_eq!(doc.selection(), Some(TimelineSelection::Span(selected_key)));

    doc.select_marker(missing_key).unwrap_err();
    doc.select_span(selected_key).unwrap();
    doc.replace_content(
        [TimelineLane::new("Main")],
        [TimelineSpan::new("Other", 2.0, 3.0, LaneId::new(0)).with_key(missing_key)],
        [],
        [],
    )
    .unwrap();
    assert_eq!(doc.selection(), None);
}

#[test]
fn id_iterators_return_typed_ids() {
    let mut doc = one_span_doc();
    doc.set_markers([TimelineMarker::new_global("Marker", 12.0)])
        .unwrap();
    doc.set_flows([TimelineFlow::new(SpanId::new(0), SpanId::new(0))])
        .unwrap();

    assert_eq!(
        doc.lanes_with_ids()
            .map(|(id, lane)| (id, lane.label.as_str()))
            .collect::<Vec<_>>(),
        [(LaneId::new(0), "Main")]
    );
    assert_eq!(
        doc.spans_with_ids()
            .map(|(id, span)| (id, span.label.as_str()))
            .collect::<Vec<_>>(),
        [(SpanId::new(0), "Layout")]
    );
    assert_eq!(
        doc.markers_with_ids()
            .map(|(id, marker)| (id, marker.label.as_str()))
            .collect::<Vec<_>>(),
        [(MarkerId::new(0), "Marker")]
    );
    assert_eq!(
        doc.flows_with_ids()
            .map(|(id, flow)| (id, flow.to_span))
            .collect::<Vec<_>>(),
        [(FlowId::new(0), SpanId::new(0))]
    );
}

#[test]
fn span_edits_apply_min_duration() {
    let mut doc = one_span_doc();

    doc.move_span_by(SpanId::new(0), 5.0).unwrap();
    assert_eq!(doc.spans()[0].start, 15.0);
    assert_eq!(doc.spans()[0].end, 25.0);

    doc.resize_span_start_by(SpanId::new(0), 20.0, 4.0).unwrap();
    assert_eq!(doc.spans()[0].start, 21.0);

    doc.resize_span_end_by(SpanId::new(0), -50.0, 4.0).unwrap();
    assert_eq!(doc.spans()[0].end, 25.0);
}

#[test]
fn edits_reject_invalid_ids_and_inputs() {
    let mut doc = one_span_doc();

    assert_eq!(
        doc.move_span_by(SpanId::new(9), 1.0),
        Err(TimelineError::UnknownSpan {
            span: SpanId::new(9),
        })
    );
    assert_eq!(
        doc.move_span_by(SpanId::new(0), f64::INFINITY),
        Err(TimelineError::NonFiniteDelta {
            delta: f64::INFINITY,
        })
    );
    assert_eq!(
        doc.resize_span_end_by(SpanId::new(0), 1.0, -1.0),
        Err(TimelineError::InvalidMinimumDuration { min_duration: -1.0 })
    );
}

#[test]
fn content_bounds_include_spans_and_markers() {
    let mut doc = TimelineDoc::try_new(
        [TimelineLane::new("Main")],
        [TimelineSpan::new("Span", 15.0, 30.0, LaneId::new(0))],
    )
    .unwrap();
    doc.set_markers([TimelineMarker::new_global("Marker", 12.0)])
        .unwrap();

    assert_eq!(doc.content_bounds(), Some(12.0..30.0));
}

#[test]
fn time_range_queries_filter_spans_and_markers_without_allocating() {
    let mut doc = TimelineDoc::try_new(
        [TimelineLane::new("Main")],
        [
            TimelineSpan::new("Before", 0.0, 5.0, LaneId::new(0)),
            TimelineSpan::new("OverlapStart", 8.0, 12.0, LaneId::new(0)),
            TimelineSpan::new("Inside", 12.0, 14.0, LaneId::new(0)),
            TimelineSpan::new("PointAtStart", 10.0, 10.0, LaneId::new(0)),
            TimelineSpan::new("PointInside", 15.0, 15.0, LaneId::new(0)),
            TimelineSpan::new("PointAtEnd", 16.0, 16.0, LaneId::new(0)),
            TimelineSpan::new("After", 20.0, 25.0, LaneId::new(0)),
        ],
    )
    .unwrap();
    doc.set_markers([
        TimelineMarker::new_global("AtStart", 10.0),
        TimelineMarker::new_global("Inside", 12.5),
        TimelineMarker::new_global("AtEnd", 16.0),
    ])
    .unwrap();

    let span_ids = doc
        .spans_intersecting(10.0..16.0)
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    let marker_ids = doc
        .markers_in(10.0..16.0)
        .map(|(id, _)| id)
        .collect::<Vec<_>>();

    assert_eq!(
        span_ids,
        [
            SpanId::new(1),
            SpanId::new(2),
            SpanId::new(3),
            SpanId::new(4)
        ],
        "range queries include partial overlaps, interior spans, and start/inside point spans"
    );
    assert_eq!(
        marker_ids,
        [MarkerId::new(0), MarkerId::new(1)],
        "marker queries are start-inclusive and end-exclusive"
    );
    assert_eq!(doc.spans_intersecting(16.0..16.0).count(), 0);
    assert_eq!(doc.markers_in(16.0..16.0).count(), 0);

    let content = doc.content_in(10.0..16.0);
    assert_eq!(content.spans().count(), 4);
    assert_eq!(content.markers().count(), 2);
}

#[test]
fn empty_content_has_no_bounds() {
    let doc = TimelineDoc::<String>::try_new([], []).unwrap();
    assert_eq!(doc.content_bounds(), None);
}

#[test]
fn selection_edits_work() {
    let mut doc = TimelineDoc::<String>::try_new([], []).unwrap();
    doc.set_selection(40.0, 10.0).unwrap();
    assert_eq!(
        doc.selection(),
        Some(TimelineSelection::TimeRange(
            TimelineTimeRange::try_new(10.0, 40.0).unwrap()
        ))
    );

    assert!(doc.move_selection_by(5.0).unwrap());
    assert_eq!(
        doc.selection(),
        Some(TimelineSelection::TimeRange(
            TimelineTimeRange::try_new(15.0, 45.0).unwrap()
        ))
    );

    assert!(doc.resize_selection_start_by(50.0, 4.0).unwrap());
    assert_eq!(
        doc.selection(),
        Some(TimelineSelection::TimeRange(
            TimelineTimeRange::try_new(41.0, 45.0).unwrap()
        ))
    );

    assert!(doc.resize_selection_end_by(-100.0, 4.0).unwrap());
    assert_eq!(
        doc.selection(),
        Some(TimelineSelection::TimeRange(
            TimelineTimeRange::try_new(41.0, 45.0).unwrap()
        ))
    );
}

#[test]
fn selection_edits_report_absent_selection() {
    let mut doc = TimelineDoc::<String>::try_new([], []).unwrap();
    assert!(!doc.move_selection_by(5.0).unwrap());
    assert!(!doc.resize_selection_start_by(5.0, 1.0).unwrap());
    assert!(!doc.resize_selection_end_by(5.0, 1.0).unwrap());
}
