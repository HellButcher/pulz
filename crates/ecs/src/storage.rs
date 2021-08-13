use std::{
    any::{Any, TypeId},
    marker::PhantomData,
};

use fnv::FnvHashMap;

use crate::{
    component::{ComponentId, ComponentMap, ComponentSet},
    Entity,
};

pub trait AnyComponentMaper<T>: 'static {
    type Target: Any;
    #[inline]
    fn is_target(storage: &dyn Storage) -> bool {
        TypeId::of::<Self::Target>() == storage.type_id()
    }
    #[inline]
    fn as_ref(storage: &dyn Storage) -> Option<&Self::Target> {
        if Self::is_target(storage) {
            // SAFETY: just checked whether we are pointing to the correct type
            unsafe {
                let storage: *const dyn Storage = storage;
                Some(&*(storage as *const Self::Target))
            }
        } else {
            None
        }
    }
    #[inline]
    fn as_mut(storage: &mut dyn Storage) -> Option<&mut Self::Target> {
        if Self::is_target(storage) {
            // SAFETY: just checked whether we are pointing to the correct type
            unsafe {
                let storage: *mut dyn Storage = storage;
                Some(&mut *(storage as *mut Self::Target))
            }
        } else {
            None
        }
    }
}

pub struct ComponentStorageMap<M> {
    map: ComponentMap<Box<dyn Storage>>,
    phantom: PhantomData<M>,
}

impl<M> ComponentStorageMap<M> {
    #[inline]
    pub fn new() -> Self {
        Self {
            map: ComponentMap::new(),
            phantom: PhantomData,
        }
    }

    #[inline]
    pub fn contains_id(&self, component_id: ComponentId) -> bool {
        self.map.contains(component_id)
    }

    #[inline]
    pub fn component_ids(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.map.keys()
    }

    #[inline]
    pub fn component_id_set(&self) -> ComponentSet {
        self.map.key_set()
    }

    #[inline]
    pub fn entries(&self) -> impl Iterator<Item = (ComponentId, &dyn Storage)> + '_ {
        self.map.entries().map(|(id, boxed)| (id, boxed.as_ref()))
    }

    #[inline]
    pub fn entries_mut(&mut self) -> impl Iterator<Item = (ComponentId, &mut dyn Storage)> + '_ {
        self.map
            .entries_mut()
            .map(|(id, boxed)| (id, boxed.as_mut()))
    }

    #[inline]
    pub fn get(&self, component_id: ComponentId) -> Option<&dyn Storage> {
        if let Some(boxed) = self.map.get(component_id) {
            Some(boxed.as_ref())
        } else {
            None
        }
    }

    #[inline]
    pub fn get_mut(&mut self, component_id: ComponentId) -> Option<&mut dyn Storage> {
        if let Some(boxed) = self.map.get_mut(component_id) {
            Some(boxed.as_mut())
        } else {
            None
        }
    }

    #[inline]
    pub fn typed_get<T>(&self, component_id: ComponentId) -> Option<&M::Target>
    where
        T: 'static,
        M: AnyComponentMaper<T>,
    {
        if let Some(boxed) = self.map.get(component_id) {
            M::as_ref(boxed.as_ref())
        } else {
            None
        }
    }

    #[inline]
    pub fn typed_get_mut<T>(&mut self, component_id: ComponentId) -> Option<&mut M::Target>
    where
        T: 'static,
        M: AnyComponentMaper<T>,
    {
        if let Some(boxed) = self.map.get_mut(component_id) {
            M::as_mut(boxed.as_mut())
        } else {
            None
        }
    }

    #[inline]
    pub fn get_or_insert_with<F>(
        &mut self,
        component_id: ComponentId,
        create: F,
    ) -> &mut dyn Storage
    where
        F: FnOnce() -> Box<dyn Storage>,
    {
        self.map.get_or_insert_with(component_id, create).as_mut()
    }
}

pub struct ColumnStorageMapper;

