use std::{
    any::TypeId,
    borrow::Cow,
    collections::{btree_map::Entry, BTreeMap},
};

use crate::storage::{AnyStorage, Storage};

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct ComponentId(isize);

impl ComponentId {
    #[inline]
    pub fn is_sparse(self) -> bool {
        self.0 < 0
    }

    #[inline]
    pub fn offset(self) -> usize {
        // storage type is encoded inside the id: negative is sparse, positive is dense
        if self.is_sparse() {
            !(self.0 as usize)
        } else {
            self.0 as usize
        }
    }
}

pub struct Component {
    id: ComponentId,
    name: Cow<'static, str>,
    type_id: TypeId,
    pub(crate) new_storage: fn() -> Box<dyn AnyStorage>,
}

impl Component {
    #[inline]
    pub fn id(&self) -> ComponentId {
        self.id
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }
}

pub struct Components {
    pub(crate) components: Vec<Component>,
    by_type_id: BTreeMap<TypeId, ComponentId>,
}

impl Components {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            components: Vec::new(),
            by_type_id: BTreeMap::new(),
        }
    }

    #[inline]
    pub fn get_id<T>(&self) -> Option<ComponentId>
    where
        T: 'static,
    {
        let type_id = std::any::TypeId::of::<T>();
        self.by_type_id.get(&type_id).copied()
    }

    #[inline]
    pub fn get_or_insert_id<T>(&mut self) -> ComponentId
    where
        T: 'static,
    {
        match self.insert::<T>() {
            Ok(id) | Err(id) => id,
        }
    }

    pub fn insert<T>(&mut self) -> Result<ComponentId, ComponentId>
    where
        T: 'static,
    {
        let type_id = std::any::TypeId::of::<T>();
        let components = &mut self.components;
        match self.by_type_id.entry(type_id) {
            Entry::Vacant(entry) => {
                let index = components.len();
                let id = ComponentId(index as isize); // keep positive => dense
                components.push(Component {
                    id,
                    name: Cow::Borrowed(std::any::type_name::<T>()),
                    type_id,
                    new_storage: || Box::new(Storage::<T>::Dense(Default::default())),
                });
                entry.insert(id);
                Ok(id)
            }
            Entry::Occupied(entry) => Err(*entry.get()),
        }
    }

    pub fn insert_sparse<T>(&mut self) -> Result<ComponentId, ComponentId>
    where
        T: 'static,
    {
        let type_id = std::any::TypeId::of::<T>();
        let components = &mut self.components;
        match self.by_type_id.entry(type_id) {
            Entry::Vacant(entry) => {
                let index = components.len();
                let id = ComponentId(!index as isize); // make inverse (negative) => sparse
                components.push(Component {
                    id,
                    name: Cow::Borrowed(std::any::type_name::<T>()),
                    type_id,
                    new_storage: || Box::new(Storage::<T>::Sparse(Default::default())),
                });
                entry.insert(id);
                Ok(id)
            }
            Entry::Occupied(entry) => Err(*entry.get()),
        }
    }
}

/// Bit-Set like structure
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentSet(Vec<u64>);

impl ComponentSet {
    #[inline]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    pub fn clear(&mut self) {
        self.0.clear()
    }

    #[inline]
    fn split(id: ComponentId) -> (usize, u64) {
        let offset = id.offset();
        let index = offset / 64;
        let bits = 1u64 << (offset % 64);
        (index, bits)
    }

    #[inline]
    pub fn contains(&self, id: ComponentId) -> bool {
        let (index, bits) = Self::split(id);
        if let Some(value) = self.0.get(index) {
            *value & bits != 0
        } else {
            false
        }
    }

    pub fn insert(&mut self, id: ComponentId) {
        let (index, bits) = Self::split(id);
        if index >= self.0.len() {
            self.0.resize(index + 1, 0);
        }
        // SAFETY: vec was extended to contain index
        let value = unsafe { self.0.get_unchecked_mut(index) };
        *value |= bits;
    }

    pub fn remove(&mut self, id: ComponentId) {
        let (index, bits) = Self::split(id);
        if let Some(value) = self.0.get_mut(index) {
            *value &= !bits;
        }
        // shrink (for Eq)
        if index + 1 == self.0.len() {
            while let Some(0) = self.0.last() {
                self.0.pop();
            }
        }
    }

    fn ones(start: usize, mut value: u64) -> impl Iterator<Item = usize> {
        let mut i = start;
        std::iter::from_fn(move || {
            while value != 0 {
                if value & 1 == 1 {
                    let result = i;
                    i += 1;
                    value >>= 1;
                    return Some(result);
                }
                i += 1;
                value >>= 1;
            }
            None
        })
    }

    pub fn offsets(&self) -> impl Iterator<Item = usize> + '_ {
        self.0
            .iter()
            .copied()
            .enumerate()
            .flat_map(|(i, value)| Self::ones(i * 64, value))
    }

    pub fn into_offsets(self) -> impl Iterator<Item = usize> {
        self.0
            .into_iter()
            .enumerate()
            .flat_map(|(i, value)| Self::ones(i * 64, value))
    }

    pub fn iter<'l>(
        &'l self,
        components: &'l Components,
    ) -> impl Iterator<Item = ComponentId> + 'l {
        self.offsets()
            .map(move |offset| components.components[offset].id)
    }

    pub fn into_iter(self, components: &Components) -> impl Iterator<Item = ComponentId> + '_ {
        self.into_offsets()
            .map(move |offset| components.components[offset].id)
    }
}

