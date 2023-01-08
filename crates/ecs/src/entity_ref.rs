use std::{any::Any, panic};

use crate::{
    archetype::{Archetype, ArchetypeId},
    component::{Component, ComponentId, ComponentMap, ComponentSet, Components, Ref, RefMut},
    entity::{Entity, EntityLocation},
    get_or_init_component,
    resource::{Res, ResMut, ResourceId, Resources},
    storage::{AnyStorage, Storage},
    world::{World, WorldMut},
    WorldInner,
};

/// A shared reference to a entity of a world.
pub struct EntityRef<'w> {
    res: &'w Resources,
    world: &'w WorldInner,
    entity: Entity,
    location: EntityLocation,
}

impl<'w> EntityRef<'w> {
    #[inline]
    fn new(
        res: &'w Resources,
        world: &'w WorldInner,
        entity: Entity,
        location: EntityLocation,
    ) -> Self {
        Self {
            res,
            world,
            entity,
            location,
        }
    }

    /// Returns the id this entity
    #[inline]
    pub fn id(&self) -> Entity {
        self.entity
    }

    #[inline]
    pub fn archetype(&self) -> &Archetype {
        &self.world.archetypes[self.location.archetype_id]
    }

    #[inline]
    pub fn contains<T>(&self) -> bool
    where
        T: Component,
    {
        let Some(component_id) = self.world.components.id::<T>() else {
            return false;
        };
        self.contains_id(component_id)
    }

    pub fn contains_id<X>(&self, component_id: ComponentId<X>) -> bool
    where
        X: Component,
    {
        if component_id.is_sparse() {
            matches!(storage_dyn(self.res, &self.world.components, component_id), Some(storage) if storage.contains(self.entity, self.location.archetype_id, self.location.index))
        } else {
            self.archetype().components.contains(component_id)
        }
    }

    /// Returns a shared reference to the given component of this entity.
    #[inline]
    pub fn borrow<T>(&self) -> Option<Ref<'_, T>>
    where
        T: Component,
    {
        let component_id = self.world.components.id::<T>()?;
        self.borrow_id::<T>(component_id)
    }

    /// Returns a shared reference to the given component of this entity.
    #[inline]
    pub fn borrow_id<T>(&self, component_id: ComponentId<T>) -> Option<Ref<'_, T>>
    where
        T: Component,
    {
        let storage = storage(self.res, &self.world.components, component_id)?;
        Ref::filter_map(storage, |storage| {
            storage.get(self.entity, self.location.archetype_id, self.location.index)
        })
    }
}

/// An exclusive reference to a entity of a world.
pub struct EntityMut<'w> {
    res: &'w mut Resources,
    world: &'w mut WorldInner,
    entity: Entity,
    location: EntityLocation,
    remove_components: ComponentSet,
    insert_components: ComponentMap<Box<dyn Any>>,
}

impl<'w> EntityMut<'w> {
    fn new(
        res: &'w mut Resources,
        world: &'w mut WorldInner,
        entity: Entity,
        location: EntityLocation,
    ) -> Self {
        Self {
            res,
            world,
            entity,
            location,
            remove_components: ComponentSet::new(),
            insert_components: ComponentMap::new(),
        }
    }

    /// Returns the id this entity
    #[inline]
    pub const fn id(&self) -> Entity {
        self.entity
    }

    #[inline]
    pub fn archetype(&self) -> &Archetype {
        &self.world.archetypes[self.location.archetype_id]
    }

    #[inline]
    pub fn contains<T>(&self) -> bool
    where
        T: Component,
    {
        let Some(component_id) = self.world.components.id::<T>() else {
            return false;
        };
        self.contains_id(component_id)
    }

    pub fn contains_id<X>(&self, component_id: ComponentId<X>) -> bool
    where
        X: Component,
    {
        if self.remove_components.contains(component_id) {
            return false;
        } else if self.insert_components.contains(component_id) {
            return true;
        }
        if component_id.is_sparse() {
            matches!(storage_dyn(self.res, &self.world.components, component_id), Some(storage) if storage.contains(self.entity, self.location.archetype_id, self.location.index))
        } else {
            self.archetype().components.contains(component_id)
        }
    }

