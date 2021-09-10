use std::borrow::Cow;

use crate::{archetype::ArchetypeId, Entity, World};

use super::{PreparedQuery, Query, QueryFetch, QueryItem};

pub struct QueryExec<'w, Q>
where
    Q: Query<'w>,
{
    prepared: Cow<'w, PreparedQuery<Q>>,
    world: &'w World,
    borrow: Q::Borrow,
}

pub struct QueryIter<'w, 'a, Q>
where
    Q: Query<'w>,
{
    prepared: &'a PreparedQuery<Q>,
    world: &'w World,
    borrow: &'a mut Q::Borrow,
    archetype_id: Option<ArchetypeId>,
    state: Option<Q::State>,
    index: usize,
}

impl<'w, Q> QueryExec<'w, Q>
where
    Q: Query<'w>,
{
    pub fn new(world: &'w mut World) -> Self {
        // TODO: try to not require mut world
        let prepared = PreparedQuery::new(world);
        let tmp = prepared.prepared;
        Self {
            prepared: Cow::Owned(prepared),
            world,
            borrow: Q::borrow(world, tmp),
        }
    }

    pub(crate) fn new_prepared(prepared: &'w PreparedQuery<Q>, world: &'w World) -> Self {
        Self {
            prepared: Cow::Borrowed(prepared),
            world,
            borrow: Q::borrow(world, prepared.prepared),
        }
    }

    #[inline]
    pub fn iter<'a>(&'a mut self) -> QueryIter<'w, 'a, Q::Fetch>
    where
        Q: QueryFetch<'w, 'a>,
        'w: 'a,
    {
        let archetype_id = if self
            .prepared
            .matching_archetypes
            .contains(ArchetypeId::EMPTY)
        {
            Some(ArchetypeId::EMPTY)
        } else {
            self.prepared
                .matching_archetypes
                .find_next(ArchetypeId::EMPTY)
        };
        QueryIter {
            prepared: self.prepared.as_ref(),
            world: self.world,
            borrow: &mut self.borrow,
            archetype_id,
            state: None,
            index: 0,
        }
    }

    pub fn for_each<F>(&'w mut self, mut f: F)
    where
        for<'a> Q: QueryFetch<'w, 'a>,
        for<'a> F: FnMut(QueryItem<'w, 'a, Q>),
    {
        for item in self.iter() {
            f(item);
        }
    }

    pub fn get<'a>(&'a mut self, entity: Entity) -> Option<QueryItem<'w, 'a, Q>>
    where
        Q: QueryFetch<'w, 'a>,
    {
        let location = self.world.entities().get(entity)?;
        if !self
            .prepared
            .matching_archetypes
            .contains(location.archetype_id)
        {
            return None;
        }
        let archetype = &self.world.archetypes()[location.archetype_id];
        let state = Q::state(self.prepared.prepared, archetype);
        Some(Q::get(&mut self.borrow, state, archetype, location.index))
    }
}

impl<'w: 'a, 'a, Q> IntoIterator for &'a mut QueryExec<'w, Q>
where
    Q: QueryFetch<'w, 'a>,
{
    type Item = QueryItem<'w, 'a, Q>;
    type IntoIter = QueryIter<'w, 'a, Q::Fetch>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'w: 'a, 'a, F> Iterator for QueryIter<'w, 'a, F>
where
    F: QueryFetch<'w, 'a>,
{
    type Item = QueryItem<'w, 'a, F>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let borrow: *mut _ = &mut self.borrow;
        while let Some(archetype_id) = self.archetype_id {
            let archetype = &self.world.archetypes()[archetype_id];
            if self.index < archetype.len() {
                let index = self.index;
                self.index += 1;

                let state;
                if let Some(s) = self.state {
                    state = s;
                } else {
                    state = F::state(self.prepared.prepared, archetype);
                    self.state = Some(state);
                }

                // TODO: filter

                // TODO: SAFETY 🤷‍♂️ ???
                let item = F::get(unsafe { &mut *borrow }, state, archetype, index);
                return Some(item);
            }
            self.state = None;
            self.index = 0;
            self.archetype_id = self.prepared.matching_archetypes.find_next(archetype_id);
        }
        None
    }
}
