use std::cell::{Cell, UnsafeCell};

use pulz_schedule::prelude::Resources;

use crate::{
    WorldInner, WorldMutInnerTemp,
    archetype::{Archetype, ArchetypeId},
    component::{Component, ComponentId, Ref, RefMut},
    entity::{Entity, EntityLocation},
    storage::Storage,
    world::{World, WorldMut},
};

/// A shared reference to a entity of a world.
pub struct EntityRef<'w> {
    res: &'w Resources,
    world: &'w WorldInner,
    entity: Entity,
    location: EntityLocation,
}

macro_rules! impl_common_nonmut {
    () => {
        /// Returns the id this entity
        #[inline]
        pub fn id(&self) -> Entity {
            self.entity
        }

        /// Returns the archetype this entity currently belongs to.
        #[inline]
        pub fn archetype(&self) -> &Archetype {
            let location = self.location();
            &self.world.archetypes[location.archetype_id]
        }

        /// Returns `true` if this entity has component `T`.
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

        /// Returns `true` if this entity has the component identified by `component_id`.
        #[inline]
        pub fn contains_id<X>(&self, component_id: ComponentId<X>) -> bool
        where
            X: Component,
        {
            self.location(); // flush
            self.world
                .contains_component(self.res, self.entity, component_id.untyped())
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
            let location = self.location();
            let component = &self.world.components.get(component_id)?;
            let storage = component.borrow_res_storage::<T>(self.res)?;
            Ref::filter_map(storage, |storage| {
                storage.get(self.entity, location.archetype_id, location.index())
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
            let location = self.location();
            let component = &self.world.components.get(component_id)?;
            let storage = component.borrow_mut_storage::<T>(self.res)?;
            RefMut::filter_map(storage, |storage| {
                storage.get_mut(self.entity, location.archetype_id, location.index())
            })
        }
    };
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

    /// Returns the current archetype location of this entity.
    #[inline]
    pub fn location(&self) -> EntityLocation {
        self.location
    }

    impl_common_nonmut! {}
}

struct UnsafeRefCell<'a, T: ?Sized>(UnsafeCell<&'a mut T>);

impl<'a, T: ?Sized> UnsafeRefCell<'a, T> {
    #[inline]
    const fn new(value: &'a mut T) -> Self {
        Self(UnsafeCell::new(value))
    }

    #[inline]
    fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.0.get() }
    }

    #[inline]
    fn get(&self) -> &T {
        unsafe { &*self.0.get() }
    }

    #[inline]
    #[allow(clippy::mut_from_ref)]
    unsafe fn get_mut_unchecked(&self) -> &mut T {
        unsafe { &mut *self.0.get() }
    }
}

impl<T: ?Sized> std::ops::Deref for UnsafeRefCell<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T: ?Sized> std::ops::DerefMut for UnsafeRefCell<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

/// An exclusive reference to a entity of a world.
pub struct EntityMut<'w> {
    res: &'w mut Resources,
    world: UnsafeRefCell<'w, WorldInner>,
    world_tmp: UnsafeRefCell<'w, WorldMutInnerTemp>,
    entity: Entity,
    location: Cell<EntityLocation>,
}

impl<'w> EntityMut<'w> {
    fn new(
        res: &'w mut Resources,
        world: &'w mut WorldInner,
        world_tmp: &'w mut WorldMutInnerTemp,
        entity: Entity,
        location: EntityLocation,
    ) -> Self {
        debug_assert!(world_tmp.tmp_removed.is_empty());
        debug_assert!(world_tmp.tmp_inserted.is_empty());
        Self {
            res,
            world: UnsafeRefCell::new(world),
            world_tmp: UnsafeRefCell::new(world_tmp),
            entity,
            location: Cell::new(location),
        }
    }

    /// Returns the current archetype location of this entity, flushing any pending changes first.
    #[inline]
    pub fn location(&self) -> EntityLocation {
        self.flush();
        self.location.get()
    }

