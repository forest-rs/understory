// Copyright 2025 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Binary bounding hierarchy backend generic over scalar `T: Scalar`.

use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Debug;

use crate::backend::Backend;
use crate::types::{Aabb2D, Scalar};

/// A simple BVH backend using SAH-like splits.
pub struct Bvh<T: Scalar> {
    max_leaf: usize,
    root: Option<NodeIdx>,
    arena: Vec<Node<T>>,
    slots: Vec<Option<Aabb2D<T>>>,
}

enum Kind<T: Scalar> {
    Leaf(Vec<(usize, Aabb2D<T>)>),
    Internal { left: NodeIdx, right: NodeIdx },
}

struct Node<T: Scalar> {
    bbox: Aabb2D<T>,
    kind: Kind<T>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct NodeIdx(usize);

impl NodeIdx {
    const fn new(i: usize) -> Self {
        Self(i)
    }

    const fn get(self) -> usize {
        self.0
    }
}

impl<T: Scalar> Default for Bvh<T> {
    fn default() -> Self {
        Self {
            max_leaf: 8,
            root: None,
            arena: Vec::new(),
            slots: Vec::new(),
        }
    }
}

// Reduce clippy::type_complexity noise for local helpers.
type BvhItem<TS> = (usize, Aabb2D<TS>);
type BvhItems<TS> = Vec<BvhItem<TS>>;
type BvhBestSplit<TS> = Option<(crate::types::ScalarAcc<TS>, BvhItems<TS>, BvhItems<TS>)>;

const INLINE_STACK_CAP: usize = 64;

impl<T: Scalar> Bvh<T> {
    fn ensure_slot(&mut self, slot: usize, bbox: Aabb2D<T>) {
        if self.slots.len() <= slot {
            self.slots.resize_with(slot + 1, || None);
        }
        self.slots[slot] = Some(bbox);
    }

    fn bbox_items(items: &[(usize, Aabb2D<T>)]) -> Aabb2D<T> {
        let mut it = items.iter();
        if let Some((_, b)) = it.next() {
            let mut acc = *b;
            for (_, bb) in it {
                acc = acc.union(*bb);
            }
            acc
        } else {
            Aabb2D::new(T::zero(), T::zero(), T::zero(), T::zero())
        }
    }

    /// SAH-like split: sort along an axis, precompute prefix/suffix AABBs, and
    /// choose `k` that minimizes `area(LB_k) * k + area(RB_k) * (n - k)`.
    fn split_sah(mut items: BvhItems<T>, max_leaf: usize) -> (BvhItems<T>, BvhItems<T>) {
        let n = items.len();
        let min_children = (max_leaf / 2).max(2).min(n.saturating_sub(2));
        let mut best: BvhBestSplit<T> = None;
        for axis in 0..2 {
            items.sort_by(|a, b| {
                let ca = if axis == 0 {
                    Scalar::mid(a.1.min_x, a.1.max_x)
                } else {
                    Scalar::mid(a.1.min_y, a.1.max_y)
                };
                let cb = if axis == 0 {
                    Scalar::mid(b.1.min_x, b.1.max_x)
                } else {
                    Scalar::mid(b.1.min_y, b.1.max_y)
                };
                match ca.partial_cmp(&cb) {
                    Some(ord) => ord,
                    None => core::cmp::Ordering::Equal,
                }
            });

            // Precompute prefix/suffix bboxes for O(1) split evaluation
            let mut prefix: Vec<Aabb2D<T>> = Vec::with_capacity(n);
            for (i, (_, bb)) in items.iter().enumerate() {
                if i == 0 {
                    prefix.push(*bb);
                } else {
                    let prev = *prefix.last().unwrap();
                    prefix.push(prev.union(*bb));
                }
            }
            let mut suffix: Vec<Aabb2D<T>> = Vec::with_capacity(n);
            for (i, (_, bb)) in items.iter().enumerate().rev() {
                if i == n - 1 {
                    suffix.push(*bb);
                } else {
                    let prev = *suffix.last().unwrap();
                    suffix.push(prev.union(*bb));
                }
            }
            suffix.reverse();

            for k in min_children..=(n - min_children) {
                let lb = prefix[k - 1];
                let rb = suffix[k];
                let cost = lb.area() * T::acc_from_usize(k) + rb.area() * T::acc_from_usize(n - k);
                if best.as_ref().map(|(bc, _, _)| cost < *bc).unwrap_or(true) {
                    let left = items[..k].to_vec();
                    let right = items[k..].to_vec();
                    best = Some((cost, left, right));
                }
            }
        }
        let (_, l, r) = best.expect("BVH split requires at least 4 items");
        (l, r)
    }

