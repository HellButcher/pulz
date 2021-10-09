use std::{
    any::{Any, TypeId},
    cmp::Ordering,
};

use fnv::FnvHashMap;

use crate::{
    archetype::ArchetypeId,
    resource::{Res, ResourceId, Resources},
    Entity,
};

pub enum Storage<T> {
    Dense(Vec<Vec<T>>),
    Sparse(FnvHashMap<Entity, T>),
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

fn vec_make_available<T: Default>(vec: &mut Vec<T>, index: usize) -> &mut T {
    if vec.len() <= index {
        vec.resize_with(index + 1, Default::default);
    }
    // SAFETY: was resized if length was to short
    unsafe { vec.get_unchecked_mut(index) }
}

impl<T> Storage<T>
where
    T: Send + Sync + 'static,
{
    #[inline]
    pub fn new_dense() -> Self {
        Self::Dense(Vec::new())
    }
    #[inline]
    pub fn new_sparse() -> Self {
        Self::Sparse(FnvHashMap::default())
    }
    fn take_t(value: &mut dyn Any) -> Option<T> {
        value.downcast_mut::<Option<T>>()?.take()
    }

    pub(crate) fn from_res(res: &Resources, id: ResourceId) -> Option<Res<'_, dyn AnyStorage>> {
        Some(Res::map(res.borrow_res_id::<Self>(id.typed())?, |s| {
            let d: &dyn AnyStorage = s;
            d
        }))
    }

    pub(crate) fn from_res_mut(res: &mut Resources, id: ResourceId) -> Option<&mut dyn AnyStorage> {
        Some(res.get_mut_id::<Self>(id.typed())?)
    }

    #[inline]
    pub fn get(&self, entity: Entity, archetype: ArchetypeId, index: usize) -> Option<&T> {
        match self {
            Storage::Dense(columns) => columns.get(archetype.index())?.get(index),
            Storage::Sparse(map) => map.get(&entity),
        }
    }

    #[inline]
    pub fn get_mut(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        index: usize,
    ) -> Option<&mut T> {
        match self {
            Storage::Dense(columns) => columns.get_mut(archetype.index())?.get_mut(index),
            Storage::Sparse(map) => map.get_mut(&entity),
        }
    }
}

impl<T> AnyStorage for Storage<T>
where
    T: Send + Sync + 'static,
{
    #[inline]
    fn component_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    #[inline]
    fn contains(&self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool {
        match self {
            Storage::Dense(columns) => columns
                .get(archetype.index())
                .map_or(false, |col| index < col.len()),
            Storage::Sparse(map) => map.contains_key(&entity),
        }
    }

    #[inline]
    fn swap_remove(&mut self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool {
        match self {
            Storage::Dense(columns) => {
                if let Some(col) = columns.get_mut(archetype.index()) {
                    if index < col.len() {
                        col.swap_remove(index);
                        return true;
                    }
                }
            }
            Storage::Sparse(map) => {
                if map.remove(&entity).is_some() {
                    return true;
                }
            }
        }
        false
    }

    #[inline]
    fn insert(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        value: &mut dyn Any,
    ) -> Option<usize> {
        let value_t = Self::take_t(value)?;
        match self {
            Storage::Dense(columns) => {
                let col = vec_make_available(columns, archetype.index());
                let new_index = col.len();
                col.push(value_t);
                Some(new_index)
            }
            Storage::Sparse(map) => {
                map.insert(entity, value_t);
                Some(0)
            }
        }
    }

    #[inline]
    fn replace(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        index: usize,
        value: &mut dyn Any,
    ) -> bool {
        let value_t = if let Some(value) = Self::take_t(value) {
            value
        } else {
            return false;
        };
        match self {
            Storage::Dense(columns) => {
                let col = vec_make_available(columns, archetype.index());
                if let Some(entry) = col.get_mut(index) {
                    *entry = value_t;
                    true
                } else {
                    false
                }
            }
            Storage::Sparse(map) => {
                map.insert(entity, value_t);
                true
            }
        }
    }

    fn swap_remove_and_insert_to(
        &mut self,
        _entity: Entity,
        remove_from_archetype: ArchetypeId,
        remove_from_index: usize,
        insert_to_archetype: ArchetypeId,
    ) -> Option<usize> {
        if remove_from_archetype == insert_to_archetype {
            return None;
        }
        if let Storage::Dense(columns) = self {
            let from_len = columns.get(remove_from_archetype.index())?.len();
            if remove_from_index < from_len {
                vec_make_available(columns, insert_to_archetype.index());
                if let Some((from_col, to_col)) = slice_get_mut2(
                    columns,
                    remove_from_archetype.index(),
                    insert_to_archetype.index(),
                ) {
                    let to_index = to_col.len();
                    to_col.push(from_col.swap_remove(remove_from_index));
                    return Some(to_index);
                }
            }
        }
        None
    }
}

#[inline]
pub fn slice_get_mut2<T>(
    slice: &mut [T],
    index1: usize,
    index2: usize,
) -> Option<(&'_ mut T, &'_ mut T)> {
    match index1.cmp(&index2) {
        Ordering::Less => {
            let (a, b) = slice.split_at_mut(index2);
            Some((&mut a[index1], &mut b[0]))
        }
        Ordering::Greater => {
            let (a, b) = slice.split_at_mut(index1);
            Some((&mut b[0], &mut a[index2]))
        }
        Ordering::Equal => None,
    }
}
