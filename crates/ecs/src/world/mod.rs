use crate::{
    archetype::Archetypes,
    component::{ComponentMap, Components},
    entity::{Entities, Entity},
    storage::ComponentStorageMap,
};

mod entity_ref;

pub use entity_ref::{EntityMut, EntityRef};

pub struct World {
    pub(crate) components: Components,
    pub(crate) archetypes: Archetypes,
    pub(crate) entities: Entities,
    pub(crate) storage: ComponentStorageMap,

    // tracks removed components
    removed: ComponentMap<Vec<Entity>>,
}

impl World {
    pub fn new() -> Self {
        Self {
            components: Components::new(),
            archetypes: Archetypes::new(),
            entities: Entities::new(),
            storage: ComponentStorageMap::new(),
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

    // #[inline]
    // pub(crate) fn archetypes(&self) -> &Archetypes {
    //     &self.archetypes
    // }

    // #[inline]
    // pub(crate) fn entities(&self) -> &Entities {
    //     &self.entities
    // }

    /// Returns `true` if the world contains the given entity.
    #[inline]
    pub fn contains_entity(&self, entity: Entity) -> bool {
        self.entities.contains(entity)
    }

    #[inline]
    pub fn contains<T>(&self, entity: Entity) -> bool
    where
        T: 'static,
    {
        if let Some(e) = self.entity(entity) {
            e.contains::<T>()
        } else {
            false
        }
    }

    #[inline]
    pub fn get<T>(&self, entity: Entity) -> Option<T>
    where
        T: Copy + Clone + 'static,
    {
        self.entity(entity)?.borrow::<T>().map(|v| *v)
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

    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    struct A(usize);
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    struct B(usize);
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    struct C(usize);
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    struct D(usize);
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    struct E(usize);
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    struct F(usize);
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
                .insert(C(i * 2))
                .insert(D(i))
                .insert(E(i));
        }
        for (i, entity) in entities.iter().enumerate() {
            assert_eq!(Some(A(i)), world.get::<A>(*entity));
            assert_eq!(Some(B(i)), world.get::<B>(*entity));
            assert_eq!(Some(C(i * 2)), world.get::<C>(*entity));
            assert_eq!(Some(D(i)), world.get::<D>(*entity));
            assert_eq!(Some(E(i)), world.get::<E>(*entity));
            assert_eq!(None, world.get::<F>(*entity));
            assert_eq!(None, world.get::<G>(*entity));
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
        for (i, entity) in entities.iter().enumerate() {
            assert_eq!(None, world.get::<A>(*entity));
            assert_eq!(Some(B(i)), world.get::<B>(*entity));
            assert_eq!(None, world.get::<C>(*entity));
            assert_eq!(Some(D(i)), world.get::<D>(*entity));
            assert_eq!(None, world.get::<E>(*entity));
            assert_eq!(Some(F(i)), world.get::<F>(*entity));
            assert_eq!(Some(G(i)), world.get::<G>(*entity));
        }

        let mut i = 0usize;
        while i < entities.len() {
            let entity = entities[i];
            assert_eq!(None, world.get::<A>(entity));
            assert_eq!(Some(G(i)), world.get::<G>(entity));
            world.entity_mut(entity).unwrap().clear();
            i += 100;
        }
        let mut i = 0;
        while i < entities.len() {
            let entity = entities[i];
            assert!(!world.contains::<G>(entity));
            world.entity_mut(entity).unwrap().insert(A(i));
            i += 100;
        }
    }
}
