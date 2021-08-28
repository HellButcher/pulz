use crate::{archetype::Archetype, entity::EntityLocation, World};

use super::{FetchGet, ItemFn, PreparedQuery, Query, QueryFetch};

pub struct One<'w, Q>
where
    Q: Query + QueryFetch<'w>,
{
    archetype: &'w Archetype,
    fetched: Q::Fetch,
    location: EntityLocation,
}

impl<'w, Q> One<'w, Q>
where
    Q: Query + QueryFetch<'w>,
{
    #[inline]
    pub(crate) fn new(
        prepared: &'w PreparedQuery<Q>,
        world: &'w World,
        location: EntityLocation,
    ) -> Self {
        let archetype = &world.archetypes[location.archetype_id];
        let fetched = Q::fetch(prepared.prepared, world);
        Self {
            archetype,
            fetched,
            location,
        }
    }

    #[inline]
    pub fn get(&mut self) -> <Q::Fetch as FetchGet<'_>>::Target {
        self.fetched.get(self.archetype, self.location.index)
    }

    #[inline]
    pub fn map<G, R>(&mut self, mut apply: G) -> R
    where
        G: for<'l> ItemFn<<Q::Fetch as FetchGet<'l>>::Target, R>,
    {
        let item = self.get();
        apply.call(item)
    }
}