    /// Returns a shared reference to the given component of this entity.
    #[inline]
    pub fn borrow<T>(&self) -> Option<Ref<'_, T>>
    where
        T: Component,
    {
        let component_id = self.world.components.id::<T>()?;
        self.borrow_by_id::<T>(component_id)
    }

    /// Returns a shared reference to the given component of this entity.
    #[inline]
    pub fn borrow_by_id<T>(&self, component_id: ComponentId<T>) -> Option<Ref<'_, T>>
    where
        T: Component,
    {
        let storage = storage::<T>(self.res, &self.world.components, component_id)?;
        Ref::filter_map(storage, |storage| {
            storage.get(self.entity, self.location.archetype_id, self.location.index)
        })
    }

    /// Returns an exclusive reference to the given component of this entity, if not already borrowed
    #[inline]
    pub fn borrow_mut<T>(&self) -> Option<RefMut<'_, T>>
    where
        T: Component,
    {
        let component_id = self.world.components.id::<T>()?;
        self.borrow_mut_by_id::<T>(component_id)
    }

    /// Returns a shared reference to the given component of this entity.
    #[inline]
    pub fn borrow_mut_by_id<T>(&self, component_id: ComponentId<T>) -> Option<RefMut<'_, T>>
    where
        T: Component,
    {
        let storage = storage_mut::<T>(self.res, &self.world.components, component_id)?;
        RefMut::filter_map(storage, |storage| {
            storage.get_mut(self.entity, self.location.archetype_id, self.location.index)
        })
    }

    #[inline]
    pub fn insert<T>(&mut self, value: T) -> &mut Self
    where
        T: Component,
    {
        let component_id = get_or_init_component::<T>(self.res, &mut self.world.components).1;
        self.insert_by_id(component_id, value)
    }

    pub fn insert_by_id<T>(&mut self, component_id: ComponentId<T>, value: T) -> &mut Self
    where
        T: Component,
    {
        self.remove_components.remove(component_id);
        // TODO: try to improve performance by using a pool inside 'world' for reducing allocations?
        self.insert_components
            .insert(component_id, Box::new(Some(value)));
        self
    }

    #[inline]
    pub fn remove<T>(&mut self) -> &mut Self
    where
        T: Component,
    {
        if let Some(id) = self.world.components.id::<T>() {
            self.remove_by_id(id);
        }
        self
    }

    pub fn remove_by_id<X>(&mut self, component_id: ComponentId<X>) -> &mut Self {
        self.insert_components.remove(component_id);
        self.remove_components.insert(component_id);
        self
    }

    pub fn clear(&mut self) -> &mut Self {
        // clear open operations
        self.insert_components.clear();

        // mark all components for removal
        self.remove_components = self.world.components.to_set();

        self
    }

    /// Removes the entity and all its components from the world.
    ///
    /// Like `clear`, but also removes the entity from the world.
    pub fn despawn(mut self) {
        // clear open operations
        self.remove_components.clear();
        self.insert_components.clear();

        let location = self.location;

        // remove components and track removal
        let track_removed = &mut self.world.removed;
        for component in &self.world.components.components {
            let id = component.id();
            if let Some(storage) = storage_mut_dyn(self.res, &self.world.components, id) {
                // remove
                if storage.swap_remove(self.entity, location.archetype_id, location.index) {
                    // track
                    track_removed.get_or_insert_default(id).push(self.entity);
                }
            }
        }

        // remove entity from archetype by swapping
        if location.is_occupied() {
            let archetype = self
                .world
                .archetypes
                .get_mut(location.archetype_id)
                .expect("archetype");
            archetype.entities.swap_remove(location.index);
            if let Some(old_swapped) = archetype.entities.get(location.index).copied() {
                *self
                    .world
                    .entities
                    .get_mut(old_swapped)
                    .expect("swapped entity") = location;
            }
        }

        self.location = EntityLocation::VACANT;
        self.world.entities.remove(self.entity);
    }
}

