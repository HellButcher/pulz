use std::any::Any;

use crate::{
    archetype::Archetype,
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
            if let Some(storage) = self.world.sparse_storage.get(id) {
                return storage.contains(self.entity);
            }
            false
        } else {
            self.archetype().dense_storage.contains(id)
        }
    }

    /// Returns a shared reference to the given component of this entity.
    pub fn get<T>(&self) -> Option<&'w T> {
        unimplemented!() // TODO:
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
        if let Some(id) = self.world.components.get_id::<T>() {
            self.contains_id(id)
        } else {
            false
        }
    }

    pub fn contains_id(&self, id: ComponentId) -> bool {
        if self.remove_components.contains(id) {
            return false;
        } else if self.insert_components.contains(id) {
            return true;
        }
        if id.is_sparse() {
            if let Some(storage) = self.world.sparse_storage.get(id) {
                return storage.contains(self.entity);
            }
            false
        } else {
            self.archetype().dense_storage.contains(id)
        }
    }

    /// Returns a shared reference to the given component of this entity.
    pub fn get<T>(&self) -> Option<&'w T> {
        unimplemented!() // TODO:
    }

    /// Returns an exclusive reference to the given component of this entity.
    pub fn get_mut<T>(&mut self) -> Option<&'w mut T> {
        unimplemented!() // TODO:
    }

    #[inline]
    pub fn insert<T>(&mut self, value: T) -> &mut Self
    where
        T: 'static,
    {
        let id = self.world.components.get_or_insert_id::<T>();
        self.insert_id(id, value)
    }

    pub fn insert_id<T>(&mut self, id: ComponentId, value: T) -> &mut Self
    where
        T: 'static,
    {
        self.remove_components.remove(id);
        self.insert_components.insert(id, Box::new(Some(value)));
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

    pub fn remove_id(&mut self, id: ComponentId) -> &mut Self {
        self.insert_components.remove(id);
        self.remove_components.insert(id);
        self
    }

    pub fn clear(&mut self) -> &mut Self {
        // clear open operations
        self.insert_components.clear();

        // mark all components for removal
        let archetype = &self.world.archetypes[self.location.archetype_id];
        for component_id in archetype.dense_storage.keys() {
            self.remove_components.insert(component_id);
        }
        for (component_id, storage) in self.world.sparse_storage.entries() {
            if storage.contains(self.entity) {
                self.remove_components.insert(*component_id);
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
        for (component_id, storage) in archetype.dense_storage.entries_mut() {
            // remove
            storage.swap_remove(location.index);

            // track
            track_removed
                .get_or_insert_default(component_id)
                .push(self.entity);
        }

        // remove sparse components and track removal
        for (component_id, storage) in self.world.sparse_storage.entries_mut() {
            if storage.remove(self.entity) {
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
                    if storage.remove(self.entity) {
                        // track removal
                        self.world
                            .removed
                            .get_or_insert_default(id)
                            .push(self.entity);
                    }
                }
            } else if archetype.dense_storage.contains(id) {
                // remember removal of dense components
                remove_dense.push(id);
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
                    .get_or_insert_with(id, || components.new_world_storage(id))
                    .as_mut();
                if !storage.insert_impl(self.entity, box_value.as_mut()) {
                    panic!(
                        "unexpected type {:?} != {:?} of sparse component with id {:?}",
                        box_value.type_id(),
                        storage.typeid(),
                        id
                    );
                }
            } else if let Some(storage) = archetype.dense_storage.get_mut(id) {
                // update existing dense components
                if !storage.replace_impl(old_index, box_value.as_mut()) {
                    panic!(
                        "unexpected type {:?} != {:?} of dense component with id {:?}",
                        box_value.type_id(),
                        storage.typeid(),
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

        let mut new_components = archetype.dense_storage.key_set();
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

        // copy old
        for (id, old_storage) in &mut old_archetype.dense_storage.entries_mut() {
            if let Some(new_storage) = new_archetype.dense_storage.get_mut(id) {
                let result = old_storage.swap_remove_and_push(old_index, new_storage.as_mut());
                assert_eq!(
                    Some(new_index),
                    result,
                    "unexpected type of storage {:?} != {:?} of dense component with id {:?}",
                    old_storage.typeid(),
                    new_storage.typeid(),
                    id
                );
            }
        }
        // push new
        for (id, mut box_value) in insert_dense {
            if let Some(storage) = new_archetype.dense_storage.get_mut(id) {
                let result = storage.push_impl(box_value.as_mut());
                assert_eq!(
                    Some(new_index),
                    result,
                    "unexpected type {:?} != {:?} of dense component with id {:?}",
                    box_value.type_id(),
                    storage.typeid(),
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
        let location = self.entities[entity];
        EntityMut::new(self, entity, location)
    }
}
