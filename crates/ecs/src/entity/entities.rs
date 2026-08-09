//! Entity identifier and the entity slot-map that tracks their archetype locations.

use slotmap::SlotMap;

use crate::entity::EntityLocation;

slotmap::new_key_type! {
    /// A lightweight, versioned handle to an entity.
    ///
    /// The version field prevents use-after-free: once an entity is removed its slot's
    /// version is incremented, making old handles invalid.
    pub struct Entity;
}

/// Manages the set of all live entities and their [`EntityLocation`]s.
pub struct Entities(SlotMap<Entity, EntityLocation>);

impl Default for Entities {
    fn default() -> Self {
        Self::new()
    }
}

impl Entities {
    pub(crate) fn new() -> Self {
        Self(SlotMap::with_key())
    }

    /// Removes all entities, resetting the registry to an empty state.
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Pre-allocates slot capacity for at least `additional_capacity` more entities.
    pub fn reserve(&mut self, additional_capacity: usize) {
        self.0.reserve(additional_capacity);
    }

    /// Allocates and returns a new live entity with a vacant location.
    pub fn create(&mut self) -> Entity {
        self.0.insert(EntityLocation::VACANT)
    }

    /// Removes the entity and returns its last location, or `None` if the handle is stale.
    pub fn remove(&mut self, entity: Entity) -> Option<EntityLocation> {
        self.0.remove(entity)
    }

    /// Returns `true` if the entity handle is still live (not removed or stale).
    pub fn contains(&self, entity: Entity) -> bool {
        self.0.contains_key(entity)
    }

    /// Returns the entity's current archetype location, or `None` if stale.
    pub fn get(&self, entity: Entity) -> Option<EntityLocation> {
        self.0.get(entity).copied()
    }

    /// Returns a mutable reference to the entity's location, or `None` if stale.
    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut EntityLocation> {
        self.0.get_mut(entity)
    }

    /// Returns the number of live (non-removed) entities.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if no entities are currently alive.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::ops::Index<Entity> for Entities {
    type Output = EntityLocation;
    #[inline]
    fn index(&self, entity: Entity) -> &EntityLocation {
        &self.0[entity]
    }
}

impl std::ops::IndexMut<Entity> for Entities {
    #[inline]
    fn index_mut(&mut self, entity: Entity) -> &mut EntityLocation {
        &mut self.0[entity]
    }
}