impl<'w> Drop for EntityMut<'w> {
    fn drop(&mut self) {
        let old = self.location;
        let components = &self.world.components;
        let old_archetype = self
            .world
            .archetypes
            .get(old.archetype_id)
            .expect("old.archetype");

        // split & cleanup `remove_components`
        // remove_dense will contain the components, that will be removed from the archetype
        let mut remove_dense = Vec::new();
        for id in std::mem::take(&mut self.remove_components).iter(components) {
            if id.is_sparse() {
                // apply removal of sparse components
                if let Some(storage) = storage_mut_dyn(self.res, &self.world.components, id) {
                    if storage.swap_remove(self.entity, old.archetype_id, old.index) {
                        // track removal
                        self.world
                            .removed
                            .get_or_insert_default(id)
                            .push(self.entity);
                    }
                }
            } else if old_archetype.components.contains(id) {
                // remember removal of dense components
                remove_dense.push(id);

                // track removal
                self.world
                    .removed
                    .get_or_insert_default(id)
                    .push(self.entity);
            }
        }

        // split & cleanup `insert_components`
        // insert_dense will contain the components, that will be added to the archetype
        let mut insert_dense = Vec::new();
        for (id, mut box_value) in std::mem::take(&mut self.insert_components).into_entries() {
            if id.is_sparse() {
                // apply insertion of sparse components
                let Some(storage) = storage_mut_dyn(self.res, &self.world.components, id) else {
                    panic!("component {id:?} is not available as storage");
                };
                if storage
                    .insert(self.entity, old.archetype_id, box_value.as_mut())
                    .is_none()
                {
                    panic!(
                        "unexpected type {:?} != {:?} of sparse component with id {:?}",
                        box_value.type_id(),
                        storage.component_type_id(),
                        id
                    );
                }
            } else if old_archetype.components.contains(id) {
                let Some(storage) = storage_mut_dyn(self.res, &self.world.components, id) else {
                    panic!("component {id:?} is not available as storage");
                };
                // update existing dense components
                if !storage.replace(self.entity, old.archetype_id, old.index, box_value.as_mut()) {
                    panic!(
                        "unexpected type {:?} != {:?} of dense component with id {:?}",
                        box_value.type_id(),
                        storage.component_type_id(),
                        id
                    );
                }
            } else {
                // remember new dense components
                insert_dense.push((id, box_value));
            }
        }

        if remove_dense.is_empty() && insert_dense.is_empty() {
            // no archetype change: stop here
            return;
        }

        // find the new archetype by the components, that it will contain
        let mut new_components = old_archetype.components.clone();
        for rm in &remove_dense {
            new_components.remove(*rm);
        }
        for (ins, _) in &insert_dense {
            new_components.insert(*ins);
        }

        let new_archetype_id = self.world.archetypes.get_or_insert(new_components);
        debug_assert_ne!(old.archetype_id, new_archetype_id);

        let [old_archetype, new_archetype] = self
            .world
            .archetypes
            .get_disjoint_array_mut([old.archetype_id, new_archetype_id])
            .expect("unable to find archetypes");
        let new_index = new_archetype.len();

        // move or remove old components
        for id in old_archetype.components.iter(components) {
            let Some(storage) = storage_mut_dyn(self.res, &self.world.components, id) else {
                panic!("component {id:?} is not available as storage");
            };
            if new_archetype.components.contains(id) {
                let result = storage.swap_remove_and_insert_to(
                    self.entity,
                    old.archetype_id,
                    old.index,
                    new_archetype_id,
                );
                assert_eq!(
                    Some(new_index),
                    result,
                    "unexpected type of storage {:?} of dense component with id {:?}",
                    storage.component_type_id(),
                    id
                );
            } else {
                storage.swap_remove(self.entity, old.archetype_id, old.index);
            }
        }
        // insert new components
        for (id, mut box_value) in insert_dense {
            let Some(storage) = storage_mut_dyn(self.res, &self.world.components, id) else {
                panic!("component {id:?} is not available as storage");
            };
            let result = storage.insert(self.entity, new_archetype_id, box_value.as_mut());
            assert_eq!(
                Some(new_index),
                result,
                "unexpected type {:?} != {:?} of dense component with id {:?}",
                box_value.type_id(),
                storage.component_type_id(),
                id
            );
        }

        // move entity by swaping entity locations
        // remove from old
        if old.is_occupied() {
            old_archetype.entities.swap_remove(old.index);
            if let Some(old_swapped) = old_archetype.entities.get(old.index).copied() {
                *self
                    .world
                    .entities
                    .get_mut(old_swapped)
                    .expect("swapped entity") = self.location;
            }
        }
        // set new location
        self.location = EntityLocation {
            archetype_id: new_archetype_id,
            index: new_index,
        };
        new_archetype.entities.push(self.entity);
        *self.world.entities.get_mut(self.entity).expect("entity") = self.location;
    }
}

