// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::hash::Hash;

use hashbrown::{HashMap, HashSet};
use smallvec::SmallVec;

use crate::{
    ImagePrimitive, PathPrimitive, PlainTextPrimitive, Primitive, SurfacePrimitive, TextPrimitive,
};

/// Retained presentation data for one geometry node.
///
/// `SourceKey` is chosen by the host, typically a widget or element id used for
/// diagnostics and template back-references.
#[derive(Clone, Debug, PartialEq)]
pub struct PresentationNode<SourceKey, ImageKey = u64> {
    source: SourceKey,
    primitives: SmallVec<[Primitive<ImageKey>; 1]>,
}

impl<SourceKey, ImageKey> PresentationNode<SourceKey, ImageKey> {
    /// Creates an empty presentation node for `source`.
    #[must_use]
    pub const fn new(source: SourceKey) -> Self {
        Self {
            source,
            primitives: SmallVec::new_const(),
        }
    }

    /// Returns the source widget or element key.
    #[must_use]
    pub const fn source(&self) -> &SourceKey {
        &self.source
    }

    /// Returns a mutable reference to the source widget or element key.
    #[must_use]
    pub const fn source_mut(&mut self) -> &mut SourceKey {
        &mut self.source
    }

    /// Replaces the source widget or element key.
    pub fn set_source(&mut self, source: SourceKey) {
        self.source = source;
    }

    /// Returns the resolved primitives stored on this node.
    #[must_use]
    pub fn primitives(&self) -> &[Primitive<ImageKey>] {
        &self.primitives
    }

    /// Returns mutable access to the resolved primitive list.
    #[must_use]
    pub fn primitives_mut(&mut self) -> &mut SmallVec<[Primitive<ImageKey>; 1]> {
        &mut self.primitives
    }

    /// Clears all primitives from this node.
    pub fn clear_primitives(&mut self) {
        self.primitives.clear();
    }

    /// Returns the first surface primitive, if present.
    #[must_use]
    pub fn surface(&self) -> Option<&SurfacePrimitive> {
        self.primitives.iter().find_map(Primitive::as_surface)
    }

    /// Returns the first surface primitive, creating a default one if needed.
    #[must_use]
    pub fn surface_mut(&mut self) -> &mut SurfacePrimitive {
        let index = self
            .primitives
            .iter()
            .position(|primitive| matches!(primitive, Primitive::Surface(_)))
            .unwrap_or_else(|| {
                self.primitives
                    .push(Primitive::surface(SurfacePrimitive::default()));
                self.primitives.len() - 1
            });

        let Primitive::Surface(surface) = &mut self.primitives[index] else {
            unreachable!("surface index points at a surface primitive");
        };
        surface.as_mut()
    }

    /// Returns the first text primitive, if present.
    #[must_use]
    pub fn text(&self) -> Option<&TextPrimitive> {
        self.primitives.iter().find_map(Primitive::as_text)
    }

    /// Returns the first text primitive, creating plain text if needed.
    #[must_use]
    pub fn text_mut(&mut self) -> &mut TextPrimitive {
        let index = self
            .primitives
            .iter()
            .position(|primitive| matches!(primitive, Primitive::Text(_)))
            .unwrap_or_else(|| {
                self.primitives
                    .push(Primitive::text(TextPrimitive::default()));
                self.primitives.len() - 1
            });

        let Primitive::Text(text) = &mut self.primitives[index] else {
            unreachable!("text index points at a text primitive");
        };
        text.as_mut()
    }

    /// Returns the first plain text primitive, if present.
    #[must_use]
    pub fn plain_text(&self) -> Option<&PlainTextPrimitive> {
        self.primitives
            .iter()
            .find_map(|primitive| primitive.as_text().and_then(TextPrimitive::as_plain))
    }

