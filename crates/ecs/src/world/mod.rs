use crate::{
    archetype::Archetypes,
    component::{ComponentMap, Components},
    entity::{Entities, Entity},
    query::{exec::Query, QueryBorrow},
    resource::{FromWorld, ResourceId, Resources, SendMarker, UnsendMarker},
    storage::ComponentStorageMap,
};

mod entity_ref;

pub use atomic_refcell::{AtomicRef as Ref, AtomicRefMut as RefMut};
pub use entity_ref::{EntityMut, EntityRef};

pub type WorldSend = BaseWorld<SendMarker>;
pub type World = BaseWorld<UnsendMarker>;

pub struct BaseWorld<Marker = UnsendMarker> {
    resources: Resources<Marker>,
    components: Components,
    archetypes: Archetypes,
    entities: Entities,
    storage: ComponentStorageMap,

    // tracks removed components
    removed: ComponentMap<Vec<Entity>>,
}

impl<Marker> BaseWorld<Marker> {
    pub fn new() -> Self {
        Self {
            resources: Resources::new(),
            components: Components::new(),
            archetypes: Archetypes::new(),
            entities: Entities::new(),
            storage: ComponentStorageMap::new(),
            removed: ComponentMap::new(),
        }
    }

    #[inline]
    pub const fn resources(&self) -> &Resources<Marker> {
        &self.resources
    }

    #[inline]
    pub const fn components(&self) -> &Components {
        &self.components
    }

    #[inline]
    pub fn components_mut(&mut self) -> &mut Components {
        &mut self.components
    }

    #[inline]
    pub(crate) const fn archetypes(&self) -> &Archetypes {
        &self.archetypes
    }

    #[inline]
    pub(crate) const fn entities(&self) -> &Entities {
        &self.entities
    }

    #[inline]
    pub(crate) const fn storage(&self) -> &ComponentStorageMap {
        &self.storage
    }

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

    #[inline]
    pub fn query<'w, Q>(&'w mut self) -> Query<'w, Q>
    where
        Q: QueryBorrow<'w>,
    {
        Query::new(self.as_send_mut())
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

    #[inline]
    pub fn insert_resource<T>(&mut self, value: T) -> ResourceId<T>
    where
        T: 'static + Send + Sync,
    {
        self.resources.insert(value)
    }

    #[inline]
    pub fn as_send(&self) -> &WorldSend {
        // SAFETY: same type but different Phantom-Data.
        // Unsend -> Send is allowed, because it will restrict access-methods even more (to only accept send+sync types)
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn as_send_mut(&mut self) -> &mut WorldSend {
        // SAFETY: same type but different Phantom-Data.
        // Unsend -> Send is allowed, because it will restrict access-methods even more (to only accept send+sync types)
        unsafe { std::mem::transmute(self) }
    }

    /// # Unsafe
    /// User must ensure, that no UnSend Resources are send to a other thread
    #[inline]
    pub unsafe fn as_unsend(&self) -> &World {
        // SAFETY: same type but different Phantom-Data.
        // Send -> Unsend is unsafe (see doc)
        std::mem::transmute(self)
    }
}

impl World {
    pub fn init_resource<T>(&mut self) -> ResourceId<T>
    where
        T: 'static + Send + Sync + FromWorld,
    {
        if let Some(id) = self.resources.get_id::<T>() {
            id
        } else {
            let value = T::from_world(self);
            self.resources.insert(value)
        }
    }

    #[inline]
    pub fn insert_unsend_resource<T>(&mut self, value: T) -> ResourceId<T>
    where
        T: 'static,
    {
        self.resources.insert_unsend(value)
    }

    pub fn init_unsend_resource<T>(&mut self) -> ResourceId<T>
    where
        T: 'static + FromWorld,
    {
        if let Some(id) = self.resources.get_id::<T>() {
            id
        } else {
            let value = T::from_world(self);
            self.resources.insert_unsend(value)
        }
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