fn storage<'a, T>(
    res: &'a Resources,
    comps: &Components,
    component_id: ComponentId<T>,
) -> Option<Res<'a, T::Storage>>
where
    T: Component,
{
    let component = comps.get(component_id)?;
    let storage_id: ResourceId<T::Storage> = component.storage_id.typed();
    res.borrow_res_id(storage_id)
}

fn storage_mut<'a, T>(
    res: &'a Resources,
    comps: &Components,
    component_id: ComponentId<T>,
) -> Option<ResMut<'a, T::Storage>>
where
    T: Component,
{
    let component = comps.get(component_id)?;
    let storage_id: ResourceId<T::Storage> = component.storage_id.typed();
    res.borrow_res_mut_id(storage_id)
}

fn storage_dyn<'a, X>(
    res: &'a Resources,
    comps: &Components,
    component_id: ComponentId<X>,
) -> Option<Res<'a, dyn AnyStorage>>
where
    X: 'static,
{
    let component = comps.get(component_id)?;
    (component.any_getter)(res, component.storage_id)
}

fn storage_mut_dyn<'a, X>(
    res: &'a mut Resources,
    comps: &Components,
    component_id: ComponentId<X>,
) -> Option<&'a mut dyn AnyStorage>
where
    X: 'static,
{
    let component = comps.get(component_id)?;
    (component.any_getter_mut)(res, component.storage_id)
}

impl World<'_> {
    /// Returns a shared reference ([`EntityRef`]) to the entity with the given
    /// id.
    pub fn entity(&self, entity: Entity) -> Option<EntityRef<'_>> {
        self.world
            .entities
            .get(entity)
            .map(|location| EntityRef::new(self.res, &self.world, entity, location))
    }
}
impl WorldMut<'_> {
    /// Returns a shared reference ([`EntityRef`]) to the entity with the given
    /// id.
    pub fn entity(&self, entity: Entity) -> Option<EntityRef<'_>> {
        self.world
            .entities
            .get(entity)
            .map(|location| EntityRef::new(self.res, &self.world, entity, location))
    }

    /// Returns an exclusive reference ([`EntityMut`]) to the entity with the
    /// given id.
    pub fn entity_mut(&mut self, entity: Entity) -> Option<EntityMut<'_>> {
        let Some(location) = self.world.entities.get_mut(entity) else {
            return None;
        };
        let location = *location;
        Some(EntityMut::new(self.res, &mut self.world, entity, location))
    }

    /// Spawns/creates an new empty [`Entity`] in this `World` and returns a handle
    /// for modifying it.
    #[must_use]
    pub fn spawn(&mut self) -> EntityMut<'_> {
        let entity = self.world.entities.create();
        let empty_archetype = &mut self
            .world
            .archetypes
            .get_mut(ArchetypeId::EMPTY)
            .expect("empty archetype");
        let index = empty_archetype.len();
        empty_archetype.entities.push(entity);
        self.world.entities.get_mut(entity).expect("entity").index = index;
        let location = self.world.entities[entity];
        EntityMut::new(self.res, &mut self.world, entity, location)
    }
}
