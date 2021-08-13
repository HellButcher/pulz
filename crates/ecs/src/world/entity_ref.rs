use std::any::Any;

use crate::{
    archetype::{Archetype, ArchetypeId},
    component::{ComponentId, ComponentMap, ComponentSet},
    entity::{Entity, EntityLocation},
    world::World,
};

/// A shared reference to a entity of a world.
pub struct EntityRef<'w> {
    world: &'w World,
    entity: Entity,
    location: EntityLocation,
}

impl<'w> EntityRef<'w> {
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
        T: 'static,
    {
        if let Some(id) = self.world.components.get_id::<T>() {
            self.contains_id(id)
        } else {
            false
        }
    }

    pub fn contains_id(&self, id: ComponentId) -> bool {
        if id.is_sparse() {
            matches!(self.world.sparse_storage.get(id), Some(storage) if storage.contains(self.entity, self.location.index))
        } else {
            self.archetype().dense_storage.contains_id(id)
        }
    }

    /// Returns a shared reference to the given component of this entity.
    pub fn get<T>(&self) -> Option<&'w T>
    where
        T: 'static,
    {
        if let Some(component_id) = self.world.components.get_id::<T>() {
            self.get_by_id(component_id)
        } else {
            None
        }
    }

    /// Returns a shared reference to the given component of this entity.
    pub fn get_by_id<T>(&self, component_id: ComponentId) -> Option<&'w T>
    where
        T: 'static,
    {
        if component_id.is_sparse() {
            self.world
                .sparse_storage
                .typed_get::<T>(component_id)?
                .get(&self.entity)
        } else {
            let location = self.location;
            self.world.archetypes[location.archetype_id]
                .dense_storage
                .typed_get::<T>(component_id)?
                .get(location.index)
        }
    }
}

/// An exclusive reference to a entity of a world.
pub struct EntityMut<'w> {
    world: &'w mut World,
    entity: Entity,
    location: EntityLocation,
    remove_components: ComponentSet,
    insert_components: ComponentMap<Box<dyn Any>>,
}

