// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::TypeId;
use core::fmt;
use core::hash::Hash;

use hashbrown::{HashMap, HashSet};
use invalidation::{Channel, CycleHandling, InvalidationTracker};
use understory_property::ErasedValue;

use crate::endpoint::{BindingId, EndpointKey, PropertyEndpoint};
use crate::error::{BindingDrainError, BindingError};
use crate::host::BindingHost;
use crate::report::{BindingReport, BindingStats, BindingWrite};

trait ErasedBinding<K: Copy, C: Copy> {
    fn source(&self) -> EndpointKey<K>;
    fn target(&self) -> EndpointKey<K>;
    fn evaluate(
        &self,
        binding: BindingId,
        host: &mut dyn BindingHost<K, C>,
    ) -> Result<BindingWrite, BindingError<K>>;
}

struct TypedBinding<K, C, S, T> {
    source: PropertyEndpoint<K, S>,
    target: PropertyEndpoint<K, T>,
    write_context: C,
    map: Box<dyn Fn(&S) -> T>,
}

impl<K, C, S, T> ErasedBinding<K, C> for TypedBinding<K, C, S, T>
where
    K: Copy,
    C: Copy,
    S: Clone + 'static,
    T: Clone + 'static,
{
    fn source(&self) -> EndpointKey<K> {
        self.source.key()
    }

    fn target(&self) -> EndpointKey<K> {
        self.target.key()
    }

    fn evaluate(
        &self,
        binding: BindingId,
        host: &mut dyn BindingHost<K, C>,
    ) -> Result<BindingWrite, BindingError<K>> {
        let source = self.source();
        let erased = host.get_erased(source).ok_or(BindingError::MissingSource {
            binding,
            endpoint: source,
        })?;
        let source_value =
            erased
                .downcast_ref::<S>()
                .ok_or_else(|| BindingError::SourceTypeMismatch {
                    binding,
                    endpoint: source,
                    expected: TypeId::of::<S>(),
                    actual: erased.type_id(),
                })?;
        let target_value = (self.map)(source_value);
        Ok(host.set_erased(
            self.target(),
            ErasedValue::new(target_value),
            self.write_context,
        ))
    }
}

/// Registered one-way bindings, write contexts, and dirty state.
///
/// The set stores bindings, endpoint indexes, and a binding-local invalidation
/// graph. It does not store property values; values are read from and written to
/// a host passed to [`Self::drain`].
///
/// `C` is binding-owned write context. Each binding stores one `C` and passes it
/// to [`BindingHost::set_erased`] whenever the binding writes its target. The
/// binding set orders and transports the context but does not interpret it.
/// Hosts that do not need write metadata use the default `()` context.
///
/// For example, Overstory can use
/// `BindingSet<BindingOwner, LocalValueSource>`. App-authored bindings register
/// with `LocalValueSource::Local`, while template-owner-to-part bindings
/// register with `LocalValueSource::TemplateBinding`. During drain, the UI host
/// receives that context and writes the target property into the correct local
/// value layer. Plain hosts use `BindingSet<Owner>` and pass `()` when
/// registering bindings.
pub struct BindingSet<K, C = ()>
where
    K: Copy + Eq + Hash + 'static,
    C: Copy + 'static,
{
    binding_channel: Channel,
    bindings: Vec<Option<Box<dyn ErasedBinding<K, C>>>>,
    active_bindings: usize,
    source_index: HashMap<EndpointKey<K>, Vec<BindingId>>,
    target_index: HashMap<EndpointKey<K>, Vec<BindingId>>,
    tracker: InvalidationTracker<u32>,
}