    /// Returns the first plain text primitive, creating one if needed.
    #[must_use]
    pub fn plain_text_mut(&mut self) -> &mut PlainTextPrimitive {
        let index = self
            .primitives
            .iter()
            .position(|primitive| match primitive {
                Primitive::Text(text) => text.as_plain().is_some(),
                Primitive::Surface(_) | Primitive::Image(_) | Primitive::Path(_) => false,
            })
            .unwrap_or_else(|| {
                self.primitives
                    .push(Primitive::plain_text(PlainTextPrimitive::default()));
                self.primitives.len() - 1
            });

        let Primitive::Text(text) = &mut self.primitives[index] else {
            unreachable!("text index points at a text primitive");
        };
        text.as_plain_mut()
            .expect("plain text index points at a plain text primitive")
    }

    /// Returns the first image primitive, if present.
    #[must_use]
    pub fn image(&self) -> Option<&ImagePrimitive<ImageKey>> {
        self.primitives.iter().find_map(Primitive::as_image)
    }

    /// Returns mutable access to the first image primitive, if present.
    #[must_use]
    pub fn image_mut(&mut self) -> Option<&mut ImagePrimitive<ImageKey>> {
        self.primitives.iter_mut().find_map(Primitive::as_image_mut)
    }

    /// Replaces the first image primitive or appends one when none exists.
    pub fn set_image(&mut self, image: ImagePrimitive<ImageKey>) -> &mut ImagePrimitive<ImageKey> {
        if let Some(index) = self
            .primitives
            .iter()
            .position(|primitive| matches!(primitive, Primitive::Image(_)))
        {
            let Primitive::Image(existing) = &mut self.primitives[index] else {
                unreachable!("image index points at an image primitive");
            };
            **existing = image;
            existing.as_mut()
        } else {
            self.primitives.push(Primitive::image(image));
            let Some(Primitive::Image(inserted)) = self.primitives.last_mut() else {
                unreachable!("last inserted primitive is an image");
            };
            inserted.as_mut()
        }
    }

    /// Returns the first path primitive, if present.
    #[must_use]
    pub fn path(&self) -> Option<&PathPrimitive> {
        self.primitives.iter().find_map(Primitive::as_path)
    }

    /// Returns mutable access to the first path primitive, if present.
    #[must_use]
    pub fn path_mut(&mut self) -> Option<&mut PathPrimitive> {
        self.primitives.iter_mut().find_map(Primitive::as_path_mut)
    }

    /// Replaces the first path primitive or appends one when none exists.
    pub fn set_path(&mut self, path: PathPrimitive) -> &mut PathPrimitive {
        if let Some(index) = self
            .primitives
            .iter()
            .position(|primitive| matches!(primitive, Primitive::Path(_)))
        {
            let Primitive::Path(existing) = &mut self.primitives[index] else {
                unreachable!("path index points at a path primitive");
            };
            **existing = path;
            existing.as_mut()
        } else {
            self.primitives.push(Primitive::path(path));
            let Some(Primitive::Path(inserted)) = self.primitives.last_mut() else {
                unreachable!("last inserted primitive is a path");
            };
            inserted.as_mut()
        }
    }
}

/// Flat keyed store of retained presentation nodes.
///
/// `NodeKey` is chosen by the host, typically a geometry key such as a
/// box-tree node id. This store owns no tree structure or layout/scene
/// geometry; callers paint by walking their own geometry tree and looking up
/// nodes here. Individual primitives may still own local drawing geometry.
#[derive(Clone, Debug)]
pub struct PresentationStore<NodeKey, SourceKey, ImageKey = u64> {
    nodes: HashMap<NodeKey, PresentationNode<SourceKey, ImageKey>>,
    order: Vec<NodeKey>,
    dirty: Vec<NodeKey>,
    dirty_set: HashSet<NodeKey>,
}

impl<NodeKey, SourceKey, ImageKey> Default for PresentationStore<NodeKey, SourceKey, ImageKey>
where
    NodeKey: Copy + Eq + Hash,
{
    fn default() -> Self {
        Self::empty()
    }
}