    fn insert_node(
        arena: &mut Vec<Node<T>>,
        node_idx: usize,
        slot: usize,
        bbox: Aabb2D<T>,
        max_leaf: usize,
    ) {
        let kind = core::mem::replace(&mut arena[node_idx].kind, Kind::Leaf(Vec::new()));
        match kind {
            Kind::Leaf(mut items) => {
                items.push((slot, bbox));
                let mut node_bbox = arena[node_idx].bbox.union(bbox);
                let new_kind = if items.len() > max_leaf {
                    let (l, r) = Self::split_sah(items, max_leaf);
                    let l_idx = arena.len();
                    arena.push(Node {
                        bbox: Self::bbox_items(&l),
                        kind: Kind::Leaf(l),
                    });
                    let r_idx = arena.len();
                    arena.push(Node {
                        bbox: Self::bbox_items(&r),
                        kind: Kind::Leaf(r),
                    });
                    node_bbox = arena[l_idx].bbox.union(arena[r_idx].bbox);
                    Kind::Internal {
                        left: NodeIdx::new(l_idx),
                        right: NodeIdx::new(r_idx),
                    }
                } else {
                    Kind::Leaf(items)
                };
                arena[node_idx].kind = new_kind;
                arena[node_idx].bbox = node_bbox;
            }
            Kind::Internal { left, right } => {
                let lb = arena[left.get()].bbox;
                let rb = arena[right.get()].bbox;
                let cost_l = lb.union(bbox).area() - lb.area();
                let cost_r = rb.union(bbox).area() - rb.area();
                if cost_l <= cost_r {
                    Self::insert_node(arena, left.get(), slot, bbox, max_leaf);
                } else {
                    Self::insert_node(arena, right.get(), slot, bbox, max_leaf);
                }
                let node_bbox = arena[node_idx].bbox.union(bbox);
                arena[node_idx].kind = Kind::Internal { left, right };
                arena[node_idx].bbox = node_bbox;
            }
        }
    }

    fn remove_node(
        arena: &mut Vec<Node<T>>,
        node_idx: usize,
        slot: usize,
        old: &Aabb2D<T>,
    ) -> bool {
        if !arena[node_idx].bbox.overlaps(old) {
            return false;
        }
        let kind = core::mem::replace(&mut arena[node_idx].kind, Kind::Leaf(Vec::new()));
        let (new_kind, new_bbox, removed) = match kind {
            Kind::Leaf(mut items) => {
                let before = items.len();
                items.retain(|(s, _)| *s != slot);
                let removed = items.len() != before;
                let bbox = Self::bbox_items(&items);
                (Kind::Leaf(items), bbox, removed)
            }
            Kind::Internal { left, right } => {
                let removed = Self::remove_node(arena, left.get(), slot, old)
                    | Self::remove_node(arena, right.get(), slot, old);
                let is_left_empty =
                    matches!(arena[left.get()].kind, Kind::Leaf(ref v) if v.is_empty());
                let is_right_empty =
                    matches!(arena[right.get()].kind, Kind::Leaf(ref v) if v.is_empty());
                if removed {
                    if is_left_empty && !is_right_empty {
                        let kind = core::mem::replace(
                            &mut arena[right.get()].kind,
                            Kind::Leaf(Vec::new()),
                        );
                        let bbox = arena[right.get()].bbox;
                        (kind, bbox, true)
                    } else if is_right_empty && !is_left_empty {
                        let kind =
                            core::mem::replace(&mut arena[left.get()].kind, Kind::Leaf(Vec::new()));
                        let bbox = arena[left.get()].bbox;
                        (kind, bbox, true)
                    } else {
                        let bbox = arena[left.get()].bbox.union(arena[right.get()].bbox);
                        (Kind::Internal { left, right }, bbox, true)
                    }
                } else {
                    let bbox = arena[left.get()].bbox.union(arena[right.get()].bbox);
                    (Kind::Internal { left, right }, bbox, false)
                }
            }
        };
        arena[node_idx].kind = new_kind;
        arena[node_idx].bbox = new_bbox;
        removed
    }