impl<T> AnyComponentMaper<T> for ColumnStorageMapper
where
    T: 'static,
{
    type Target = ColumnStorage<T>;
}

pub struct SparseStorageMapper;

impl<T> AnyComponentMaper<T> for SparseStorageMapper
where
    T: 'static,
{
    type Target = SparseStorage<T>;
}

pub type ColumnStorage<T> = Vec<T>;
pub type SparseStorage<T> = FnvHashMap<Entity, T>;

pub trait Storage: Any {
    fn component_type_id(&self) -> TypeId;
    fn len(&self) -> usize;
    fn contains(&self, entity: Entity, index: usize) -> bool;
    fn swap_remove(&mut self, entity: Entity, index: usize) -> bool;
    fn insert(&mut self, entity: Entity, value: &mut dyn Any) -> Option<usize>;
    fn replace(&mut self, entity: Entity, index: usize, value: &mut dyn Any) -> bool;
    fn swap_remove_and_insert_to(
        &mut self,
        entity: Entity,
        remove_index: usize,
        insert_to: &mut dyn Storage,
    ) -> Option<usize>;
}

impl<T> Storage for ColumnStorage<T>
where
    T: 'static,
{
    #[inline]
    fn component_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    #[inline]
    fn len(&self) -> usize {
        self.len()
    }
    #[inline]
    fn contains(&self, _entity: Entity, index: usize) -> bool {
        index < self.len()
    }

    #[inline]
    fn swap_remove(&mut self, _entity: Entity, index: usize) -> bool {
        if index < self.len() {
            self.swap_remove(index);
            true
        } else {
            false
        }
    }

    #[inline]
    fn insert(&mut self, _entity: Entity, value: &mut dyn Any) -> Option<usize> {
        if let Some(transfer) = value.downcast_mut::<Option<T>>() {
            if let Some(value) = transfer.take() {
                let index = self.len();
                self.push(value);
                return Some(index);
            }
        }
        None
    }

    #[inline]
    fn replace(&mut self, _entity: Entity, index: usize, value: &mut dyn Any) -> bool {
        if index >= self.len() {
            return false;
        }
        if let Some(transfer) = value.downcast_mut::<Option<T>>() {
            if let Some(value) = transfer.take() {
                self[index] = value;
                return true;
            }
        }
        false
    }

    fn swap_remove_and_insert_to(
        &mut self,
        entity: Entity,
        remove_index: usize,
        insert_to: &mut dyn Storage,
    ) -> Option<usize> {
        if remove_index < self.len() {
            let mut value = Some(self.swap_remove(remove_index));
            insert_to.insert(entity, &mut value)
        } else {
            None
        }
    }
}

impl<T> Storage for SparseStorage<T>
where
    T: 'static,
{
    #[inline]
    fn component_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    #[inline]
    fn len(&self) -> usize {
        self.len()
    }
    #[inline]
    fn contains(&self, entity: Entity, _index: usize) -> bool {
        self.contains_key(&entity)
    }
    #[inline]
    fn swap_remove(&mut self, entity: Entity, _index: usize) -> bool {
        self.remove(&entity).is_some()
    }

    #[inline]
    fn insert(&mut self, entity: Entity, value: &mut dyn Any) -> Option<usize> {
        if let Some(transfer) = value.downcast_mut::<Option<T>>() {
            if let Some(value) = transfer.take() {
                self.insert(entity, value);
                return Some(0);
            }
        }
        None
    }

    #[inline]
    fn replace(&mut self, entity: Entity, _index: usize, value: &mut dyn Any) -> bool {
        <Self as Storage>::insert(self, entity, value).is_some()
    }

    fn swap_remove_and_insert_to(
        &mut self,
        entity: Entity,
        _remove_offset: usize,
        insert_to: &mut dyn Storage,
    ) -> Option<usize> {
        if let Some(value) = self.remove(&entity) {
            let mut value = Some(value);
            insert_to.insert(entity, &mut value)
        } else {
            None
        }
    }
}
