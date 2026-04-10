// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use understory_timeline_model::{
    LaneId, SpanId, TimelineDoc, TimelineItemKey, TimelineLane, TimelineMarker, TimelineSpan,
};

const LANE_COUNT: usize = 64;
const SIZES: [usize; 2] = [10_000, 100_000];

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct LabelId(u32);

type TimelineParts = (
    Vec<TimelineLane<LabelId>>,
    Vec<TimelineSpan<LabelId>>,
    Vec<TimelineMarker<LabelId>>,
);

fn key(namespace: u64, index: usize) -> TimelineItemKey {
    TimelineItemKey::new(namespace + index as u64)
}

fn lanes() -> Vec<TimelineLane<LabelId>> {
    (0..LANE_COUNT)
        .map(|index| {
            TimelineLane::from_label(LabelId(index as u32)).with_key(key(1_000_000, index))
        })
        .collect()
}

fn spans(n: usize) -> Vec<TimelineSpan<LabelId>> {
    let mut spans = Vec::with_capacity(n);
    for index in 0..n {
        let lane = LaneId::new(index % LANE_COUNT);
        let start = index as f64 * 0.125;
        let duration = 0.05 + (index % 13) as f64 * 0.025;
        spans.push(
            TimelineSpan::from_label(
                LabelId((index % 4_096) as u32),
                start,
                start + duration,
                lane,
            )
            .with_key(key(2_000_000, index))
            .with_depth(index % 6),
        );
    }
    spans
}

fn markers(n: usize) -> Vec<TimelineMarker<LabelId>> {
    let marker_count = n / 16;
    let mut markers = Vec::with_capacity(marker_count);
    for index in 0..marker_count {
        let time = index as f64 * 2.0;
        markers.push(
            TimelineMarker::global_from_label(LabelId((index % 512) as u32), time)
                .with_key(key(3_000_000, index)),
        );
    }
    markers
}

fn make_parts(n: usize) -> TimelineParts {
    (lanes(), spans(n), markers(n))
}

fn make_doc(n: usize) -> TimelineDoc<LabelId> {
    let (lanes, spans, markers) = make_parts(n);
    TimelineDoc::try_from_parts(lanes, spans, markers, []).expect("synthetic timeline is valid")
}

fn bench_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("timeline_model/build");
    for n in SIZES {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(|| n, make_doc, BatchSize::SmallInput);
        });
    }
    group.finish();
}

fn bench_replace_content(c: &mut Criterion) {
    let mut group = c.benchmark_group("timeline_model/replace_content");
    for n in SIZES {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                || make_parts(n),
                |(lanes, spans, markers)| {
                    let mut doc: TimelineDoc<LabelId> = TimelineDoc::default();
                    black_box(doc.replace_content(lanes, spans, markers, []))
                },
                BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

fn bench_content_bounds(c: &mut Criterion) {
    let mut group = c.benchmark_group("timeline_model/content_bounds");
    for n in SIZES {
        let doc = make_doc(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(doc.content_bounds()));
        });
    }
    group.finish();
}

fn bench_visible_range(c: &mut Criterion) {
    let mut group = c.benchmark_group("timeline_model/visible_range");
    for n in SIZES {
        let doc = make_doc(n);
        let query_start = n as f64 * 0.04;
        let query = query_start..query_start + 250.0;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let content = doc.content_in(query.start..query.end);
                let spans = content.spans().count();
                let markers = content.markers().count();
                black_box((spans, markers))
            });
        });
    }
    group.finish();
}

fn bench_edit_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("timeline_model/edit_cycle");
    let mut doc = make_doc(100_000);
    doc.set_time_selection(1_000.0, 2_000.0)
        .expect("selection is valid");
    let span = SpanId::new(50_000);
    let mut direction = 1.0;

    group.bench_function("move_span_and_selection/100000", |b| {
        b.iter(|| {
            direction = -direction;
            doc.move_span_by(span, direction)
                .expect("span edit is valid");
            black_box(
                doc.move_selection_by(direction)
                    .expect("selection edit is valid"),
            );
        });
    });
    group.finish();
}

fn timeline_model(c: &mut Criterion) {
    bench_build(c);
    bench_replace_content(c);
    bench_content_bounds(c);
    bench_visible_range(c);
    bench_edit_cycle(c);
}

criterion_group!(benches, timeline_model);
criterion_main!(benches);