    impl_common_nonmut! {}

    /// Inserts or replaces component `T` on this entity, registering it if needed.
    #[inline]
    pub fn insert<T>(&mut self, value: T) -> &mut Self
    where
        T: Component,
    {
        let component_id = self.world.init_component(self.res);
        self.insert_by_id(component_id, value)
    }

    /// Inserts or replaces component with a pre-resolved `component_id`.
    ///
    /// # Panics
    ///
    /// Panics if the component metadata for `component_id` is not found.
    pub fn insert_by_id<T>(&mut self, component_id: ComponentId<T>, value: T) -> &mut Self
    where
        T: Component,
    {
        self.world_tmp.tmp_removed.remove(component_id);
        self.world_tmp.tmp_inserted.insert(component_id);
        {
            let component = &self.world.components.get(component_id).expect("component");
            let mut storage = component
                .borrow_mut_storage::<T>(self.res)
                .expect("storage");
            storage.insert(self.entity, value);
        }
        self
    }

    /// Removes component `T` from this entity, if it is registered and present.
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

    /// Removes a component by its pre-resolved id.
    pub fn remove_by_id<X>(&mut self, component_id: ComponentId<X>) -> &mut Self {
        self.world_tmp.tmp_inserted.remove(component_id);
        self.world_tmp.tmp_removed.insert(component_id);
        self
    }

    /// Queues removal of all components currently on this entity.
    pub fn clear(&mut self) -> &mut Self {
        // clear open operations
        self.reset();

        // mark all components for removal
        if let Some(iter) = self.world.get_components(self.res, self.entity) {
            self.world_tmp.tmp_removed.extend(iter.map(|c| c.id()));
        }
        self
    }

    /// Resets the temporary state of this entity, clearing all open operations.
    pub fn reset(&mut self) {
        // clear open operations
        self.world_tmp.tmp_removed.clear();
        self.world_tmp.tmp_inserted.clear();
    }

    /// Removes the entity and all its components from the world.
    ///
    /// Like `clear`, but also removes the entity from the world.
    ///
    /// # Panics
    ///
    /// Panics if the swapped entity's location cannot be found in the entities map.
    /// Should never happen unless the world is corrupted.
    pub fn despawn(mut self) {
        self.reset();
        let location = self.location.get();

        // remove components and track removal
        if let Some(iter) = self.world.get_components_fast(self.entity) {
            for component in iter {
                let mut storage = component.borrow_mut_any_storage(self.res);
                // remove
                if storage.swap_remove(self.entity, location.archetype_id, location.index()) {
                    // TODO: track removed
                }
            }
        }

        // remove entity from archetype by swapping
        if location.is_occupied() {
            let archetype = &mut self.world.archetypes[location.archetype_id];
            archetype.entities.swap_remove(location.index());
            if let Some(old_swapped) = archetype.entities.get(location.index()).copied() {
                *self
                    .world
                    .entities
                    .get_mut(old_swapped)
                    .expect("swapped entity") = location;
            }
        }

        self.location.set(EntityLocation::VACANT);
        self.world.entities.remove(self.entity);
    }