impl<NodeKey, SourceKey, ImageKey> PresentationStore<NodeKey, SourceKey, ImageKey>
where
    NodeKey: Copy + Eq + Hash,
{
    fn empty() -> Self {
        Self {
            nodes: HashMap::new(),
            order: Vec::new(),
            dirty: Vec::new(),
            dirty_set: HashSet::new(),
        }
    }

    /// Creates an empty presentation store.
    #[must_use]
    pub fn new() -> Self {
        Self::empty()
    }

    /// Returns the number of live presentation nodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns true when the store has no live presentation nodes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns true when `key` has a live presentation node.
    #[must_use]
    pub fn contains_key(&self, key: NodeKey) -> bool {
        self.nodes.contains_key(&key)
    }

    /// Inserts or replaces a presentation node and marks `key` dirty.
    ///
    /// Replacing an existing key preserves its insertion-order position.
    pub fn insert(
        &mut self,
        key: NodeKey,
        source: SourceKey,
    ) -> Option<PresentationNode<SourceKey, ImageKey>> {
        let old = self.nodes.insert(key, PresentationNode::new(source));
        if old.is_none() {
            self.order.push(key);
        }
        self.mark_dirty(key);
        old
    }

    /// Removes a presentation node, marking `key` dirty if it was live.
    pub fn remove(&mut self, key: NodeKey) -> Option<PresentationNode<SourceKey, ImageKey>> {
        let old = self.nodes.remove(&key)?;
        self.order.retain(|ordered| *ordered != key);
        self.mark_dirty(key);
        Some(old)
    }

    /// Clears all presentation nodes and marks every previously live key dirty.
    pub fn clear(&mut self) {
        let keys = core::mem::take(&mut self.order);
        for key in keys {
            self.mark_dirty(key);
        }
        self.nodes.clear();
    }

    /// Returns a presentation node by key.
    #[must_use]
    pub fn node(&self, key: NodeKey) -> Option<&PresentationNode<SourceKey, ImageKey>> {
        self.nodes.get(&key)
    }

    /// Returns a mutable presentation node by key and marks it dirty.
    pub fn node_mut(&mut self, key: NodeKey) -> Option<&mut PresentationNode<SourceKey, ImageKey>> {
        if self.nodes.contains_key(&key) {
            self.mark_dirty(key);
        }
        self.nodes.get_mut(&key)
    }

    /// Returns the first surface primitive on a node, creating it if needed.
    ///
    /// Returns `None` when `key` does not identify a live presentation node.
    pub fn surface_mut(&mut self, key: NodeKey) -> Option<&mut SurfacePrimitive> {
        self.node_mut(key).map(PresentationNode::surface_mut)
    }

    /// Returns the first text primitive on a node, creating it if needed.
    ///
    /// Returns `None` when `key` does not identify a live presentation node.
    pub fn text_mut(&mut self, key: NodeKey) -> Option<&mut TextPrimitive> {
        self.node_mut(key).map(PresentationNode::text_mut)
    }

    /// Returns the first plain text primitive on a node, creating it if needed.
    ///
    /// Returns `None` when `key` does not identify a live presentation node.
    pub fn plain_text_mut(&mut self, key: NodeKey) -> Option<&mut PlainTextPrimitive> {
        self.node_mut(key).map(PresentationNode::plain_text_mut)
    }

    /// Returns the first image primitive on a node.
    ///
    /// Returns `None` when `key` does not identify a live presentation node or
    /// when the node has no image primitive.
    pub fn image_mut(&mut self, key: NodeKey) -> Option<&mut ImagePrimitive<ImageKey>> {
        if self
            .nodes
            .get(&key)
            .and_then(PresentationNode::image)
            .is_some()
        {
            self.mark_dirty(key);
        }
        self.nodes
            .get_mut(&key)
            .and_then(PresentationNode::image_mut)
    }

    /// Replaces or appends the first image primitive on a node.
    ///
    /// Returns `None` when `key` does not identify a live presentation node.
    pub fn set_image(
        &mut self,
        key: NodeKey,
        image: ImagePrimitive<ImageKey>,
    ) -> Option<&mut ImagePrimitive<ImageKey>> {
        self.node_mut(key).map(|node| node.set_image(image))
    }

    /// Returns the first path primitive on a node.
    ///
    /// Returns `None` when `key` does not identify a live presentation node or
    /// when the node has no path primitive.
    pub fn path_mut(&mut self, key: NodeKey) -> Option<&mut PathPrimitive> {
        if self
            .nodes
            .get(&key)
            .and_then(PresentationNode::path)
            .is_some()
        {
            self.mark_dirty(key);
        }
        self.nodes
            .get_mut(&key)
            .and_then(PresentationNode::path_mut)
    }

    /// Replaces or appends the first path primitive on a node.
    ///
    /// Returns `None` when `key` does not identify a live presentation node.
    pub fn set_path(&mut self, key: NodeKey, path: PathPrimitive) -> Option<&mut PathPrimitive> {
        self.node_mut(key).map(|node| node.set_path(path))
    }

    /// Returns live keys in insertion order.
    ///
    /// This order is useful for diagnostics and listing. It is not paint order:
    /// callers should walk their own geometry tree and use [`Self::node`] to
    /// look up presentation data for each geometry key.
    pub fn keys(&self) -> impl Iterator<Item = NodeKey> + '_ {
        self.order.iter().copied()
    }

    /// Returns live nodes in insertion order.
    ///
    /// This order is useful for diagnostics and listing. It is not paint order:
    /// callers should walk their own geometry tree and use [`Self::node`] to
    /// look up presentation data for each geometry key.
    pub fn nodes(
        &self,
    ) -> impl Iterator<Item = (NodeKey, &PresentationNode<SourceKey, ImageKey>)> + '_ {
        self.order
            .iter()
            .copied()
            .filter_map(|key| self.nodes.get(&key).map(|node| (key, node)))
    }

    /// Marks `key` dirty if it is not already dirty.
    ///
    /// This is useful when data outside the presentation node changes but paint
    /// should still revisit the node.
    pub fn mark_dirty(&mut self, key: NodeKey) {
        if self.dirty_set.insert(key) {
            self.dirty.push(key);
        }
    }

    /// Returns true when `key` is currently dirty.
    #[must_use]
    pub fn is_dirty(&self, key: NodeKey) -> bool {
        self.dirty_set.contains(&key)
    }

    /// Returns the number of currently dirty keys.
    #[must_use]
    pub fn dirty_len(&self) -> usize {
        self.dirty.len()
    }

    /// Returns true when no keys are currently dirty.
    #[must_use]
    pub fn dirty_is_empty(&self) -> bool {
        self.dirty.is_empty()
    }

    /// Clears dirty state without yielding the dirty keys.
    pub fn clear_dirty(&mut self) {
        self.dirty.clear();
        self.dirty_set.clear();
    }

    /// Drains dirty keys in first-dirty order.
    ///
    /// Dirty tracking has set semantics: each key is yielded at most once until
    /// it is marked dirty again.
    pub fn take_dirty(&mut self) -> impl Iterator<Item = NodeKey> + '_ {
        self.dirty_set.clear();
        self.dirty.drain(..)
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;
    use crate::{Color, PathPrimitive, RoundedRectRadii, TextContent};
    use peniko::kurbo::BezPath;

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct Asset(&'static str);
    #[test]
    fn insert_tracks_nodes_and_dirty_order() {
        let mut store: PresentationStore<i32, i32> = PresentationStore::new();

        assert_eq!(store.insert(1, 10), None);
        assert_eq!(store.insert(2, 10), None);

        assert_eq!(store.len(), 2);
        assert_eq!(store.keys().collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(store.take_dirty().collect::<Vec<_>>(), vec![1, 2]);
        assert!(store.dirty_is_empty());
    }

    #[test]
    fn replacing_node_preserves_order_and_returns_old_node() {
        let mut store: PresentationStore<i32, i32> = PresentationStore::new();
        store.insert(1, 10);
        store.insert(2, 10);
        store.clear_dirty();

        let old = store.insert(1, 11).unwrap();

        assert_eq!(old.source(), &10);
        assert_eq!(store.keys().collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(store.node(1).unwrap().source(), &11);
        assert_eq!(store.take_dirty().collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn surface_mut_creates_surface_and_dedupes_dirty() {
        let mut store: PresentationStore<i32, i32> = PresentationStore::new();
        store.insert(1, 10);
        store.clear_dirty();

        let surface = store.surface_mut(1).unwrap();
        surface.set_background(Color::from_rgb8(20, 30, 40));

        let surface = store.surface_mut(1).unwrap();
        surface.corner_radii = RoundedRectRadii::from_single_radius(4.0);

        assert!(store.node(1).unwrap().surface().is_some());
        assert_eq!(store.take_dirty().collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn plain_text_mut_creates_plain_text() {
        let mut store: PresentationStore<i32, i32> = PresentationStore::new();
        store.insert(1, 10);
        store.clear_dirty();

        let text = store.plain_text_mut(1).unwrap();
        text.content = TextContent::plain("Apply");

        assert_eq!(
            store
                .node(1)
                .unwrap()
                .plain_text()
                .unwrap()
                .content
                .as_str(),
            "Apply"
        );
        assert_eq!(store.take_dirty().collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn set_image_uses_host_image_key_type() {
        let mut store = PresentationStore::<u32, u32, Asset>::new();
        store.insert(1, 10);
        store.clear_dirty();

        let image = store
            .set_image(1, ImagePrimitive::new(Asset("button-background")))
            .unwrap();

        assert_eq!(image.brush.image, Asset("button-background"));
        assert_eq!(
            store.node(1).unwrap().image().unwrap().brush.image,
            Asset("button-background")
        );
        assert_eq!(store.take_dirty().collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn set_image_replaces_existing_image() {
        let mut store: PresentationStore<i32, i32> = PresentationStore::new();
        store.insert(1, 10);

        store.set_image(1, ImagePrimitive::new(42));
        store.set_image(1, ImagePrimitive::new(43));

        let node = store.node(1).unwrap();
        assert_eq!(node.primitives().len(), 1);
        assert_eq!(node.image().unwrap().brush.image, 43);
    }

    #[test]
    fn set_path_replaces_existing_path() {
        let mut store: PresentationStore<i32, i32> = PresentationStore::new();
        store.insert(1, 10);
        store.clear_dirty();

        store.set_path(1, PathPrimitive::new(BezPath::new()));
        let mut path = BezPath::new();
        path.move_to((1.0, 2.0));
        path.line_to((3.0, 4.0));
        store.set_path(1, PathPrimitive::new(path.clone()));

        let node = store.node(1).unwrap();
        assert_eq!(node.primitives().len(), 1);
        assert_eq!(node.path().unwrap().path, path);
        assert_eq!(store.take_dirty().collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn image_mut_does_not_create_default_image() {
        let mut store: PresentationStore<i32, i32> = PresentationStore::new();
        store.insert(1, 10);
        store.clear_dirty();

        assert!(store.image_mut(1).is_none());
        assert!(store.dirty_is_empty());
    }

    #[test]
    fn missing_primitive_helpers_return_none_without_dirty() {
        let mut store: PresentationStore<u32, u32> = PresentationStore::new();

        assert!(store.surface_mut(99).is_none());
        assert!(store.text_mut(99).is_none());
        assert!(store.plain_text_mut(99).is_none());
        assert!(store.image_mut(99).is_none());
        assert!(store.path_mut(99).is_none());
        assert!(store.dirty_is_empty());
    }

    #[test]
    fn remove_marks_existing_key_dirty() {
        let mut store: PresentationStore<i32, i32> = PresentationStore::new();
        store.insert(1, 10);
        store.insert(2, 10);
        store.clear_dirty();

        assert!(store.remove(1).is_some());
        assert!(store.node(1).is_none());
        assert_eq!(store.keys().collect::<Vec<_>>(), vec![2]);
        assert_eq!(store.take_dirty().collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn clear_marks_live_keys_dirty_in_insertion_order() {
        let mut store: PresentationStore<i32, i32> = PresentationStore::new();
        store.insert(1, 10);
        store.insert(2, 10);
        store.clear_dirty();

        store.clear();

        assert!(store.is_empty());
        assert_eq!(store.take_dirty().collect::<Vec<_>>(), vec![1, 2]);
    }
}
