// Copyright 2026 the Understory Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Internal generational arena storage.

use alloc::vec::Vec;
use core::convert::TryFrom;

use crate::ids::ArenaId;

#[derive(Clone, Debug)]
struct Slot<T> {
    generation: u32,
    value: Option<T>,
    next_free: Option<u32>,
}

/// Internal arena keyed by generational IDs.
#[derive(Clone, Debug)]
pub(crate) struct Arena<I, T> {
    slots: Vec<Slot<T>>,
    free_head: Option<u32>,
    len: usize,
    _marker: core::marker::PhantomData<I>,
}

impl<I, T> Default for Arena<I, T> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free_head: None,
            len: 0,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<I, T> Arena<I, T>
where
    I: ArenaId,
{
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn insert(&mut self, value: T) -> I {
        match self.free_head {
            Some(index) => {
                let slot = &mut self.slots[index as usize];
                self.free_head = slot.next_free;
                slot.value = Some(value);
                slot.next_free = None;
                self.len += 1;
                I::from_parts(index, slot.generation)
            }
            None => {
                let index =
                    u32::try_from(self.slots.len()).expect("arena slot count exceeds u32::MAX");
                self.slots.push(Slot {
                    generation: 0,
                    value: Some(value),
                    next_free: None,
                });
                self.len += 1;
                I::from_parts(index, 0)
            }
        }
    }

    pub(crate) fn contains(&self, id: I) -> bool {
        self.get(id).is_some()
    }

    pub(crate) fn get(&self, id: I) -> Option<&T> {
        let slot = self.slots.get(id.index() as usize)?;
        (slot.generation == id.generation())
            .then_some(slot.value.as_ref())
            .flatten()
    }

    pub(crate) fn get_mut(&mut self, id: I) -> Option<&mut T> {
        let slot = self.slots.get_mut(id.index() as usize)?;
        (slot.generation == id.generation())
            .then_some(slot.value.as_mut())
            .flatten()
    }

    pub(crate) fn remove(&mut self, id: I) -> Option<T> {
        let index = id.index() as usize;
        let slot = self.slots.get_mut(index)?;
        if slot.generation != id.generation() {
            return None;
        }
        let value = slot.value.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        slot.next_free = self.free_head;
        self.free_head = Some(id.index());
        self.len -= 1;
        Some(value)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (I, &T)> {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            let value = slot.value.as_ref()?;
            let index = u32::try_from(index).expect("arena slot index exceeds u32::MAX");
            Some((I::from_parts(index, slot.generation), value))
        })
    }
}