    #[inline]
    fn pop_stack(
        inline: &mut [NodeIdx; INLINE_STACK_CAP],
        inline_len: &mut usize,
        heap: &mut Vec<NodeIdx>,
    ) -> Option<NodeIdx> {
        if let Some(idx) = heap.pop() {
            Some(idx)
        } else if *inline_len > 0 {
            *inline_len -= 1;
            Some(inline[*inline_len])
        } else {
            None
        }
    }

    #[inline]
    fn push_stack(
        inline: &mut [NodeIdx; INLINE_STACK_CAP],
        inline_len: &mut usize,
        heap: &mut Vec<NodeIdx>,
        idx: NodeIdx,
    ) {
        if !heap.is_empty() || *inline_len == inline.len() {
            heap.push(idx);
        } else {
            inline[*inline_len] = idx;
            *inline_len += 1;
        }
    }
}

impl<T: Scalar> Backend<T> for Bvh<T> {
    fn insert(&mut self, slot: usize, aabb: Aabb2D<T>) {
        self.ensure_slot(slot, aabb);
        match self.root {
            None => {
                let idx = self.arena.len();
                self.arena.push(Node {
                    bbox: aabb,
                    kind: Kind::Leaf(vec![(slot, aabb)]),
                });
                self.root = Some(NodeIdx::new(idx));
            }
            Some(root_idx) => {
                Self::insert_node(&mut self.arena, root_idx.get(), slot, aabb, self.max_leaf);
            }
        }
    }

    fn update(&mut self, slot: usize, aabb: Aabb2D<T>) {
        if let Some(old) = self.slots.get(slot).and_then(|x| *x)
            && let Some(root_idx) = self.root
        {
            let _ = Self::remove_node(&mut self.arena, root_idx.get(), slot, &old);
        }
        self.insert(slot, aabb);
    }

    fn remove(&mut self, slot: usize) {
        if let Some(old) = self.slots.get(slot).and_then(|x| *x)
            && let Some(root_idx) = self.root
        {
            let _ = Self::remove_node(&mut self.arena, root_idx.get(), slot, &old);
            if let Some(s) = self.slots.get_mut(slot) {
                *s = None;
            }
        }
    }

    fn clear(&mut self) {
        self.root = None;
        self.arena.clear();
        self.slots.clear();
    }

    fn visit_point<F: FnMut(usize)>(&self, x: T, y: T, mut f: F) {
        let Some(root_idx) = self.root else {
            return;
        };
        if !self.arena[root_idx.get()].bbox.contains_point(x, y) {
            return;
        }

        let mut inline = [root_idx; INLINE_STACK_CAP];
        let mut inline_len = 1_usize;
        let mut heap = Vec::new();

        while let Some(i) = Self::pop_stack(&mut inline, &mut inline_len, &mut heap) {
            let n = &self.arena[i.get()];
            match &n.kind {
                Kind::Leaf(items) => {
                    for (s, b) in items {
                        if b.contains_point(x, y) {
                            f(*s);
                        }
                    }
                }
                Kind::Internal { left, right } => {
                    let lb = self.arena[left.get()].bbox;
                    let rb = self.arena[right.get()].bbox;
                    if rb.contains_point(x, y) {
                        Self::push_stack(&mut inline, &mut inline_len, &mut heap, *right);
                    }
                    if lb.contains_point(x, y) {
                        Self::push_stack(&mut inline, &mut inline_len, &mut heap, *left);
                    }
                }
            }
        }
    }