impl Default for ComponentSet {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

/// A Map-Like structure based on a sorted array and binary-search
pub struct ComponentMap<T>(Vec<(ComponentId, T)>);

impl<T> ComponentMap<T> {
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    pub fn clear(&mut self) {
        self.0.clear()
    }

    #[inline]
    fn search(&self, id: ComponentId) -> Result<usize, usize> {
        self.0.binary_search_by(|(item_id, _)| item_id.cmp(&id))
    }

    #[inline]
    pub fn contains(&self, id: ComponentId) -> bool {
        self.search(id).is_ok()
    }

    #[inline]
    pub fn get(&self, id: ComponentId) -> Option<&T> {
        if let Ok(index) = self.search(id) {
            // SAFETY: index was found by search
            Some(unsafe { &self.0.get_unchecked(index).1 })
        } else {
            None
        }
    }

    #[inline]
    pub fn get_mut(&mut self, id: ComponentId) -> Option<&mut T> {
        if let Ok(index) = self.search(id) {
            // SAFETY: index was found by search
            Some(unsafe { &mut self.0.get_unchecked_mut(index).1 })
        } else {
            None
        }
    }

    #[inline]
    pub fn remove(&mut self, id: ComponentId) -> Option<T> {
        match self.search(id) {
            Ok(index) => Some(self.0.remove(index).1),
            Err(_) => None,
        }
    }

    #[inline]
    pub fn insert(&mut self, id: ComponentId, value: T) -> &mut T {
        match self.search(id) {
            Ok(index) => {
                // SAFETY: index was found by search
                let item = unsafe { &mut self.0.get_unchecked_mut(index).1 };
                *item = value;
                item
            }
            Err(index) => {
                self.0.insert(index, (id, value));
                // SAFETY: index was inserted
                unsafe { &mut self.0.get_unchecked_mut(index).1 }
            }
        }
    }

    #[inline]
    pub fn get_or_insert_with<F>(&mut self, id: ComponentId, create: F) -> &mut T
    where
        F: FnOnce() -> T,
    {
        match self.search(id) {
            Ok(index) => {
                // SAFETY: index was found by search
                unsafe { &mut self.0.get_unchecked_mut(index).1 }
            }
            Err(index) => {
                self.0.insert(index, (id, create()));
                // SAFETY: index was inserted
                unsafe { &mut self.0.get_unchecked_mut(index).1 }
            }
        }
    }

    #[inline]
    pub fn entries<'l>(&'l self) -> impl Iterator<Item = (ComponentId, &'l T)> + '_ {
        self.0.iter().map(|(id, value)| (*id, value))
    }

    #[inline]
    pub fn entries_mut<'l>(&'l mut self) -> impl Iterator<Item = (ComponentId, &'l mut T)> + '_ {
        self.0.iter_mut().map(|(id, value)| (*id, value))
    }

    #[inline]
    pub fn into_entries(self) -> impl Iterator<Item = (ComponentId, T)> {
        self.0.into_iter().map(|(id, value)| (id, value))
    }

    #[inline]
    pub fn keys(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.0.iter().map(|(id, _)| *id)
    }

    #[inline]
    pub fn key_set(&self) -> ComponentSet {
        let mut set = ComponentSet::new();
        if let Some(((last_id, _), rest)) = self.0.split_last() {
            set.insert(*last_id); // add last id first, for allocating only once
            for (id, _) in rest {
                set.insert(*id);
            }
        }
        set
    }
}

impl<T: Default> ComponentMap<T> {
    #[inline]
    pub fn get_or_insert_default(&mut self, id: ComponentId) -> &mut T {
        self.get_or_insert_with(id, Default::default)
    }
}

impl<T> Default for ComponentMap<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
