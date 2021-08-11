use std::any::{Any, TypeId};

use fnv::FnvHashMap;

use crate::{
    component::{ComponentId, Components},
    Entity,
};

pub trait ArchetypeStorage {
    fn swap_remove(&mut self, archetype_offset: usize);
    fn replace_impl(&mut self, archetype_offset: usize, value: &mut dyn Any) -> bool;
    fn push_impl(&mut self, value: &mut dyn Any) -> Option<usize>;
    fn swap_remove_and_push(
        &mut self,
        remove_offset: usize,
        push_to: &mut dyn ArchetypeStorage,
    ) -> Option<usize>;

    fn typeid(&self) -> TypeId;
}

struct ColumnStorage<T>(Vec<T>);

impl<T> ColumnStorage<T>
where
    T: 'static,
{
    pub(crate) fn new_dyn() -> Box<dyn ArchetypeStorage> {
        Box::new(Self(Vec::new()))
    }
}

impl<T> ArchetypeStorage for ColumnStorage<T>
where
    T: 'static,
{
    fn swap_remove(&mut self, archetype_offset: usize) {
        self.0.swap_remove(archetype_offset);
        if self.0.is_empty() {
            self.0.shrink_to_fit();
        }
    }

    fn replace_impl(&mut self, archetype_offset: usize, value: &mut dyn Any) -> bool {
        if let Some(transfer) = value.downcast_mut::<Option<T>>() {
            if let Some(value) = transfer.take() {
                self.0[archetype_offset] = value;
                return true;
            }
        }
        false
    }
    fn push_impl(&mut self, value: &mut dyn Any) -> Option<usize> {
        let index = self.0.len();
        if let Some(transfer) = value.downcast_mut::<Option<T>>() {
            if let Some(value) = transfer.take() {
                self.0.push(value);
                return Some(index);
            }
        }
        None
    }

    fn swap_remove_and_push(
        &mut self,
        remove_offset: usize,
        push_to: &mut dyn ArchetypeStorage,
    ) -> Option<usize> {
        let mut value = Some(self.0.swap_remove(remove_offset));
        if self.0.is_empty() {
            self.0.shrink_to_fit();
        }
        push_to.push_impl(&mut value)
    }

    fn typeid(&self) -> TypeId {
        TypeId::of::<T>()
    }
}

pub trait WorldStorage {
    fn insert_impl(&mut self, entity: Entity, value: &mut dyn Any) -> bool;
    fn remove(&mut self, entity: Entity) -> bool;
    fn contains(&self, entity: Entity) -> bool;

    fn typeid(&self) -> TypeId;
}

struct SparseStorage<T>(FnvHashMap<Entity, T>);

impl<T> SparseStorage<T>
where
    T: 'static,
{
    pub(crate) fn new_dyn() -> Box<dyn WorldStorage> {
        Box::new(Self(FnvHashMap::default()))
    }
}

impl<T> WorldStorage for SparseStorage<T>
where
    T: 'static,
{
    fn insert_impl(&mut self, entity: Entity, value: &mut dyn Any) -> bool {
        if let Some(transfer) = value.downcast_mut::<Option<T>>() {
            if let Some(value) = transfer.take() {
                self.0.insert(entity, value);
                return true;
            }
        }
        false
    }
    fn remove(&mut self, entity: Entity) -> bool {
        self.0.remove(&entity).is_some()
    }
    fn contains(&self, entity: Entity) -> bool {
        self.0.contains_key(&entity)
    }

    fn typeid(&self) -> TypeId {
        TypeId::of::<T>()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NewStorage {
    None,
    World(fn() -> Box<dyn WorldStorage>),
    Archetype(fn() -> Box<dyn ArchetypeStorage>),
}

impl Default for NewStorage {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl NewStorage{
    #[inline]
    pub fn world<T>() -> Self
        where T: 'static
    {
        Self::World(SparseStorage::<T>::new_dyn)
    }

    #[inline]
    pub fn archetype<T>() -> Self
        where T: 'static
    {
        Self::Archetype(ColumnStorage::<T>::new_dyn)
    }
}

impl Components {

    pub(crate) fn new_world_storage(&self, id: ComponentId) -> Box<dyn WorldStorage> {
        if let NewStorage::World(new_fn) = self.components[id.offset()].new_storage {
            new_fn()
        } else {
            panic!("unexpected storage constructor type");
        }
    }

    pub(crate) fn new_archetype_storage(&self, id: ComponentId) -> Box<dyn ArchetypeStorage> {
        if let NewStorage::Archetype(new_fn) = self.components[id.offset()].new_storage {
            new_fn()
        } else {
            panic!("unexpected storage constructor type");
        }
    }
}

