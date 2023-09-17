use std::{
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
};

use crate::{
    archetype::Archetypes,
    component::{Component, ComponentId, Components},
    entity::{Entities, Entity},
    get_or_init_component,
    query::{Query, QueryParam},
    resource::{RemovedResource, Res, Resources},
    WorldInner,
};

pub struct World<'a> {
    pub(crate) res: &'a Resources,
    pub(crate) world: Res<'a, WorldInner>,
}

impl World<'_> {
    #[inline]
    pub fn archetypes(&self) -> &Archetypes {
        &self.world.archetypes
    }

    #[inline]
    pub fn components(&self) -> &Components {
        &self.world.components
    }

    #[inline]
    pub fn entities(&self) -> &Entities {
        &self.world.entities
    }
}

impl Clone for World<'_> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            res: self.res,
            world: Res::clone(&self.world),
        }
    }
}

impl Deref for World<'_> {
    type Target = Resources;
    #[inline]
    fn deref(&self) -> &Resources {
        self.res
    }
}

pub struct WorldMut<'a> {
    pub(crate) res: &'a mut Resources,
    pub(crate) world: ManuallyDrop<RemovedResource<WorldInner>>,
}

impl WorldMut<'_> {
    #[inline]
    pub fn archetypes(&self) -> &Archetypes {
        &self.world.archetypes
    }

    #[inline]
    pub fn components(&self) -> &Components {
        &self.world.components
    }

    #[inline]
    pub fn entities(&self) -> &Entities {
        &self.world.entities
    }

    #[inline]
    pub fn init<T>(&mut self) -> ComponentId<T>
    where
        T: Component,
    {
        get_or_init_component::<T>(self.res, &mut self.world.components).1
    }

    /// Removes the entity and all its components from the world.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        let Some(ent) = self.entity_mut(entity) else {
            return false;
        };
        ent.despawn();
        true
    }
}

impl Drop for WorldMut<'_> {
    fn drop(&mut self) {
        // SAFETY: only deconstructed here
        let world = unsafe { ManuallyDrop::take(&mut self.world) };
        self.res.insert_again(world);
    }
}

impl Deref for WorldMut<'_> {
    type Target = Resources;
    #[inline]
    fn deref(&self) -> &Resources {
        self.res
    }
}

impl DerefMut for WorldMut<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Resources {
        self.res
    }
}

pub trait WorldExt {
    fn world(&self) -> World<'_>;
    fn world_mut(&mut self) -> WorldMut<'_>;

    fn query<Q>(&mut self) -> Query<'_, Q>
    where
        Q: QueryParam + 'static;
}

impl WorldExt for Resources {
    #[inline]
    fn world(&self) -> World<'_> {
        let world = self.borrow_res::<WorldInner>().expect("not initialized");
        World { res: self, world }
    }

    #[inline]
    fn world_mut(&mut self) -> WorldMut<'_> {
        let id = self.init::<WorldInner>();
        let world = self.remove_id(id).unwrap();
        WorldMut {
            res: self,
            world: ManuallyDrop::new(world),
        }
    }

    #[inline]
    fn query<Q>(&mut self) -> Query<'_, Q>
    where
        Q: QueryParam,
    {
        Query::new(self)
    }
}
