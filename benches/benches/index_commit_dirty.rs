// Copyright 2025 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use understory_index::{Aabb2D, Backend, Index, IndexGeneric, Key};

const SPARSE_DIRTY_SIZES: [usize; 3] = [1_024, 16_384, 65_536];
const ALL_DIRTY_SIZES: [usize; 2] = [1_024, 16_384];

fn gen_grid_rects(n: usize) -> Vec<Aabb2D<f64>> {
    let side = n.isqrt();
    let mut out = Vec::with_capacity(n);
    for y in 0..side {
        for x in 0..side {
            if out.len() == n {
                return out;
            }
            let x0 = x as f64 * 10.0;
            let y0 = y as f64 * 10.0;
            out.push(Aabb2D::from_xywh(x0, y0, 8.0, 8.0));
        }
    }
    out
}

fn build_index<B, F>(rects: &[Aabb2D<f64>], make_index: F) -> IndexGeneric<f64, u32, B>
where
    B: Backend<f64>,
    F: Fn() -> IndexGeneric<f64, u32, B>,
{
    let (idx, _) = build_index_with_keys(rects, make_index);
    idx
}

fn build_index_with_keys<B, F>(
    rects: &[Aabb2D<f64>],
    make_index: F,
) -> (IndexGeneric<f64, u32, B>, Vec<Key>)
where
    B: Backend<f64>,
    F: Fn() -> IndexGeneric<f64, u32, B>,
{
    let mut idx = make_index();
    let mut keys = Vec::with_capacity(rects.len());
    idx.reserve(rects.len());
    for (i, rect) in rects.iter().copied().enumerate() {
        keys.push(idx.insert(rect, i as u32));
    }
    let _ = idx.commit();
    (idx, keys)
}

fn bench_commit_noop<B, F>(c: &mut Criterion, group_name: &str, backend_name: &str, make_index: F)
where
    B: Backend<f64> + 'static,
    F: Fn() -> IndexGeneric<f64, u32, B> + Copy + 'static,
{
    let mut group = c.benchmark_group(group_name);
    for n in SPARSE_DIRTY_SIZES {
        let rects = gen_grid_rects(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_function(BenchmarkId::new(backend_name, n), |b| {
            let mut idx = build_index(&rects, make_index);
            b.iter(|| black_box(idx.commit()));
        });
    }
    group.finish();
}

fn bench_commit_one_update<B, F>(
    c: &mut Criterion,
    group_name: &str,
    backend_name: &str,
    make_index: F,
) where
    B: Backend<f64> + 'static,
    F: Fn() -> IndexGeneric<f64, u32, B> + Copy + 'static,
{
    let mut group = c.benchmark_group(group_name);
    for n in SPARSE_DIRTY_SIZES {
        let rects = gen_grid_rects(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_function(BenchmarkId::new(backend_name, n), |b| {
            let (mut idx, keys) = build_index_with_keys(&rects, make_index);
            let key = keys[0];
            let mut shifted = false;
            b.iter(|| {
                shifted = !shifted;
                let x = if shifted { 1.0 } else { 0.0 };
                idx.update(key, Aabb2D::from_xywh(x, 0.0, 8.0, 8.0));
                black_box(idx.commit())
            });
        });
    }
    group.finish();
}

fn bench_commit_all_updates<B, F>(
    c: &mut Criterion,
    group_name: &str,
    backend_name: &str,
    make_index: F,
) where
    B: Backend<f64> + 'static,
    F: Fn() -> IndexGeneric<f64, u32, B> + Copy + 'static,
{
    let mut group = c.benchmark_group(group_name);
    for n in ALL_DIRTY_SIZES {
        let rects = gen_grid_rects(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_function(BenchmarkId::new(backend_name, n), |b| {
            let (mut idx, keys) = build_index_with_keys(&rects, make_index);
            let mut shifted = false;
            b.iter(|| {
                shifted = !shifted;
                let dx = if shifted { 1.0 } else { 0.0 };
                for (key, rect) in keys.iter().copied().zip(rects.iter().copied()) {
                    idx.update(
                        key,
                        Aabb2D::from_xywh(rect.min_x + dx, rect.min_y, 8.0, 8.0),
                    );
                }
                black_box(idx.commit())
            });
        });
    }
    group.finish();
}

fn index_commit_dirty(c: &mut Criterion) {
    bench_commit_noop::<understory_index::backends::FlatVec<f64>, _>(
        c,
        "index_commit_noop",
        "flatvec",
        Index::<f64, u32>::new,
    );
    bench_commit_noop::<understory_index::backends::RTreeF64, _>(
        c,
        "index_commit_noop",
        "rtree",
        Index::<f64, u32>::with_rtree,
    );

    bench_commit_one_update::<understory_index::backends::FlatVec<f64>, _>(
        c,
        "index_commit_one_update",
        "flatvec",
        Index::<f64, u32>::new,
    );
    bench_commit_one_update::<understory_index::backends::RTreeF64, _>(
        c,
        "index_commit_one_update",
        "rtree",
        Index::<f64, u32>::with_rtree,
    );

    bench_commit_all_updates::<understory_index::backends::FlatVec<f64>, _>(
        c,
        "index_commit_all_updates",
        "flatvec",
        Index::<f64, u32>::new,
    );
    bench_commit_all_updates::<understory_index::backends::RTreeF64, _>(
        c,
        "index_commit_all_updates",
        "rtree",
        Index::<f64, u32>::with_rtree,
    );
}

criterion_group!(benches, index_commit_dirty);
criterion_main!(benches);