    fn flush(&self) {
        // SAFETY: mutable access only, when already has mutable access, throug get_mut(),
        // or only here
        let world_tmp = unsafe { self.world_tmp.get_mut_unchecked() };
        if world_tmp.tmp_removed.is_empty() && world_tmp.tmp_inserted.is_empty() {
            return; // nothing to do
        }

        let world = unsafe { self.world.get_mut_unchecked() };

        let old = self.location.get();
        let old_archetype = world
            .archetypes
            .get(old.archetype_id)
            .expect("old.archetype");

        let mut needs_update_archetype = false;

        // remove components
        // TODO: track_removed
        world_tmp.tmp_removed = world_tmp.tmp_removed.filter(|component_id| {
            let component = &world.components[component_id];
            let mut storage = component.borrow_mut_any_storage(self.res);
            if storage.swap_remove(self.entity, old.archetype_id, old.index()) {
                if world.archetypes.is_archetype_component(component_id) {
                    needs_update_archetype = true;
                }
                return true;
            }
            false
        });

        // replace existing components
        world_tmp.tmp_inserted = world_tmp.tmp_inserted.filter(|component_id| {
            let component = &world.components[component_id];
            let mut storage = component.borrow_mut_any_storage(self.res);
            if !storage.flush_replace(old.archetype_id, old.index()) {
                if world.archetypes.is_archetype_component(component_id) {
                    needs_update_archetype = true;
                }
                return true;
            }
            false
        });

        if !needs_update_archetype {
            world_tmp.tmp_removed.clear();
            world_tmp.tmp_inserted.clear();
            return;
        }

        // calculate new archetype
        let mut new_components = old_archetype.components().clone();
        new_components.difference_with(&world_tmp.tmp_removed);
        new_components.union_with(&world_tmp.tmp_inserted);
        let new_archetype_id = world.archetypes.get_or_insert_by_components(new_components);
        debug_assert_ne!(old.archetype_id, new_archetype_id);

        let [old_archetype, new_archetype] = world
            .archetypes
            .get_disjoint_mut([old.archetype_id, new_archetype_id])
            .expect("unable to find archetypes");
        let new_index = new_archetype.len();

        // move old components
        for component in old_archetype
            .components()
            .iter()
            .map(|id| &world.components[id])
        {
            let id = component.id();
            if new_archetype.contains(id) {
                let mut storage = component.borrow_mut_any_storage(self.res);
                let result =
                    storage.swap_remove_and_push(old.archetype_id, old.index(), new_archetype_id);
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
        for component in world_tmp
            .tmp_inserted
            .iter()
            .map(|id| &world.components[id])
        {
            let id = component.id();
            let mut storage = component.borrow_mut_any_storage(self.res);
            let result = storage.flush_push(new_archetype_id);
            assert_eq!(
                Some(new_index),
                result,
                "unexpected index of component {:?}({}) (flush push)",
                id,
                component.name(),
            );
        }

        world_tmp.tmp_removed.clear();
        world_tmp.tmp_inserted.clear();

        // set new location
        let new_location = EntityLocation {
            archetype_id: new_archetype_id,
            index: new_index as u32,
        };
        self.location.set(new_location);

        // move entity by swaping entity locations
        // remove from old
        if old.is_occupied() {
            old_archetype.entities.swap_remove(old.index());
            if let Some(old_swapped) = old_archetype.entities.get(old.index()).copied() {
                *world.entities.get_mut(old_swapped).expect("swapped entity") = new_location;
            }
        }
        new_archetype.entities.push(self.entity);
        *world.entities.get_mut(self.entity).expect("entity") = new_location;
    }
}

impl Drop for EntityMut<'_> {
    fn drop(&mut self) {
        self.flush();
    }
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
        Some(EntityMut::new(
            self.res,
            &mut self.world,
            &mut self.world_tmp,
            entity,
            location,
        ))
    }

    /// Spawns/creates an new empty [`Entity`] in this `World` and returns a handle
    /// for modifying it.
    #[must_use]
    #[allow(clippy::missing_panics_doc)] // panic should never happen
    pub fn spawn(&mut self) -> EntityMut<'_> {
        let entity = self.world.entities.create();
        let empty_archetype = &mut self
            .world
            .archetypes
            .get_mut(ArchetypeId::EMPTY)
            .expect("empty archetype");
        let index = empty_archetype.len();
        empty_archetype.entities.push(entity);
        self.world.entities.get_mut(entity).expect("entity").index = index as u32;
        let location = self.world.entities[entity];
        EntityMut::new(
            self.res,
            &mut self.world,
            &mut self.world_tmp,
            entity,
            location,
        )
    }
}
