use crate::{
    archetype::Archetypes,
    component::{Components,ComponentMap},
    entity::{Entities, Entity},
    storage::WorldStorage,
};

mod entity_ref;

pub use entity_ref::{EntityMut, EntityRef};

pub struct World {
    components: Components,
    archetypes: Archetypes,
    entities: Entities,

    sparse_storage: ComponentMap<Box<dyn WorldStorage>>,

    // tracks removed components
    removed: ComponentMap<Vec<Entity>>,
}

impl World {
    pub fn new() -> Self {
        Self {
            components: Components::new(),
            archetypes: Archetypes::new(),
            entities: Entities::new(),
            sparse_storage: ComponentMap::new(),
            removed: ComponentMap::new(),
        }
    }

    #[inline]
    pub fn components(&self) -> &Components {
        &self.components
    }

    #[inline]
    pub fn components_mut(&mut self) -> &mut Components {
        &mut self.components
    }

    #[inline]
    pub(crate) fn archetypes(&self) -> &Archetypes {
        &self.archetypes
    }

    #[inline]
    pub(crate) fn entities(&self) -> &Entities {
        &self.entities
    }

    /// Returns `true` if the world contains the given entity.
    #[inline]
    pub fn contains_entity(&self, entity: Entity) -> bool {
        self.entities.contains(entity)
    }

    #[inline]
    pub fn get<T>(&self, entity: Entity) -> Option<&T> {
        self.entity(entity)?.get()
    }

    #[inline]
    pub fn get_mut<T>(&mut self, entity: Entity) -> Option<&mut T> {
        self.entity_mut(entity)?.get_mut()
    }

    /// Removes the entity and all its components from the world.
    ///
    /// Returns `false` if the world didn't contain the given entity.
    ///
    /// **See** [`EntityMut::despawn`]
    ///
    /// ## Example
    ///
    /// ```
    /// # use pulz_ecs::World;
    /// let mut world = World::new();
    /// let entity = world.spawn().id();
    /// assert!(world.contains_entity(entity));
    /// assert!(world.despawn(entity));
    /// assert!(!world.contains_entity(entity));
    /// assert!(!world.despawn(entity)); // now returns false
    /// ```
    #[inline]
    pub fn despawn(&mut self, entity: Entity) -> bool {
        self.entity_mut(entity).map_or(false, |e| {
            e.despawn();
            true
        })
    }
}

impl Default for World {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct A(usize);
    struct B(usize);
    struct C(usize);
    struct D(usize);
    struct E(usize);
    struct F(usize);
    struct G(usize);

    #[test]
    fn test_swawn() {
        let mut world = World::new();
        let mut entities = Vec::new();
        for i in 0..40000 {
            entities.push(world.spawn().insert(A(i)).insert(B(i)).insert(C(i)).id());
        }
        for (i, entity) in entities.iter().enumerate() {
            world
                .entity_mut(*entity)
                .unwrap()
                .insert(C(i))
                .insert(D(i))
                .insert(E(i));
        }
        for (i, entity) in entities.iter().enumerate() {
            world
                .entity_mut(*entity)
                .unwrap()
                .remove::<A>()
                .insert(F(i))
                .remove::<C>()
                .insert(G(i))
                .remove::<E>();
        }
    }
}
