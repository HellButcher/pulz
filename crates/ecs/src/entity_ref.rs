use std::any::TypeId;

use crate::{
    WorldInner,
    archetype::{Archetype, ArchetypeId},
    component::{Component, ComponentDetails, ComponentId, Ref, RefMut},
    entity::{Entity, EntityLocation},
    get_or_init_component,
    resource::{Res, ResMut, ResourceId, Resources},
    storage::{AnyStorage, Storage},
    world::{World, WorldMut},
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
        let archetype = &self.world.archetypes[self.location.archetype_id];
        if let Some(component) = self.world.components.get(component_id) {
            X::Storage::fast_contains(self.res, self.entity, component, archetype)
        } else {
            false
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
        let component = &self.world.components.get(component_id)?;
        let storage = storage::<T>(self.res, component)?;
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
}

impl<'w> EntityMut<'w> {
    fn new(
        res: &'w mut Resources,
        world: &'w mut WorldInner,
        entity: Entity,
        location: EntityLocation,
    ) -> Self {
        // reset temporaries
        world.tmp_removed.clear();
        world.tmp_inserted.clear();

        Self {
            res,
            world,
            entity,
            location,
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
        if self.world.tmp_removed.contains(component_id) {
            return false;
        } else if self.world.tmp_inserted.contains(component_id) {
            return true;
        }
        let archetype = &self.world.archetypes[self.location.archetype_id];
        if let Some(component) = self.world.components.get(component_id) {
            X::Storage::fast_contains(self.res, self.entity, component, archetype)
        } else {
            false
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
        let component = &self.world.components.get(component_id)?;
        let storage = storage::<T>(self.res, component)?;
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
        let component = &self.world.components.get(component_id)?;
        let storage = storage_mut::<T>(self.res, component)?;
        RefMut::filter_map(storage, |storage| {
            storage.get_mut(self.entity, self.location.archetype_id, self.location.index)
        })
    }

    #[inline]
    pub fn insert<T>(&mut self, value: T) -> &mut Self
    where
        T: Component,
    {
        let (_, component_id) = get_or_init_component::<T>(self.res, &mut self.world.components);
        self.insert_by_id(component_id, value)
    }

    pub fn insert_by_id<T>(&mut self, component_id: ComponentId<T>, value: T) -> &mut Self
    where
        T: Component,
    {
        self.world.tmp_removed.remove(component_id);
        self.world.tmp_inserted.insert(component_id);
        let component = &self.world.components.get(component_id).expect("component");
        {
            let mut storage = storage_mut::<T>(self.res, component).expect("storage");
            storage.insert(self.entity, value);
        }
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
        self.world.tmp_inserted.remove(component_id);
        self.world.tmp_removed.insert(component_id);
        self
    }

    pub fn clear(&mut self) -> &mut Self {
        // clear open operations
        self.world.tmp_inserted.clear();

        // mark all components for removal
        self.world
            .tmp_removed
            .insert_range(0..self.world.components.len());
        self
    }

    /// Removes the entity and all its components from the world.
    ///
    /// Like `clear`, but also removes the entity from the world.
    pub fn despawn(mut self) {
        // clear open operations
        self.world.tmp_removed.clear();
        self.world.tmp_inserted.clear();

        let location = self.location;

        // remove components and track removal
        // TODO: track_removed
        for component in &self.world.components.components {
            let id = component.id();
            if let Some(storage) = storage_mut_dyn(self.res, component) {
                // remove
                if storage.swap_remove(self.entity, location.archetype_id, location.index) {
                    // track
                    self.world.tmp_removed.insert(id);
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

impl Drop for EntityMut<'_> {
    fn drop(&mut self) {
        let old = self.location;
        let old_archetype = self
            .world
            .archetypes
            .get(old.archetype_id)
            .expect("old.archetype");

        let mut needs_update_archetype = false;

        // remove components
        // TODO: track_removed
        self.world.tmp_removed.retain(|index| {
            let component = &self.world.components.components[index];
            if let Some(storage) = storage_mut_dyn(self.res, component)
                && storage.swap_remove(self.entity, old.archetype_id, old.index) {
                    if component.archetype_component {
                        needs_update_archetype = true;
                    }
                    return true;
                }
            false
        });

        // replace existing components
        self.world.tmp_inserted.retain(|index| {
            let component = &self.world.components.components[index];
            let storage = storage_mut_dyn(self.res, component).expect("storage");
            if !storage.flush_replace(old.archetype_id, old.index) {
                if component.archetype_component {
                    needs_update_archetype = true;
                }
                return true;
            }
            false
        });

        if !needs_update_archetype {
            return;
        }

        // calculate new archetype
        let mut new_components = old_archetype.components.clone();
        new_components.remove_set(&self.world.tmp_removed);
        for component in self.world.tmp_inserted.iter_details(&self.world.components) {
            if component.archetype_component {
                new_components.insert(component.id());
            }
        }
        let new_archetype_id = self.world.archetypes.get_or_insert(new_components);
        debug_assert_ne!(old.archetype_id, new_archetype_id);

        let [old_archetype, new_archetype] = self
            .world
            .archetypes
            .get_disjoint_array_mut([old.archetype_id, new_archetype_id])
            .expect("unable to find archetypes");
        let new_index = new_archetype.len();

        // move old components
        for component in old_archetype
            .components
            .iter_details(&self.world.components)
        {
            let id = component.id();
            if new_archetype.components.contains(id) {
                let storage = storage_mut_dyn(self.res, component).expect("storage");
                let result =
                    storage.swap_remove_and_insert(old.archetype_id, old.index, new_archetype_id);
                assert_eq!(
                    Some(new_index),
                    result,
                    "unexpected index of component with id {:?}({})(swap_remove_and_insert)",
                    id,
                    component.name(),
                );
            }
        }

        // insert new ones
        for component in self.world.tmp_inserted.iter_details(&self.world.components) {
            let id = component.id();
            let storage = storage_mut_dyn(self.res, component).expect("storage");
            let result = storage.flush_push(new_archetype_id);
            assert_eq!(
                Some(new_index),
                result,
                "unexpected index of component {:?}({}) (flush push)",
                id,
                component.name(),
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

fn storage<'a, T>(res: &'a Resources, component: &ComponentDetails) -> Option<Res<'a, T::Storage>>
where
    T: Component,
{
    debug_assert_eq!(TypeId::of::<T>(), component.type_id());
    let storage_id: ResourceId<T::Storage> = component.storage_id.typed();
    res.borrow_res_id(storage_id)
}

fn storage_mut<'a, T>(
    res: &'a Resources,
    component: &ComponentDetails,
) -> Option<ResMut<'a, T::Storage>>
where
    T: Component,
{
    debug_assert_eq!(TypeId::of::<T>(), component.type_id());
    let storage_id: ResourceId<T::Storage> = component.storage_id.typed();
    res.borrow_res_mut_id(storage_id)
}

fn storage_mut_dyn<'a>(
    res: &'a mut Resources,
    component: &ComponentDetails,
) -> Option<&'a mut dyn AnyStorage> {
    let any = res.get_mut_any(component.storage_id)?;
    Some(unsafe { (component.storage_downcast_mut)(any) })
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
        let location = self.world.entities.get_mut(entity)?;
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