impl<K, C> BindingSet<K, C>
where
    K: Copy + Eq + Hash + 'static,
    C: Copy + 'static,
{
    /// Creates an empty binding set.
    ///
    /// The channel is used only inside this binding set's tracker. It does not
    /// reserve or define an application-level invalidation channel.
    #[must_use]
    pub fn new(binding_channel: Channel) -> Self {
        Self {
            binding_channel,
            bindings: Vec::new(),
            active_bindings: 0,
            source_index: HashMap::new(),
            target_index: HashMap::new(),
            tracker: InvalidationTracker::with_cycle_handling(CycleHandling::Error),
        }
    }

    /// Returns the binding-local invalidation channel.
    #[must_use]
    pub const fn binding_channel(&self) -> Channel {
        self.binding_channel
    }

    /// Returns the number of active bindings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.active_bindings
    }

    /// Returns `true` when no bindings are active.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.active_bindings == 0
    }

    /// Returns a snapshot of binding set structure for diagnostics.
    #[must_use]
    pub fn stats(&self) -> BindingStats {
        BindingStats::new(
            self.active_bindings,
            self.bindings.len(),
            self.source_index.len(),
            self.target_index.len(),
            self.dependency_edge_count(),
            self.dirty_binding_count(),
            self.has_dirty_bindings(),
        )
    }

    /// Returns `true` when there are dirty bindings waiting to be drained.
    #[must_use]
    pub fn has_dirty_bindings(&self) -> bool {
        self.tracker.has_invalidated(self.binding_channel)
    }

    /// Registers a one-way identity binding.
    ///
    /// The source and target endpoint value types must match. Use
    /// [`Self::bind_map`] when the target type differs or a conversion is
    /// needed. `write_context` is stored with this binding and passed to
    /// [`BindingHost::set_erased`] whenever this binding writes its target.
    pub fn bind<T>(
        &mut self,
        source: PropertyEndpoint<K, T>,
        target: PropertyEndpoint<K, T>,
        write_context: C,
    ) -> Result<BindingId, BindingError<K>>
    where
        T: Clone + 'static,
    {
        self.bind_map(source, target, write_context, T::clone)
    }

    /// Registers a one-way mapped binding.
    ///
    /// `map` runs when the source endpoint is dirty and produces the value to
    /// write to the target endpoint. `write_context` is stored with this
    /// binding and passed to [`BindingHost::set_erased`] whenever this binding
    /// writes its target. Only one active binding may write a target endpoint;
    /// use [`Self::unbind`] or [`Self::clear_endpoint`] before replacing an
    /// existing writer.
    ///
    /// Registration clones the internal binding-local invalidation tracker before
    /// adding dependency edges so cycle errors leave the existing set unchanged.
    /// That keeps the API transactional, but makes registration cost scale with
    /// the current graph size.
    pub fn bind_map<S, T, F>(
        &mut self,
        source: PropertyEndpoint<K, S>,
        target: PropertyEndpoint<K, T>,
        write_context: C,
        map: F,
    ) -> Result<BindingId, BindingError<K>>
    where
        S: Clone + 'static,
        T: Clone + 'static,
        F: Fn(&S) -> T + 'static,
    {
        let source_key = source.key();
        let target_key = target.key();
        if source_key == target_key {
            return Err(BindingError::SelfBinding {
                endpoint: source_key,
            });
        }
        if let Some(existing) = self.active_target_binding(target_key) {
            return Err(BindingError::TargetAlreadyBound {
                target: target_key,
                existing,
            });
        }

        let raw = u32::try_from(self.bindings.len()).map_err(|_| BindingError::TooManyBindings)?;
        let id = BindingId::new(raw);
        let dependencies = self.binding_dependencies(source_key, target_key, id);

        let mut tracker = self.tracker.clone();
        for (dependent, dependency) in dependencies {
            tracker
                .add_dependency(dependent.get(), dependency.get(), self.binding_channel)
                .map_err(|_| BindingError::Cycle {
                    dependent,
                    dependency,
                })?;
        }
        self.tracker = tracker;

        self.bindings.push(Some(Box::new(TypedBinding {
            source,
            target,
            write_context,
            map: Box::new(map),
        })));
        self.active_bindings += 1;
        self.source_index.entry(source_key).or_default().push(id);
        self.target_index.entry(target_key).or_default().push(id);

        Ok(id)
    }

    /// Removes one binding.
    ///
    /// Returns `false` when the id is unknown or was already removed. Binding
    /// ids are stable; removed ids are not reused by later bindings.
    pub fn unbind(&mut self, binding: BindingId) -> bool {
        let Some(index) = binding.index() else {
            return false;
        };
        let Some(slot) = self.bindings.get(index) else {
            return false;
        };
        let Some(erased) = slot.as_ref() else {
            return false;
        };

        let source = erased.source();
        let target = erased.target();
        self.bindings[index] = None;
        self.active_bindings -= 1;
        self.tracker.remove_key(binding.get());
        Self::remove_index_entry(&mut self.source_index, source, binding);
        Self::remove_index_entry(&mut self.target_index, target, binding);
        true
    }

    /// Removes active bindings that either read from or write to `endpoint`.
    ///
    /// Returns the number of bindings removed.
    pub fn clear_endpoint(&mut self, endpoint: EndpointKey<K>) -> usize {
        let bindings = self.collect_active_bindings(|binding| {
            binding.source() == endpoint || binding.target() == endpoint
        });
        self.unbind_all(bindings)
    }

    /// Removes active bindings owned by `owner`.
    ///
    /// A binding is removed when either its source owner or target owner matches
    /// `owner`. This is the common teardown path for retained template instances
    /// and other host-owned object groups.
    pub fn clear_owner(&mut self, owner: K) -> usize {
        let bindings = self.collect_active_bindings(|binding| {
            binding.source().owner() == owner || binding.target().owner() == owner
        });
        self.unbind_all(bindings)
    }

    /// Marks all bindings that read `source` as dirty.
    ///
    /// Hosts should call this after an external write changes an endpoint's
    /// observable value.
    pub fn mark_source_changed<T>(&mut self, source: PropertyEndpoint<K, T>) -> bool {
        self.mark_endpoint_changed(source.key())
    }

    /// Marks all bindings that read `source` as dirty using an untyped endpoint key.
    pub fn mark_endpoint_changed(&mut self, source: EndpointKey<K>) -> bool {
        self.mark_endpoint_changed_skipping(source, None)
    }

    /// Marks a specific binding as dirty.
    ///
    /// Returns `false` when the binding id is not registered.
    pub fn mark_binding_dirty(&mut self, binding: BindingId) -> bool {
        if !self.is_active(binding) {
            return false;
        }
        self.tracker.mark(binding.get(), self.binding_channel)
    }

    /// Evaluates dirty bindings until the binding set is clean.
    ///
    /// Bindings dirtied at the same time are evaluated in dependency order.
    /// When a binding target changes, bindings that read that target are marked
    /// dirty for a later pass unless they are already scheduled in the current
    /// pass.
    ///
    /// A binding whose source has no value (the host returns `None` from
    /// `get_erased`) is a no-op: the binding is skipped, stays clean, and
    /// is recorded in [`BindingReport::skipped_missing_source`]. The
    /// expectation is that the host will dirty the binding again via
    /// [`Self::mark_endpoint_changed`] once the source becomes available.
    /// This treats "source not yet provided" as a normal pending state
    /// rather than a drain-aborting error.
    ///
    /// Other evaluation failures (type mismatch, …) abort the drain. Writes
    /// that already happened are not rolled back. The failed binding and the
    /// rest of the current dirty batch are marked dirty again so the caller
    /// can repair the host state and retry.
    pub fn drain(
        &mut self,
        host: &mut dyn BindingHost<K, C>,
    ) -> Result<BindingReport, BindingDrainError<K>> {
        let mut report = BindingReport::default();

        while self.tracker.has_invalidated(self.binding_channel) {
            let raw_bindings: Vec<_> = self
                .tracker
                .drain_sorted_deterministic(self.binding_channel)
                .collect();
            let mut current_batch = HashSet::with_capacity(raw_bindings.len());
            current_batch.extend(raw_bindings.iter().copied());

            for (index, raw) in raw_bindings.iter().copied().enumerate() {
                match self.evaluate_raw(raw, host) {
                    Ok(Some((target, write))) => {
                        report.record(write);
                        if write.did_change() {
                            self.mark_endpoint_changed_skipping(target, Some(&current_batch));
                        }
                    }
                    Ok(None) => {}
                    Err(BindingError::MissingSource { .. }) => {
                        report.record_skipped_missing_source();
                    }
                    Err(error) => {
                        self.remark_dirty_batch(&raw_bindings[index..]);
                        return Err(BindingDrainError::new(error, report));
                    }
                }
            }
        }

        Ok(report)
    }

    fn binding_dependencies(
        &self,
        source: EndpointKey<K>,
        target: EndpointKey<K>,
        id: BindingId,
    ) -> Vec<(BindingId, BindingId)> {
        let mut dependencies = Vec::new();

        if let Some(upstream) = self.target_index.get(&source) {
            dependencies.extend(
                upstream
                    .iter()
                    .copied()
                    .filter(|dependency| self.is_active(*dependency))
                    .map(|dependency| (id, dependency)),
            );
        }

        if let Some(downstream) = self.source_index.get(&target) {
            dependencies.extend(
                downstream
                    .iter()
                    .copied()
                    .filter(|dependent| self.is_active(*dependent))
                    .map(|dependent| (dependent, id)),
            );
        }

        dependencies
    }

    fn mark_endpoint_changed_skipping(
        &mut self,
        source: EndpointKey<K>,
        skip: Option<&HashSet<u32>>,
    ) -> bool {
        let Some(bindings) = self.source_index.get(&source) else {
            return false;
        };
        let bindings = bindings.to_vec();
        let mut marked = false;
        for binding in bindings {
            if !self.is_active(binding) {
                continue;
            }
            if skip.is_some_and(|set| set.contains(&binding.get())) {
                continue;
            }
            marked |= self.tracker.mark(binding.get(), self.binding_channel);
        }
        marked
    }

    fn evaluate_raw(
        &self,
        raw: u32,
        host: &mut dyn BindingHost<K, C>,
    ) -> Result<Option<(EndpointKey<K>, BindingWrite)>, BindingError<K>> {
        let Some(index) = usize::try_from(raw).ok() else {
            return Ok(None);
        };
        let Some(binding) = self.bindings.get(index) else {
            return Ok(None);
        };
        let Some(binding) = binding.as_ref() else {
            return Ok(None);
        };
        let id = BindingId::new(raw);
        let target = binding.target();
        let write = binding.evaluate(id, host)?;
        Ok(Some((target, write)))
    }

    fn active_target_binding(&self, target: EndpointKey<K>) -> Option<BindingId> {
        self.target_index
            .get(&target)?
            .iter()
            .copied()
            .find(|binding| self.is_active(*binding))
    }

    fn dependency_edge_count(&self) -> usize {
        self.bindings
            .iter()
            .filter_map(Option::as_ref)
            .map(|binding| {
                self.target_index
                    .get(&binding.source())
                    .map_or(0, |dependencies| {
                        dependencies
                            .iter()
                            .filter(|dependency| self.is_active(**dependency))
                            .count()
                    })
            })
            .sum()
    }

    fn dirty_binding_count(&self) -> usize {
        self.tracker
            .peek_sorted_deterministic(self.binding_channel)
            .filter(|raw| self.is_active(BindingId::new(*raw)))
            .count()
    }

    fn is_active(&self, binding: BindingId) -> bool {
        let Some(index) = binding.index() else {
            return false;
        };
        self.bindings.get(index).is_some_and(Option::is_some)
    }

    fn mark_raw_dirty(&mut self, raw: u32) -> bool {
        let binding = BindingId::new(raw);
        if self.is_active(binding) {
            self.tracker.mark(raw, self.binding_channel)
        } else {
            false
        }
    }

    fn remark_dirty_batch(&mut self, bindings: &[u32]) {
        for binding in bindings {
            self.mark_raw_dirty(*binding);
        }
    }

    fn collect_active_bindings<F>(&self, mut predicate: F) -> Vec<BindingId>
    where
        F: FnMut(&dyn ErasedBinding<K, C>) -> bool,
    {
        self.bindings
            .iter()
            .enumerate()
            .filter_map(|(index, binding)| {
                let binding = binding.as_ref()?;
                let raw = u32::try_from(index).ok()?;
                predicate(binding.as_ref()).then(|| BindingId::new(raw))
            })
            .collect()
    }

    fn unbind_all(&mut self, bindings: Vec<BindingId>) -> usize {
        bindings
            .into_iter()
            .filter(|binding| self.unbind(*binding))
            .count()
    }

    fn remove_index_entry(
        index: &mut HashMap<EndpointKey<K>, Vec<BindingId>>,
        endpoint: EndpointKey<K>,
        binding: BindingId,
    ) {
        let should_remove = if let Some(bindings) = index.get_mut(&endpoint) {
            bindings.retain(|candidate| *candidate != binding);
            bindings.is_empty()
        } else {
            false
        };

        if should_remove {
            index.remove(&endpoint);
        }
    }
}

