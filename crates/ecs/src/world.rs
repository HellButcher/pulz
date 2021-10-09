use std::mem::ManuallyDrop;

use crate::{
    component::ComponentId,
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
    pub fn init<T>(&mut self) -> ComponentId<T>
    where
        T: Send + Sync + 'static,
    {
        get_or_init_component(self.res, &mut self.world.components, false).1
    }

    #[inline]
    pub fn init_sparse<T>(&mut self) -> ComponentId<T>
    where
        T: Send + Sync + 'static,
    {
        get_or_init_component(self.res, &mut self.world.components, true).1
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
    pub fn clone(&self) -> Self {
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