    fn visit_rect<F: FnMut(usize)>(&self, rect: Aabb2D<T>, mut f: F) {
        let Some(root_idx) = self.root else {
            return;
        };
        if !self.arena[root_idx.get()].bbox.overlaps(&rect) {
            return;
        }

        let mut inline = [root_idx; INLINE_STACK_CAP];
        let mut inline_len = 1_usize;
        let mut heap = Vec::new();

        while let Some(i) = Self::pop_stack(&mut inline, &mut inline_len, &mut heap) {
            let n = &self.arena[i.get()];
            match &n.kind {
                Kind::Leaf(items) => {
                    for (s, b) in items {
                        if b.overlaps(&rect) {
                            f(*s);
                        }
                    }
                }
                Kind::Internal { left, right } => {
                    let lb = self.arena[left.get()].bbox;
                    let rb = self.arena[right.get()].bbox;
                    if rb.overlaps(&rect) {
                        Self::push_stack(&mut inline, &mut inline_len, &mut heap, *right);
                    }
                    if lb.overlaps(&rect) {
                        Self::push_stack(&mut inline, &mut inline_len, &mut heap, *left);
                    }
                }
            }
        }
    }
}

impl<T: Scalar> Debug for Bvh<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let total = self.slots.len();
        let alive = self.slots.iter().filter(|e| e.is_some()).count();
        let has_root = self.root.is_some();
        f.debug_struct("Bvh")
            .field("max_leaf", &self.max_leaf)
            .field("arena_nodes", &self.arena.len())
            .field("total_slots", &total)
            .field("alive", &alive)
            .field("has_root", &has_root)
            .finish_non_exhaustive()
    }
}

/// Convenience type aliases for common scalar choices.
/// BVH with f32 coordinates and f64 metrics.
pub type BvhF32 = Bvh<f32>;

/// BVH with f64 coordinates and f64 metrics.
pub type BvhF64 = Bvh<f64>;