impl<K, C> fmt::Debug for BindingSet<K, C>
where
    K: Copy + Eq + Hash + 'static,
    C: Copy + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingSet")
            .field("binding_channel", &self.binding_channel)
            .field("len", &self.len())
            .field("has_dirty_bindings", &self.has_dirty_bindings())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;
    use alloc::format;
    use alloc::string::String;
    use alloc::vec::Vec;
    use invalidation::{Channel, ChannelSet};
    use understory_property::{
        ErasedValue, Property, PropertyId, PropertyMetadataBuilder, PropertyRegistry,
    };

    use crate::{
        BindingError, BindingHost, BindingId, BindingSet, BindingWrite, EndpointKey,
        PropertyEndpoint,
    };

    const BINDING: Channel = Channel::new(0);
    const LAYOUT: Channel = Channel::new(1);
    const PAINT: Channel = Channel::new(2);

    #[derive(Default)]
    struct TestHost {
        values: BTreeMap<EndpointKey<u32>, ErasedValue>,
        channels: BTreeMap<PropertyId, ChannelSet>,
        writes: Vec<EndpointKey<u32>>,
    }

    impl TestHost {
        fn set_initial<T: Clone + 'static>(
            &mut self,
            endpoint: PropertyEndpoint<u32, T>,
            value: T,
        ) {
            self.values.insert(endpoint.key(), ErasedValue::new(value));
        }

        fn set_channels<T>(&mut self, endpoint: PropertyEndpoint<u32, T>, channels: ChannelSet) {
            self.channels.insert(endpoint.property().id(), channels);
        }

        fn value<T: 'static>(&self, endpoint: PropertyEndpoint<u32, T>) -> Option<&T> {
            self.values
                .get(&endpoint.key())
                .and_then(ErasedValue::downcast_ref)
        }

        fn erased_equal(left: &ErasedValue, right: &ErasedValue) -> bool {
            if left.type_id() != right.type_id() {
                return false;
            }
            if let (Some(left), Some(right)) =
                (left.downcast_ref::<u32>(), right.downcast_ref::<u32>())
            {
                return left == right;
            }
            if let (Some(left), Some(right)) = (
                left.downcast_ref::<String>(),
                right.downcast_ref::<String>(),
            ) {
                return left == right;
            }
            false
        }
    }

    impl BindingHost<u32> for TestHost {
        fn get_erased(&self, endpoint: EndpointKey<u32>) -> Option<ErasedValue> {
            self.values.get(&endpoint).cloned()
        }

        fn set_erased(
            &mut self,
            endpoint: EndpointKey<u32>,
            value: ErasedValue,
            (): (),
        ) -> BindingWrite {
            let changed = self
                .values
                .get(&endpoint)
                .is_none_or(|old| !Self::erased_equal(old, &value));
            self.values.insert(endpoint, value);
            self.writes.push(endpoint);

            let channels = if changed {
                self.channels
                    .get(&endpoint.property())
                    .copied()
                    .unwrap_or_else(ChannelSet::empty)
            } else {
                ChannelSet::empty()
            };

            BindingWrite::new(changed, channels)
        }
    }

    fn registry() -> (PropertyRegistry, Property<u32>) {
        let mut registry = PropertyRegistry::new();
        let width = registry.register("Width", PropertyMetadataBuilder::new(0_u32).build());
        (registry, width)
    }

    #[test]
    fn one_way_binding_copies_changed_value() {
        let (_registry, width) = registry();
        let source = PropertyEndpoint::new(1, width);
        let target = PropertyEndpoint::new(2, width);

        let mut bindings = BindingSet::new(BINDING);
        bindings.bind(source, target, ()).unwrap();

        let mut host = TestHost::default();
        host.set_initial(source, 42_u32);
        host.set_channels(target, LAYOUT.into_set());

        assert!(bindings.mark_source_changed(source));
        assert_eq!(bindings.stats().dirty_bindings(), 1);
        let report = bindings.drain(&mut host).unwrap();

        assert_eq!(report.evaluated_bindings(), 1);
        assert_eq!(report.changed_bindings(), 1);
        assert!(report.affected_channels().contains(LAYOUT));
        assert_eq!(bindings.stats().dirty_bindings(), 0);
        assert_eq!(host.value(target), Some(&42));
    }

    #[test]
    fn drain_accepts_erased_host_boundary() {
        let (_registry, width) = registry();
        let source = PropertyEndpoint::new(1, width);
        let target = PropertyEndpoint::new(2, width);

        let mut bindings = BindingSet::new(BINDING);
        bindings.bind(source, target, ()).unwrap();

        let mut host = TestHost::default();
        host.set_initial(source, 12_u32);
        bindings.mark_source_changed(source);

        let host: &mut dyn BindingHost<u32> = &mut host;
        let report = bindings.drain(host).unwrap();

        assert_eq!(report.evaluated_bindings(), 1);
    }

    #[test]
    fn drain_without_dirty_source_does_nothing() {
        let (_registry, width) = registry();
        let source = PropertyEndpoint::new(1, width);
        let target = PropertyEndpoint::new(2, width);

        let mut bindings = BindingSet::new(BINDING);
        bindings.bind(source, target, ()).unwrap();

        let mut host = TestHost::default();
        host.set_initial(source, 42_u32);

        let report = bindings.drain(&mut host).unwrap();

        assert_eq!(report.evaluated_bindings(), 0);
        assert_eq!(host.value(target), None);
    }

    #[test]
    fn mapped_binding_converts_value_type() {
        let mut registry = PropertyRegistry::new();
        let count = registry.register("Count", PropertyMetadataBuilder::new(0_u32).build());
        let label = registry.register("Label", PropertyMetadataBuilder::new(String::new()).build());
        let source = PropertyEndpoint::new(1, count);
        let target = PropertyEndpoint::new(2, label);

        let mut bindings = BindingSet::new(BINDING);
        bindings
            .bind_map(source, target, (), |value| format!("count: {value}"))
            .unwrap();

        let mut host = TestHost::default();
        host.set_initial(source, 7_u32);
        host.set_channels(target, PAINT.into_set());

        bindings.mark_source_changed(source);
        let report = bindings.drain(&mut host).unwrap();

        assert_eq!(report.evaluated_bindings(), 1);
        assert!(report.affected_channels().contains(PAINT));
        assert_eq!(host.value(target).map(String::as_str), Some("count: 7"));
    }

    #[test]
    fn binding_write_context_is_passed_to_host() {
        #[derive(Copy, Clone, Debug, PartialEq, Eq)]
        enum WriteLayer {
            Local,
            TemplateBinding,
        }

        #[derive(Default)]
        struct LayerHost {
            values: BTreeMap<EndpointKey<u32>, ErasedValue>,
            writes: Vec<(EndpointKey<u32>, WriteLayer)>,
        }

        impl LayerHost {
            fn set_initial<T: Clone + 'static>(
                &mut self,
                endpoint: PropertyEndpoint<u32, T>,
                value: T,
            ) {
                self.values.insert(endpoint.key(), ErasedValue::new(value));
            }
        }

        impl BindingHost<u32, WriteLayer> for LayerHost {
            fn get_erased(&self, endpoint: EndpointKey<u32>) -> Option<ErasedValue> {
                self.values.get(&endpoint).cloned()
            }

            fn set_erased(
                &mut self,
                endpoint: EndpointKey<u32>,
                value: ErasedValue,
                context: WriteLayer,
            ) -> BindingWrite {
                self.values.insert(endpoint, value);
                self.writes.push((endpoint, context));
                BindingWrite::changed(ChannelSet::empty())
            }
        }

        let (_registry, width) = registry();
        let first = PropertyEndpoint::new(1, width);
        let second = PropertyEndpoint::new(2, width);
        let third = PropertyEndpoint::new(3, width);

        let mut bindings = BindingSet::<u32, WriteLayer>::new(BINDING);
        bindings.bind(first, second, WriteLayer::Local).unwrap();
        bindings
            .bind(second, third, WriteLayer::TemplateBinding)
            .unwrap();

        let mut host = LayerHost::default();
        host.set_initial(first, 42_u32);
        bindings.mark_source_changed(first);
        let report = bindings.drain(&mut host).unwrap();

        assert_eq!(report.evaluated_bindings(), 2);
        assert_eq!(
            host.writes,
            alloc::vec![
                (second.key(), WriteLayer::Local),
                (third.key(), WriteLayer::TemplateBinding),
            ]
        );
    }

    #[test]
    fn chained_bindings_propagate_after_target_change() {
        let (_registry, width) = registry();
        let first = PropertyEndpoint::new(1, width);
        let second = PropertyEndpoint::new(2, width);
        let third = PropertyEndpoint::new(3, width);

        let mut bindings = BindingSet::new(BINDING);
        let first_to_second = bindings.bind(first, second, ()).unwrap();
        let second_to_third = bindings.bind(second, third, ()).unwrap();

        let mut host = TestHost::default();
        host.set_initial(first, 10_u32);
        host.set_channels(second, LAYOUT.into_set());
        host.set_channels(third, PAINT.into_set());

        bindings.mark_source_changed(first);
        let report = bindings.drain(&mut host).unwrap();

        assert_eq!(report.evaluated_bindings(), 2);
        assert_eq!(host.value(second), Some(&10));
        assert_eq!(host.value(third), Some(&10));
        assert_eq!(
            host.writes,
            alloc::vec![second.key(), third.key()],
            "binding ids {first_to_second:?} and {second_to_third:?} should drain in dependency order",
        );
    }

    #[test]
    fn simultaneous_dirty_sources_use_dependency_order_without_duplicate_downstream_eval() {
        let (_registry, width) = registry();
        let first = PropertyEndpoint::new(1, width);
        let second = PropertyEndpoint::new(2, width);
        let third = PropertyEndpoint::new(3, width);

        let mut bindings = BindingSet::new(BINDING);
        bindings.bind(first, second, ()).unwrap();
        bindings.bind(second, third, ()).unwrap();

        let mut host = TestHost::default();
        host.set_initial(first, 10_u32);
        host.set_initial(second, 99_u32);

        bindings.mark_source_changed(first);
        bindings.mark_source_changed(second);
        let report = bindings.drain(&mut host).unwrap();

        assert_eq!(report.evaluated_bindings(), 2);
        assert_eq!(host.value(third), Some(&10));
        assert_eq!(host.writes, alloc::vec![second.key(), third.key()]);
    }

    #[test]
    fn independent_dirty_bindings_drain_in_binding_id_order() {
        let (_registry, width) = registry();
        let first_source = PropertyEndpoint::new(1, width);
        let first_target = PropertyEndpoint::new(2, width);
        let second_source = PropertyEndpoint::new(3, width);
        let second_target = PropertyEndpoint::new(4, width);

        let mut bindings = BindingSet::new(BINDING);
        bindings.bind(first_source, first_target, ()).unwrap();
        bindings.bind(second_source, second_target, ()).unwrap();

        let mut host = TestHost::default();
        host.set_initial(first_source, 10_u32);
        host.set_initial(second_source, 20_u32);

        bindings.mark_source_changed(second_source);
        bindings.mark_source_changed(first_source);
        let report = bindings.drain(&mut host).unwrap();

        assert_eq!(report.evaluated_bindings(), 2);
        assert_eq!(
            host.writes,
            alloc::vec![first_target.key(), second_target.key()]
        );
    }

    #[test]
    fn cycle_is_rejected() {
        let (_registry, width) = registry();
        let first = PropertyEndpoint::new(1, width);
        let second = PropertyEndpoint::new(2, width);

        let mut bindings = BindingSet::new(BINDING);
        let first_id = bindings.bind(first, second, ()).unwrap();
        let error = bindings.bind(second, first, ()).unwrap_err();

        assert!(matches!(
            error,
            BindingError::Cycle {
                dependent,
                dependency
            } if dependent == first_id && dependency == BindingId::new(1)
        ));
        assert_eq!(bindings.len(), 1);
    }

    #[test]
    fn duplicate_target_is_rejected_until_existing_binding_is_removed() {
        let (_registry, width) = registry();
        let first = PropertyEndpoint::new(1, width);
        let second = PropertyEndpoint::new(2, width);
        let target = PropertyEndpoint::new(3, width);

        let mut bindings = BindingSet::new(BINDING);
        let first_id = bindings.bind(first, target, ()).unwrap();
        let error = bindings.bind(second, target, ()).unwrap_err();

        assert!(matches!(
            error,
            BindingError::TargetAlreadyBound {
                target: error_target,
                existing
            } if error_target == target.key() && existing == first_id
        ));

        assert!(bindings.unbind(first_id));
        assert!(!bindings.unbind(first_id));
        assert!(bindings.bind(second, target, ()).is_ok());
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings.stats().binding_slots(), 2);
    }

    #[test]
    fn unbind_removes_indexes_and_dependency_edges() {
        let (_registry, width) = registry();
        let first = PropertyEndpoint::new(1, width);
        let second = PropertyEndpoint::new(2, width);
        let third = PropertyEndpoint::new(3, width);

        let mut bindings = BindingSet::new(BINDING);
        let first_to_second = bindings.bind(first, second, ()).unwrap();
        bindings.bind(second, third, ()).unwrap();

        assert_eq!(bindings.stats().dependency_edges(), 1);
        assert!(bindings.unbind(first_to_second));
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings.stats().dependency_edges(), 0);
        assert!(!bindings.mark_source_changed(first));

        let mut host = TestHost::default();
        host.set_initial(second, 11_u32);
        bindings.mark_source_changed(second);
        let report = bindings.drain(&mut host).unwrap();

        assert_eq!(report.evaluated_bindings(), 1);
        assert_eq!(host.value(third), Some(&11));
    }

    #[test]
    fn clear_owner_removes_source_and_target_bindings() {
        let (_registry, width) = registry();
        let first = PropertyEndpoint::new(1, width);
        let second = PropertyEndpoint::new(2, width);
        let third = PropertyEndpoint::new(3, width);

        let mut bindings = BindingSet::new(BINDING);
        bindings.bind(first, second, ()).unwrap();
        bindings.bind(second, third, ()).unwrap();

        assert_eq!(bindings.clear_owner(2), 2);
        assert!(bindings.is_empty());
        assert_eq!(bindings.stats().source_endpoints(), 0);
        assert_eq!(bindings.stats().target_endpoints(), 0);
    }

    #[test]
    fn clear_endpoint_removes_readers_and_writer() {
        let (_registry, width) = registry();
        let first = PropertyEndpoint::new(1, width);
        let second = PropertyEndpoint::new(2, width);
        let third = PropertyEndpoint::new(3, width);

        let mut bindings = BindingSet::new(BINDING);
        bindings.bind(first, second, ()).unwrap();
        bindings.bind(second, third, ()).unwrap();

        assert_eq!(bindings.clear_endpoint(second.key()), 2);
        assert!(bindings.is_empty());
    }

    #[test]
    fn direct_self_binding_is_rejected() {
        let (_registry, width) = registry();
        let endpoint = PropertyEndpoint::new(1, width);

        let mut bindings = BindingSet::new(BINDING);
        let error = bindings.bind(endpoint, endpoint, ()).unwrap_err();

        assert!(matches!(error, BindingError::SelfBinding { .. }));
        assert!(bindings.is_empty());
    }

    #[test]
    fn source_type_mismatch_errors() {
        let (_registry, width) = registry();
        let source = PropertyEndpoint::new(1, width);
        let target = PropertyEndpoint::new(2, width);

        let mut bindings = BindingSet::new(BINDING);
        bindings.bind(source, target, ()).unwrap();

        let mut host = TestHost::default();
        host.values
            .insert(source.key(), ErasedValue::new(String::from("wrong")));

        bindings.mark_source_changed(source);
        let error = bindings.drain(&mut host).unwrap_err();

        assert!(matches!(
            error.error(),
            BindingError::SourceTypeMismatch {
                binding,
                endpoint,
                ..
            } if *binding == BindingId::new(0) && *endpoint == source.key()
        ));
    }

    #[test]
    fn missing_source_is_skipped_and_resumes_when_source_arrives() {
        // Binding 0: first → second. Source has no value yet.
        // Binding 1: second → third. Source already populated.
        // First drain: 0 skips (counted as skipped_missing_source),
        //              1 evaluates. No error. Binding 0 stays clean.
        // After providing first's value + re-marking, binding 0 fires.
        let (_registry, width) = registry();
        let first = PropertyEndpoint::new(1, width);
        let second = PropertyEndpoint::new(2, width);
        let third = PropertyEndpoint::new(3, width);

        let mut bindings = BindingSet::new(BINDING);
        bindings.bind(first, second, ()).unwrap();
        bindings.bind(second, third, ()).unwrap();

        let mut host = TestHost::default();
        host.set_initial(second, 10_u32);

        bindings.mark_source_changed(first);
        bindings.mark_source_changed(second);
        let report = bindings.drain(&mut host).unwrap();

        assert_eq!(report.evaluated_bindings(), 1);
        assert_eq!(report.changed_bindings(), 1);
        assert_eq!(report.skipped_missing_source(), 1);
        assert_eq!(host.value(third), Some(&10));
        assert!(!bindings.has_dirty_bindings());

        host.set_initial(first, 20_u32);
        bindings.mark_source_changed(first);
        let retry = bindings.drain(&mut host).unwrap();

        assert_eq!(retry.evaluated_bindings(), 2);
        assert_eq!(retry.changed_bindings(), 2);
        assert_eq!(retry.skipped_missing_source(), 0);
        assert_eq!(host.value(second), Some(&20));
        assert_eq!(host.value(third), Some(&20));
    }

    #[test]
    fn missing_source_does_not_block_sibling_writes() {
        // Sibling bindings: one with a source value, one without.
        // The unsourced binding is skipped; the other still writes.
        let (_registry, width) = registry();
        let first_source = PropertyEndpoint::new(1, width);
        let first_target = PropertyEndpoint::new(2, width);
        let missing_source = PropertyEndpoint::new(3, width);
        let missing_target = PropertyEndpoint::new(4, width);

        let mut bindings = BindingSet::new(BINDING);
        bindings.bind(first_source, first_target, ()).unwrap();
        bindings.bind(missing_source, missing_target, ()).unwrap();

        let mut host = TestHost::default();
        host.set_initial(first_source, 10_u32);
        host.set_channels(first_target, LAYOUT.into_set());

        bindings.mark_source_changed(first_source);
        bindings.mark_source_changed(missing_source);
        let report = bindings.drain(&mut host).unwrap();

        assert_eq!(report.evaluated_bindings(), 1);
        assert_eq!(report.changed_bindings(), 1);
        assert_eq!(report.skipped_missing_source(), 1);
        assert!(report.affected_channels().contains(LAYOUT));
        assert_eq!(host.value(first_target), Some(&10));
        assert!(!bindings.has_dirty_bindings());

        host.set_initial(missing_source, 20_u32);
        bindings.mark_source_changed(missing_source);
        let retry = bindings.drain(&mut host).unwrap();

        assert_eq!(retry.evaluated_bindings(), 1);
        assert_eq!(host.value(missing_target), Some(&20));
    }
}
