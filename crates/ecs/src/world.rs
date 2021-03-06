use std::mem::ManuallyDrop;

use crate::{
    archetype::Archetypes,
    component::{Component, ComponentId, Components},
    entity::{Entities, Entity},
    get_or_init_component,
    query::{Query, QueryPrepare},
    resource::{Res, Resources, TakenRes},
    WorldInner,
};

pub struct World<'a> {
    pub(crate) res: &'a Resources,
    pub(crate) world: Res<'a, WorldInner>,
}

pub struct WorldMut<'a> {
    pub(crate) res: &'a mut Resources,
    pub(crate) world: ManuallyDrop<TakenRes<WorldInner>>,
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
        if let Some(ent) = self.entity_mut(entity) {
            ent.despawn();
            true
        } else {
            false
        }
    }
}

impl Drop for WorldMut<'_> {
    fn drop(&mut self) {
        // SAFETY: only deconstructed here
        let world = unsafe { ManuallyDrop::take(&mut self.world) };
        self.res.insert_again(world);
    }
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

pub trait WorldExt {
    fn world(&self) -> World<'_>;
    fn world_mut(&mut self) -> WorldMut<'_>;

    fn query<Q>(&mut self) -> Query<'_, Q>
    where
        Q: QueryPrepare;
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
        Q: QueryPrepare,
    {
        Query::new(self)
    }
}
