use std::marker::PhantomData;

use crate::{archetype::ArchetypeId, Entity, World};

use super::{FetchGet, ItemFn, PreparedQuery, Query, QueryFetch};

pub struct QueryExecution<'w, 'p, Q>
where
    Q: Query + QueryFetch<'w>,
{
    prepared: &'p PreparedQuery<Q>,
    world: &'w World,
    fetched: Q::Fetch,
}

impl<'w, 'p, Q> QueryExecution<'w, 'p, Q>
where
    Q: Query + QueryFetch<'w>,
{
    pub(crate) fn new(prepared: &'p PreparedQuery<Q>, world: &'w World) -> Self {
        Self {
            prepared,
            world,
            fetched: Q::fetch(prepared.prepared, world),
        }
    }

    pub fn get(&mut self, entity: Entity) -> Option<<Q::Fetch as FetchGet<'_>>::Target> {
        let location = self.world.entities.get(entity)?;
        if !self
            .prepared
            .matching_archetypes
            .contains(location.archetype_id)
        {
            return None;
        }
        let archetype = &self.world.archetypes[location.archetype_id];
        // TODO: filter?
        let item = self.fetched.get(archetype, location.index);
        Some(item)
    }

    pub fn get_with<G, R>(&mut self, entity: Entity, mut apply: G) -> Option<R>
    where
        G: for<'l> ItemFn<<Q::Fetch as FetchGet<'l>>::Target, R>,
    {
        let item = self.get(entity)?;
        Some(apply.call(item))
    }

    #[inline]
    pub fn for_each<G>(&mut self, mut apply: G)
    where
        G: for<'l> ItemFn<<Q::Fetch as FetchGet<'l>>::Target, ()>,
    {
        if self.prepared.sparse_only {
            unimplemented!();
        } else {
            let archetypes = &self.world.archetypes;
            for archetype_id in self.prepared.matching_archetypes.iter() {
                let archetype = &archetypes[archetype_id];
                for i in 0..archetype.len() {
                    // TODO: filter
                    apply.call(self.fetched.get(archetype, i));
                }
            }
        }
    }
}

impl<'w, 'p, Q> QueryExecution<'w, 'p, Q>
where
    Q: Query + QueryFetch<'w>,
{
    pub fn into_iter(self) -> QueryIter<'w, 'p, 'p, Q> {
        QueryIter::new(self)
    }
}

impl<'w: 'p, 'p, Q> IntoIterator for QueryExecution<'w, 'p, Q>
where
    Q: Query + QueryFetch<'w>,
{
    type Item = <Q::Fetch as FetchGet<'p>>::Target;
    type IntoIter = QueryIter<'w, 'p, 'p, Q>;
    fn into_iter(self) -> Self::IntoIter {
        QueryIter::new(self)
    }
}

pub struct QueryIter<'w, 'p, 'i, Q>
where
    Q: Query + QueryFetch<'w>,
{
    exec: QueryExecution<'w, 'p, Q>,
    archetype_id: Option<ArchetypeId>,
    index: usize,
    item: PhantomData<fn() -> <Q::Fetch as FetchGet<'i>>::Target>,
}

impl<'w, 'p, 'i, Q> QueryIter<'w, 'p, 'i, Q>
where
    Q: Query + QueryFetch<'w>,
{
    fn new(exec: QueryExecution<'w, 'p, Q>) -> Self {
        let archetype_id = if exec
            .prepared
            .matching_archetypes
            .contains(ArchetypeId::EMPTY)
        {
            Some(ArchetypeId::EMPTY)
        } else {
            exec.prepared
                .matching_archetypes
                .find_next(ArchetypeId::EMPTY)
        };
        Self {
            exec,
            archetype_id,
            index: 0,
            item: PhantomData,
        }
    }
}

impl<'w: 'i, 'p, 'i, Q> Iterator for QueryIter<'w, 'p, 'i, Q>
where
    Q: Query + QueryFetch<'w>,
    'w: 'p,
{
    type Item = <Q::Fetch as FetchGet<'i>>::Target;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let fetched: *mut _ = &mut self.exec.fetched;
        while let Some(archetype_id) = self.archetype_id {
            let archetype = &self.exec.world.archetypes[archetype_id];
            if self.index < archetype.len() {
                let index = self.index;
                self.index += 1;
                // TODO: filter
                // TODO: SAFETY???
                let item = FetchGet::<'i>::get(unsafe { &mut *fetched }, archetype, index);
                return Some(item);
            }

            self.index = 0;
            self.archetype_id = self
                .exec
                .prepared
                .matching_archetypes
                .find_next(archetype_id);
        }
        None
    }
}