/// BVH with i64 coordinates and i128 metrics.
pub type BvhI64 = Bvh<i64>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::flatvec::FlatVec;
    use crate::index::Index;

    fn point_hits<B: Backend<i64>>(backend: &B, x: i64, y: i64) -> Vec<usize> {
        let mut hits = Vec::new();
        backend.visit_point(x, y, |slot| hits.push(slot));
        hits.sort_unstable();
        hits
    }

    fn rect_hits<B: Backend<i64>>(backend: &B, rect: Aabb2D<i64>) -> Vec<usize> {
        let mut hits = Vec::new();
        backend.visit_rect(rect, |slot| hits.push(slot));
        hits.sort_unstable();
        hits
    }

    #[test]
    fn bvh_f64_basic() {
        let mut idx = Index::<f64, u32>::with_bvh();
        let _k1 = idx.insert(Aabb2D::new(0.0, 0.0, 10.0, 10.0), 1);
        let _k2 = idx.insert(Aabb2D::new(5.0, 5.0, 15.0, 15.0), 2);
        let _ = idx.commit();
        let hits: Vec<_> = idx.query_point(6.0, 6.0).collect();
        assert!(hits.len() >= 2);
        let q: Vec<_> = idx
            .query_rect(Aabb2D::new(12.0, 12.0, 20.0, 20.0))
            .collect();
        assert!(!q.is_empty());
    }

    #[test]
    fn bvh_f64_update_move_correctness() {
        // Use backend directly to inspect structure behavior on updates.
        let mut b: Bvh<f64> = Bvh::default();
        b.insert(0, Aabb2D::new(0.0, 0.0, 10.0, 10.0));
        b.insert(1, Aabb2D::new(12.0, 0.0, 22.0, 10.0));

        let arena_before = b.arena.len();
        let root_leaf_before = b
            .root
            .map(|ri| matches!(b.arena[ri.get()].kind, Kind::Leaf(_)))
            .unwrap_or(false);

        // Move slot 0 far away; our remove+insert path should keep a valid tree
        // without gratuitous node growth for this tiny case.
        b.update(0, Aabb2D::new(100.0, 100.0, 110.0, 110.0));

        // Arena size should not grow for this small case; root leaf-ness unchanged or becomes internal
        // is acceptable, but for two items it should remain a leaf.
        assert_eq!(b.arena.len(), arena_before);
        let root_leaf_after = b
            .root
            .map(|ri| matches!(b.arena[ri.get()].kind, Kind::Leaf(_)))
            .unwrap_or(false);
        assert_eq!(root_leaf_after, root_leaf_before);

        // Query correctness
        let v_old: Vec<_> = b.query_point(5.0, 5.0).collect();
        assert!(v_old.is_empty());
        let v_new: Vec<_> = b.query_point(105.0, 105.0).collect();
        assert_eq!(v_new, vec![0]);
        let v_neighbor: Vec<_> = b.query_point(15.0, 5.0).collect();
        assert_eq!(v_neighbor, vec![1]);
    }

    #[test]
    fn bvh_i64_update_churn_small() {
        let mut b: Bvh<i64> = Bvh::default();
        b.insert(0, Aabb2D::new(0, 0, 10, 10));
        b.insert(1, Aabb2D::new(12, 0, 22, 10));
        let baseline_nodes = b.arena.len();

        // Move slot 0 back and forth a few times.
        for _ in 0..10 {
            b.update(0, Aabb2D::new(100, 100, 110, 110));
            b.update(0, Aabb2D::new(0, 0, 10, 10));
        }

        // Query correctness stays intact.
        let here: Vec<_> = b.query_point(5, 5).collect();
        assert_eq!(here, vec![0]);
        let there: Vec<_> = b.query_point(105, 105).collect();
        assert!(there.is_empty());

        // Arena size should not explode under small churn.
        assert!(b.arena.len() <= baseline_nodes + 2);
    }

    #[test]
    fn bvh_f64_split_then_updates_on_internal() {
        // Force a split by exceeding max_leaf (8), then update several items and
        // verify the internal-node tree remains correct.
        let mut b: Bvh<f64> = Bvh::default();

        // Build 12 non-overlapping AABBs along the x-axis
        let n = 12_usize;
        let mut current: Vec<Aabb2D<f64>> = Vec::with_capacity(n);
        for i in 0..n {
            let x0 = (i as f64) * 20.0;
            let a = Aabb2D::new(x0, 0.0, x0 + 10.0, 10.0);
            current.push(a);
            b.insert(i, a);
        }

        // Ensure we created an internal root with two children after split
        let root = b.root.expect("root exists").get();
        match b.arena[root].kind {
            Kind::Internal { left, right } => {
                assert!(matches!(b.arena[left.get()].kind, Kind::Leaf(_)));
                assert!(matches!(b.arena[right.get()].kind, Kind::Leaf(_)));
            }
            _ => panic!("expected internal root after split"),
        }

        let baseline_nodes = b.arena.len();

        // Move three items far away (to another cluster)
        for &i in &[0_usize, 5, 9] {
            let new_bb = Aabb2D::new(
                1000.0 + i as f64 * 5.0,
                1000.0,
                1010.0 + i as f64 * 5.0,
                1010.0,
            );
            b.update(i, new_bb);
            current[i] = new_bb;
        }

        // Validate: each item's midpoint hits exactly that slot
        for (i, bb) in current.iter().enumerate() {
            let mx = (bb.min_x + bb.max_x) * 0.5;
            let my = (bb.min_y + bb.max_y) * 0.5;
            let hits: Vec<_> = b.query_point(mx, my).collect();
            assert_eq!(hits, vec![i], "midpoint lookup must return the slot itself");
        }

        // Structure sanity: arena should not grow unboundedly due to updates
        assert!(b.arena.len() <= baseline_nodes + 4);
    }

    #[test]
    fn bvh_queries_match_flatvec_after_updates_and_removes() {
        let mut bvh: Bvh<i64> = Bvh::default();
        let mut flat: FlatVec<i64> = FlatVec::default();

        for slot in 0..18 {
            let x = (slot % 6) as i64 * 20;
            let y = (slot / 6) as i64 * 20;
            let bbox = Aabb2D::new(x, y, x + 12, y + 12);
            bvh.insert(slot, bbox);
            flat.insert(slot, bbox);
        }

        for (slot, bbox) in [
            (1, Aabb2D::new(130, 0, 145, 15)),
            (8, Aabb2D::new(-20, 40, -5, 55)),
            (14, Aabb2D::new(55, -30, 75, -10)),
        ] {
            bvh.update(slot, bbox);
            flat.update(slot, bbox);
        }

        for slot in [0, 5, 12, 17] {
            bvh.remove(slot);
            flat.remove(slot);
        }

        for (x, y) in [
            (6, 6),
            (25, 5),
            (135, 7),
            (-10, 48),
            (65, -20),
            (90, 50),
            (1_000, 1_000),
        ] {
            assert_eq!(point_hits(&bvh, x, y), point_hits(&flat, x, y));
        }

        for rect in [
            Aabb2D::new(-25, -35, 150, 60),
            Aabb2D::new(0, 0, 40, 40),
            Aabb2D::new(80, 0, 130, 80),
            Aabb2D::new(500, 500, 600, 600),
        ] {
            assert_eq!(rect_hits(&bvh, rect), rect_hits(&flat, rect));
        }
    }
}
