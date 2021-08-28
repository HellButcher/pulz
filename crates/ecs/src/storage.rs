use std::{
    any::{Any, TypeId},
    cell::{Ref, RefCell, RefMut},
    cmp::Ordering,
};

use fnv::FnvHashMap;

use crate::{
    archetype::ArchetypeId,
    component::{ComponentId, ComponentMap, ComponentSet, Components},
    Entity,
};

pub struct ComponentStorageMap(ComponentMap<RefCell<Box<dyn AnyStorage>>>);

impl ComponentStorageMap {
    #[inline]
    pub fn new() -> Self {
        Self(ComponentMap::new())
    }

    #[inline]
    pub fn contains_id(&self, component_id: ComponentId) -> bool {
        self.0.contains(component_id)
    }

    #[inline]
    pub fn component_ids(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.0.keys()
    }

    #[inline]
    pub fn component_id_set(&self) -> ComponentSet {
        self.0.key_set()
    }

    #[inline]
    pub fn entries_mut(&mut self) -> impl Iterator<Item = (ComponentId, &mut dyn AnyStorage)> + '_ {
        self.0
            .entries_mut()
            .map(|(id, boxed)| (id, boxed.get_mut().as_mut()))
    }

    #[inline]
    pub fn get_mut_dyn(&mut self, component_id: ComponentId) -> Option<&mut dyn AnyStorage> {
        Some(self.0.get_mut(component_id)?.get_mut().as_mut())
    }

    #[inline]
    pub fn get_mut<T>(&mut self, component_id: ComponentId) -> Option<&mut Storage<T>>
    where
        T: 'static,
    {
        self.get_mut_dyn(component_id)
            .and_then(|storage| Storage::from_mut_dyn(storage))
    }

    #[inline]
    pub fn borrow_dyn(&self, component_id: ComponentId) -> Option<Ref<'_, dyn AnyStorage>> {
        Some(Ref::map(self.0.get(component_id)?.borrow(), Box::as_ref))
    }

    #[inline]
    pub fn borrow_mut_dyn(&self, component_id: ComponentId) -> Option<RefMut<'_, dyn AnyStorage>> {
        Some(RefMut::map(
            self.0.get(component_id)?.borrow_mut(),
            Box::as_mut,
        ))
    }

    #[inline]
    pub fn borrow<T>(&self, component_id: ComponentId) -> Option<Ref<'_, Storage<T>>>
    where
        T: 'static,
    {
        let v = self.borrow_dyn(component_id)?;
        if TypeId::of::<T>() != v.component_type_id() {
            return None;
        }
        Some(Ref::map(v, |storage| {
            // SAFETY: just checked whether we are pointing to the correct component type
            unsafe {
                let storage: *const dyn AnyStorage = storage;
                &*(storage as *const Storage<T>)
            }
        }))
    }

    #[inline]
    pub fn borrow_mut<T>(&self, component_id: ComponentId) -> Option<RefMut<'_, Storage<T>>>
    where
        T: 'static,
    {
        let v = self.borrow_mut_dyn(component_id)?;
        if TypeId::of::<T>() != v.component_type_id() {
            return None;
        }
        Some(RefMut::map(v, |storage| {
            // SAFETY: just checked whether we are pointing to the correct component type
            unsafe {
                let storage: *mut dyn AnyStorage = storage;
                &mut *(storage as *mut Storage<T>)
            }
        }))
    }

    #[inline]
    pub fn get_or_insert_with<F>(
        &mut self,
        component_id: ComponentId,
        create: F,
    ) -> &mut dyn AnyStorage
    where
        F: FnOnce() -> Box<dyn AnyStorage>,
    {
        self.0
            .get_or_insert_with(component_id, || RefCell::new(create()))
            .get_mut()
            .as_mut()
    }

    #[inline]
    pub fn get_or_insert(
        &mut self,
        with_components: &Components,
        component_id: ComponentId,
    ) -> &mut dyn AnyStorage {
        self.get_or_insert_with(component_id, || {
            let component = &with_components.components[component_id.offset()];
            (component.new_storage)()
        })
    }
}

pub enum Storage<T> {
    Dense(Vec<Vec<T>>),
    Sparse(FnvHashMap<Entity, T>),
}

pub trait AnyStorage: Any {
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
    T: 'static,
{
    fn take_t(value: &mut dyn Any) -> Option<T> {
        value.downcast_mut::<Option<T>>()?.take()
    }

    #[inline]
    fn from_dyn(storage: &mut dyn AnyStorage) -> Option<&Self> {
        if TypeId::of::<T>() == storage.component_type_id() {
            // SAFETY: just checked whether we are pointing to the correct type
            unsafe {
                let storage: *const dyn AnyStorage = storage;
                Some(&*(storage as *const Self))
            }
        } else {
            None
        }
    }

    #[inline]
    fn from_mut_dyn(storage: &mut dyn AnyStorage) -> Option<&mut Self> {
        if TypeId::of::<T>() == storage.component_type_id() {
            // SAFETY: just checked whether we are pointing to the correct type
            unsafe {
                let storage: *mut dyn AnyStorage = storage;
                Some(&mut *(storage as *mut Self))
            }
        } else {
            None
        }
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
    T: 'static,
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
