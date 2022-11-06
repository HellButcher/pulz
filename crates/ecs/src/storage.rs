use std::any::{Any, TypeId};

type HashMap<K, V> = hashbrown::HashMap<K, V, fnv::FnvBuildHasher>;

use crate::{archetype::ArchetypeId, resource::FromResources, Entity};

pub trait Storage: Send + Sync + Any + FromResources {
    const SPARSE: bool;

    type Component;

    #[inline]
    fn component_type_id() -> TypeId {
        TypeId::of::<Self::Component>()
    }

    fn contains(&self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool;
    fn swap_remove(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        index: usize,
    ) -> Option<Self::Component>;
    fn insert(&mut self, entity: Entity, archetype: ArchetypeId, value: Self::Component) -> usize;
    fn replace(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        index: usize,
        value: Self::Component,
    ) -> Option<Self::Component>;

    fn get(&self, entity: Entity, archetype: ArchetypeId, index: usize)
        -> Option<&Self::Component>;

    fn get_mut(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        index: usize,
    ) -> Option<&mut Self::Component>;
}

pub trait AnyStorage: Send + Sync + Any {
    fn component_type_id(&self) -> TypeId;
    fn contains(&self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool;
    fn swap_remove(&mut self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool;
    fn insert(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        value: &mut dyn Any,
    ) -> Option<usize>;
    fn replace(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        index: usize,
        value: &mut dyn Any,
    ) -> bool;
    fn swap_remove_and_insert_to(
        &mut self,
        entity: Entity,
        remove_from_archetype: ArchetypeId,
        remove_from_index: usize,
        insert_to_archetype: ArchetypeId,
    ) -> Option<usize>;
}

pub struct DenseStorage<T>(Vec<Vec<T>>);
pub struct HashMapStorage<T>(HashMap<Entity, T>);

impl<T> Default for DenseStorage<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T> Default for HashMapStorage<T> {
    fn default() -> Self {
        Self(HashMap::default())
    }
}

fn vec_make_available<T: Default>(vec: &mut Vec<T>, index: usize) -> &mut T {
    if vec.len() <= index {
        vec.resize_with(index + 1, Default::default);
    }
    // SAFETY: was resized if length was to short
    unsafe { vec.get_unchecked_mut(index) }
}

impl<T> Storage for DenseStorage<T>
where
    T: Send + Sync + 'static,
{
    const SPARSE: bool = false;
    type Component = T;

    #[inline]
    fn contains(&self, _entity: Entity, archetype: ArchetypeId, index: usize) -> bool {
        self.0
            .get(archetype.index())
            .map_or(false, |col| index < col.len())
    }
    #[inline]
    fn swap_remove(&mut self, _entity: Entity, archetype: ArchetypeId, index: usize) -> Option<T> {
        if let Some(col) = self.0.get_mut(archetype.index()) {
            if index < col.len() {
                return Some(col.swap_remove(index));
            }
        }
        None
    }
    #[inline]
    fn insert(&mut self, _entity: Entity, archetype: ArchetypeId, value: T) -> usize {
        let col = vec_make_available(&mut self.0, archetype.index());
        let new_index = col.len();
        col.push(value);
        new_index
    }
    #[inline]
    fn replace(
        &mut self,
        _entity: Entity,
        archetype: ArchetypeId,
        index: usize,
        value: T,
    ) -> Option<T> {
        let col = vec_make_available(&mut self.0, archetype.index());
        col.get_mut(index)
            .map(|entry| std::mem::replace(entry, value))
    }

    #[inline]
    fn get(
        &self,
        _entity: Entity,
        archetype: ArchetypeId,
        index: usize,
    ) -> Option<&Self::Component> {
        self.0.get(archetype.index())?.get(index)
    }

    #[inline]
    fn get_mut(
        &mut self,
        _entity: Entity,
        archetype: ArchetypeId,
        index: usize,
    ) -> Option<&mut Self::Component> {
        self.0.get_mut(archetype.index())?.get_mut(index)
    }
}

impl<T> Storage for HashMapStorage<T>
where
    T: Send + Sync + 'static,
{
    const SPARSE: bool = true;
    type Component = T;

    #[inline]
    fn contains(&self, entity: Entity, _archetype: ArchetypeId, _index: usize) -> bool {
        self.0.contains_key(&entity)
    }
    #[inline]
    fn swap_remove(&mut self, entity: Entity, _archetype: ArchetypeId, _index: usize) -> Option<T> {
        self.0.remove(&entity)
    }
    #[inline]
    fn insert(&mut self, entity: Entity, _archetype: ArchetypeId, value: T) -> usize {
        let len = self.0.len();
        self.0.insert(entity, value);
        len
    }
    #[inline]
    fn replace(
        &mut self,
        entity: Entity,
        _archetype: ArchetypeId,
        _index: usize,
        value: T,
    ) -> Option<T> {
        self.0.insert(entity, value)
    }

    #[inline]
    fn get(
        &self,
        entity: Entity,
        _archetype: ArchetypeId,
        _index: usize,
    ) -> Option<&Self::Component> {
        self.0.get(&entity)
    }

    #[inline]
    fn get_mut(
        &mut self,
        entity: Entity,
        _archetype: ArchetypeId,
        _index: usize,
    ) -> Option<&mut Self::Component> {
        self.0.get_mut(&entity)
    }
}

fn take_option_t<T: 'static>(value: &mut dyn Any) -> Option<T> {
    value.downcast_mut::<Option<T>>()?.take()
}

impl<S> AnyStorage for S
where
    S: Storage,
{
    #[inline]
    fn component_type_id(&self) -> TypeId {
        S::component_type_id()
    }

    #[inline]
    fn contains(&self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool {
        S::contains(self, entity, archetype, index)
    }

    #[inline]
    fn swap_remove(&mut self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool {
        S::swap_remove(self, entity, archetype, index).is_some()
    }

    #[inline]
    fn insert(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        value: &mut dyn Any,
    ) -> Option<usize> {
        let value_t = take_option_t::<S::Component>(value)?;
        Some(S::insert(self, entity, archetype, value_t))
    }

    #[inline]
    fn replace(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        index: usize,
        value: &mut dyn Any,
    ) -> bool {
        let Some(value_t) = take_option_t::<S::Component>(value) else {
            return false;
        };
        S::replace(self, entity, archetype, index, value_t).is_some()
    }

    fn swap_remove_and_insert_to(
        &mut self,
        entity: Entity,
        remove_from_archetype: ArchetypeId,
        remove_from_index: usize,
        insert_to_archetype: ArchetypeId,
    ) -> Option<usize> {
        if remove_from_archetype == insert_to_archetype {
            return None;
        }
        S::swap_remove(self, entity, remove_from_archetype, remove_from_index)
            .map(|value| S::insert(self, entity, insert_to_archetype, value))
    }
}
