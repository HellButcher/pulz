use std::{
    any::TypeId,
    borrow::Cow,
    collections::{btree_map::Entry, BTreeMap},
    hash::Hash,
    marker::PhantomData,
};

use crate::{
    resource::{Res, ResMut, ResourceId, Resources},
    storage::{AnyStorage, Storage},
};

pub type Ref<'w, T> = Res<'w, T>;
pub type RefMut<'w, T> = ResMut<'w, T>;

#[repr(transparent)]
pub struct ComponentId<T = crate::Void>(isize, PhantomData<fn() -> T>);

impl<T> std::fmt::Debug for ComponentId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ComponentId").field(&self.0).finish()
    }
}
impl<T> Copy for ComponentId<T> {}
impl<T> Clone for ComponentId<T> {
    fn clone(&self) -> Self {
        Self(self.0, PhantomData)
    }
}
impl<T> Eq for ComponentId<T> {}
impl<T> Ord for ComponentId<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}
impl<T> PartialEq<Self> for ComponentId<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T> PartialOrd<Self> for ComponentId<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
impl<T> Hash for ComponentId<T> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl<T> ComponentId<T> {
    #[inline(always)]
    fn cast<X>(self) -> ComponentId<X> {
        ComponentId(self.0, PhantomData)
    }

    #[inline]
    pub fn untyped(self) -> ComponentId {
        self.cast()
    }

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

impl ComponentId {
    #[inline]
    pub fn typed<T>(self) -> ComponentId<T>
    where
        T: Send + Sync + 'static,
    {
        self.cast()
    }
}

pub(crate) struct ComponentInfo {
    id: ComponentId,
    name: Cow<'static, str>,
    type_id: TypeId,
    pub(crate) storage_id: ResourceId,
    pub(crate) any_getter: fn(&Resources, ResourceId) -> Option<Res<'_, dyn AnyStorage>>,
    pub(crate) any_getter_mut: fn(&mut Resources, ResourceId) -> Option<&mut dyn AnyStorage>,
}

impl ComponentInfo {
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
    pub(crate) components: Vec<ComponentInfo>,
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
    pub fn get_id<T>(&self) -> Option<ComponentId<T>>
    where
        T: Send + Sync + 'static,
    {
        let type_id = std::any::TypeId::of::<T>();
        self.by_type_id
            .get(&type_id)
            .copied()
            .map(ComponentId::typed)
    }

    pub(crate) fn get<T>(&self, component_id: ComponentId<T>) -> Option<&ComponentInfo>
    where
        T: 'static,
    {
        self.components.get(component_id.offset())
    }

    pub(crate) fn insert<T>(
        &mut self,
        storage_id: ResourceId<Storage<T>>,
        sparse: bool,
    ) -> Result<ComponentId<T>, ComponentId<T>>
    where
        T: Send + Sync + 'static,
    {
        let type_id = std::any::TypeId::of::<T>();
        let components = &mut self.components;
        match self.by_type_id.entry(type_id) {
            Entry::Vacant(entry) => {
                let index = components.len();
                let id = if sparse {
                    ComponentId(!index as isize, PhantomData) // make inverse (negative) => sparse
                } else {
                    ComponentId(index as isize, PhantomData) // keep positive => dense
                };
                components.push(ComponentInfo {
                    id,
                    name: Cow::Borrowed(std::any::type_name::<T>()),
                    type_id,
                    storage_id: storage_id.untyped(),
                    any_getter: Storage::<T>::from_res,
                    any_getter_mut: Storage::<T>::from_res_mut,
                });
                entry.insert(id);
                Ok(id.typed())
            }
            Entry::Occupied(entry) => Err((*entry.get()).typed()),
        }
    }

    pub fn to_set(&self) -> ComponentSet {
        let words = self.components.len() / 64;
        let mut words_rest = self.components.len() % 64;
        let mut result = Vec::with_capacity(words + 1);
        result.resize(words, u64::MAX);
        if words_rest != 0 {
            let mut last_value = 0;
            while words_rest > 0 {
                words_rest -= 1;
                last_value <<= 1;
                last_value |= 1;
            }
            result.push(last_value)
        }
        ComponentSet(result)
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
    fn split<X>(id: ComponentId<X>) -> (usize, u64) {
        let offset = id.offset();
        let index = offset / 64;
        let bits = 1u64 << (offset % 64);
        (index, bits)
    }

    #[inline]
    pub fn contains<X>(&self, id: ComponentId<X>) -> bool {
        let (index, bits) = Self::split(id);
        if let Some(value) = self.0.get(index) {
            *value & bits != 0
        } else {
            false
        }
    }

    pub fn insert<X>(&mut self, id: ComponentId<X>) {
        let (index, bits) = Self::split(id);
        if index >= self.0.len() {
            self.0.resize(index + 1, 0);
        }
        // SAFETY: vec was extended to contain index
        let value = unsafe { self.0.get_unchecked_mut(index) };
        *value |= bits;
    }

    pub fn remove<X>(&mut self, id: ComponentId<X>) {
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
    fn search<X>(&self, id: ComponentId<X>) -> Result<usize, usize> {
        self.0.binary_search_by(|(item_id, _)| item_id.0.cmp(&id.0))
    }

    #[inline]
    pub fn contains<X>(&self, id: ComponentId<X>) -> bool {
        self.search(id).is_ok()
    }

    #[inline]
    pub fn get<X>(&self, id: ComponentId<X>) -> Option<&T> {
        if let Ok(index) = self.search(id) {
            // SAFETY: index was found by search
            Some(unsafe { &self.0.get_unchecked(index).1 })
        } else {
            None
        }
    }

    #[inline]
    pub fn get_mut<X>(&mut self, id: ComponentId<X>) -> Option<&mut T> {
        if let Ok(index) = self.search(id) {
            // SAFETY: index was found by search
            Some(unsafe { &mut self.0.get_unchecked_mut(index).1 })
        } else {
            None
        }
    }

    #[inline]
    pub fn remove<X>(&mut self, id: ComponentId<X>) -> Option<T> {
        match self.search(id) {
            Ok(index) => Some(self.0.remove(index).1),
            Err(_) => None,
        }
    }

    #[inline]
    pub fn insert<X>(&mut self, id: ComponentId<X>, value: T) -> &mut T {
        match self.search(id) {
            Ok(index) => {
                // SAFETY: index was found by search
                let item = unsafe { &mut self.0.get_unchecked_mut(index).1 };
                *item = value;
                item
            }
            Err(index) => {
                self.0.insert(index, (id.untyped(), value));
                // SAFETY: index was inserted
                unsafe { &mut self.0.get_unchecked_mut(index).1 }
            }
        }
    }

    #[inline]
    pub fn get_or_insert_with<X, F>(&mut self, id: ComponentId<X>, create: F) -> &mut T
    where
        F: FnOnce() -> T,
    {
        match self.search(id) {
            Ok(index) => {
                // SAFETY: index was found by search
                unsafe { &mut self.0.get_unchecked_mut(index).1 }
            }
            Err(index) => {
                self.0.insert(index, (id.untyped(), create()));
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