impl<'w> EntityMut<'w> {
    fn new(world: &'w mut World, entity: Entity, location: EntityLocation) -> Self {
        Self {
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
        T: 'static,
    {
        if let Some(component_id) = self.world.components.get_id::<T>() {
            self.contains_id(component_id)
        } else {
            false
        }
    }

    pub fn contains_id(&self, component_id: ComponentId) -> bool {
        if self.remove_components.contains(component_id) {
            return false;
        } else if self.insert_components.contains(component_id) {
            return true;
        }
        if component_id.is_sparse() {
            matches!(self.world.sparse_storage.get(component_id), Some(storage) if storage.contains(self.entity, self.location.index))
        } else {
            self.archetype().dense_storage.contains_id(component_id)
        }
    }

    /// Returns a shared reference to the given component of this entity.
    #[inline]
    pub fn get<T>(&self) -> Option<&T>
    where
        T: 'static,
    {
        if let Some(component_id) = self.world.components.get_id::<T>() {
            self.get_by_id(component_id)
        } else {
            None
        }
    }

    /// Returns an exclusive reference to the given component of this entity.
    #[inline]
    pub fn get_mut<T>(&mut self) -> Option<&mut T>
    where
        T: 'static,
    {
        if let Some(component_id) = self.world.components.get_id::<T>() {
            self.get_mut_by_id(component_id)
        } else {
            None
        }
    }

    /// Returns a shared reference to the given component of this entity.
    pub fn get_by_id<T>(&self, component_id: ComponentId) -> Option<&T>
    where
        T: 'static,
    {
        if let Some(boxed) = self.insert_components.get(component_id) {
            if let Some(value) = boxed.as_ref().downcast_ref::<Option<T>>() {
                return value.as_ref();
            }
        }
        if component_id.is_sparse() {
            self.world
                .sparse_storage
                .typed_get::<T>(component_id)?
                .get(&self.entity)
        } else {
            let location = self.location;
            self.world.archetypes[location.archetype_id]
                .dense_storage
                .typed_get::<T>(component_id)?
                .get(location.index)
        }
    }

    /// Returns an exclusive reference to the given component of this entity.
    pub fn get_mut_by_id<T>(&mut self, component_id: ComponentId) -> Option<&mut T>
    where
        T: 'static,
    {
        if let Some(boxed) = self.insert_components.get_mut(component_id) {
            if let Some(value) = boxed.as_mut().downcast_mut::<Option<T>>() {
                return value.as_mut();
            }
        }
        if component_id.is_sparse() {
            self.world
                .sparse_storage
                .typed_get_mut::<T>(component_id)?
                .get_mut(&self.entity)
        } else {
            let location = self.location;
            self.world.archetypes[location.archetype_id]
                .dense_storage
                .typed_get_mut::<T>(component_id)?
                .get_mut(location.index)
        }
    }

    #[inline]
    pub fn insert<T>(&mut self, value: T) -> &mut Self
    where
        T: 'static,
    {
        let component_id = self.world.components.get_or_insert_id::<T>();
        self.insert_id(component_id, value)
    }

    pub fn insert_id<T>(&mut self, component_id: ComponentId, value: T) -> &mut Self
    where
        T: 'static,
    {
        self.remove_components.remove(component_id);
        // TODO: try to use a pool inside 'world' for reducing allocations?
        self.insert_components
            .insert(component_id, Box::new(Some(value)));
        self
    }

    #[inline]
    pub fn remove<T>(&mut self) -> &mut Self
    where
        T: 'static,
    {
        if let Some(id) = self.world.components.get_id::<T>() {
            self.remove_id(id);
        }
        self
    }

    pub fn remove_id(&mut self, component_id: ComponentId) -> &mut Self {
        self.insert_components.remove(component_id);
        self.remove_components.insert(component_id);
        self
    }

    pub fn clear(&mut self) -> &mut Self {
        // clear open operations
        self.insert_components.clear();

        // mark all components for removal
        let archetype = &self.world.archetypes[self.location.archetype_id];
        for component_id in archetype.dense_storage.component_ids() {
            self.remove_components.insert(component_id);
        }
        for (component_id, storage) in self.world.sparse_storage.entries() {
            if storage.contains(self.entity, self.location.index) {
                self.remove_components.insert(component_id);
            }
        }
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
        let archetype = &mut self.world.archetypes[location.archetype_id];

        // track removed components
        let track_removed = &mut self.world.removed;

        // remove components and track removal
        for (component_id, storage) in archetype
            .dense_storage
            .entries_mut()
            .chain(self.world.sparse_storage.entries_mut())
        {
            // remove
            if storage.swap_remove(self.entity, location.index) {
                // track
                track_removed
                    .get_or_insert_default(component_id)
                    .push(self.entity);
            }
        }

        // remove entity
        if location.index != usize::MAX {
            archetype.entities.swap_remove(location.index);
        }

        // swap entity locations
        if let Some(old_swapped) = archetype.entities.get(location.index).copied() {
            self.world.entities[old_swapped] = location;
        }
        self.location = EntityLocation::EMPTY;
        self.world.entities.remove(self.entity);
    }
}

impl Drop for EntityMut<'_> {
    fn drop(&mut self) {
        let old_archetype_id = self.location.archetype_id;
        let old_index = self.location.index;
        let components = &self.world.components;
        let archetype = &mut self.world.archetypes[old_archetype_id];

        // split & cleanup `remove_components`
        let mut remove_dense = Vec::new();
        for id in std::mem::take(&mut self.remove_components).iter(components) {
            if id.is_sparse() {
                // apply removal of sparse components
                if let Some(storage) = self.world.sparse_storage.get_mut(id) {
                    if storage.swap_remove(self.entity, self.location.index) {
                        // track removal
                        self.world
                            .removed
                            .get_or_insert_default(id)
                            .push(self.entity);
                    }
                }
            } else if archetype.dense_storage.contains_id(id) {
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
        let mut insert_dense = Vec::new();
        for (id, mut box_value) in std::mem::take(&mut self.insert_components).into_entries() {
            if id.is_sparse() {
                // apply insertion of sparse components
                let storage = self
                    .world
                    .sparse_storage
                    .get_or_insert_with(id, components.components[id.offset()].new_storage);
                if storage.insert(self.entity, box_value.as_mut()).is_none() {
                    panic!(
                        "unexpected type {:?} != {:?} of sparse component with id {:?}",
                        box_value.type_id(),
                        storage.component_type_id(),
                        id
                    );
                }
            } else if let Some(storage) = archetype.dense_storage.get_mut(id) {
                // update existing dense components
                if !storage.replace(self.entity, old_index, box_value.as_mut()) {
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

        let mut new_components = archetype.dense_storage.component_id_set();
        for rm in &remove_dense {
            new_components.remove(*rm);
        }
        for (ins, _) in insert_dense.iter() {
            new_components.insert(*ins);
        }

        let new_archetype_id = self
            .world
            .archetypes
            .get_or_insert(new_components, components);

        debug_assert_ne!(old_archetype_id, new_archetype_id);

        let (old_archetype, new_archetype) = self
            .world
            .archetypes
            .get_mut2(old_archetype_id, new_archetype_id)
            .expect("unable to find archetype");
        let new_index = new_archetype.len();

        // move entity
        if old_index != usize::MAX {
            old_archetype.entities.swap_remove(old_index);
        }
        new_archetype.entities.push(self.entity);

        // move or remove old components
        for (id, old_storage) in &mut old_archetype.dense_storage.entries_mut() {
            if let Some(new_storage) = new_archetype.dense_storage.get_mut(id) {
                let result =
                    old_storage.swap_remove_and_insert_to(self.entity, old_index, new_storage);
                assert_eq!(
                    Some(new_index),
                    result,
                    "unexpected type of storage {:?} != {:?} of dense component with id {:?}",
                    old_storage.component_type_id(),
                    new_storage.component_type_id(),
                    id
                );
            } else {
                old_storage.swap_remove(self.entity, old_index);
            }
        }
        // insert new components
        for (id, mut box_value) in insert_dense {
            if let Some(storage) = new_archetype.dense_storage.get_mut(id) {
                let result = storage.insert(self.entity, box_value.as_mut());
                assert_eq!(
                    Some(new_index),
                    result,
                    "unexpected type {:?} != {:?} of dense component with id {:?}",
                    box_value.type_id(),
                    storage.component_type_id(),
                    id
                );
            }
        }
        // swap entity locations
        if let Some(old_swapped) = old_archetype.entities.get(old_index).copied() {
            self.world.entities[old_swapped] = self.location;
        }
        self.location = EntityLocation {
            archetype_id: new_archetype_id,
            index: new_index,
        };
        self.world.entities[self.entity] = self.location;
    }
}

impl World {
    /// Returns a shared reference ([`EntityRef`]) to the entity with the given
    /// id.
    pub fn entity(&self, entity: Entity) -> Option<EntityRef<'_>> {
        self.entities.get(entity).map(|location| EntityRef {
            world: self,
            entity,
            location: *location,
        })
    }

    /// Returns an exclusive reference ([`EntityMut`]) to the entity with the
    /// given id.
    pub fn entity_mut(&mut self, entity: Entity) -> Option<EntityMut<'_>> {
        if let Some(location) = self.entities.get_mut(entity) {
            let location = *location;
            Some(EntityMut::new(self, entity, location))
        } else {
            None
        }
    }

    /// Spawns/creates an new empty [`Entity`] in this `World` and returns a handle
    /// for modifying it.
    #[must_use]
    pub fn spawn(&mut self) -> EntityMut<'_> {
        let entity = self.entities.create();
        let empty_archetype = &mut self.archetypes[ArchetypeId::EMPTY];
        self.entities[entity].index = empty_archetype.len();
        empty_archetype.entities.push(entity);
        let location = self.entities[entity];
        EntityMut::new(self, entity, location)
    }
}
