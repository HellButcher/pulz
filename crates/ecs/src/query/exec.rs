use std::borrow::Cow;

use crate::{
    archetype::ArchetypeId,
    entity::Entity,
    query::{PreparedQuery, QueryBorrow, QueryFetch, QueryItem, QueryPrepare},
    resource::{Res, Resources},
    system::param::{SystemParam, SystemParamFetch},
    WorldInner,
};

pub struct Query<'w, Q>
where
    Q: QueryPrepare,
{
    prepared: Cow<'w, PreparedQuery<Q>>,
    world: Res<'w, WorldInner>,
    borrow: <Q::Borrow as QueryBorrow<'w>>::Borrowed,
}

pub struct QueryIter<'w, 'a, Q>
where
    Q: QueryBorrow<'w>,
{
    prepared: &'a PreparedQuery<Q>,
    world: &'a WorldInner,
    borrow: &'a mut Q::Borrowed,
    archetype_id: Option<ArchetypeId>,
    state: Option<Q::State>,
    index: usize,
}

impl<'w, Q> Query<'w, Q>
where
    Q: QueryPrepare,
{
    pub(crate) fn new(res: &'w mut Resources) -> Self {
        let prepared = PreparedQuery::new(res);
        let world = res.borrow_res_id(prepared.resource_id).unwrap();
        let tmp = prepared.prepared;
        let borrow = <Q::Borrow as QueryBorrow<'w>>::borrow(res.as_send(), tmp);
        Self {
            prepared: Cow::Owned(prepared),
            world,
            borrow,
        }
    }
    pub(crate) fn new_prepared(prepared: &'w PreparedQuery<Q>, res: &'w Resources) -> Self {
        let tmp = prepared.prepared;
        let world = res.borrow_res_id(prepared.resource_id).unwrap();
        let borrow = <Q::Borrow as QueryBorrow<'w>>::borrow(res.as_send(), tmp);
        Self {
            prepared: Cow::Borrowed(prepared),
            world,
            borrow,
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
            world: &self.world,
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
        let location = self.world.entities.get(entity)?;
        if !self
            .prepared
            .matching_archetypes
            .contains(location.archetype_id)
        {
            return None;
        }
        let archetype = &self.world.archetypes[location.archetype_id];
        let state = Q::state(self.prepared.prepared, archetype);
        Some(Q::get(&mut self.borrow, state, archetype, location.index))
    }
}

impl<'w: 'a, 'a, Q> IntoIterator for &'a mut Query<'w, Q>
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
            let archetype = &self.world.archetypes[archetype_id];
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
                let item = F::get(unsafe { *borrow }, state, archetype, index);
                return Some(item);
            }
            self.state = None;
            self.index = 0;
            self.archetype_id = self.prepared.matching_archetypes.find_next(archetype_id);
        }
        None
    }
}

unsafe impl<Q> SystemParam for Query<'_, Q>
where
    Q: QueryPrepare + 'static,
{
    const IS_SEND: bool = true;
    type Prepared = PreparedQuery<Q>;
    type Fetch = Self;
    fn prepare(resources: &mut Resources) -> Self::Prepared {
        PreparedQuery::<Q>::new(resources)
    }
}

impl<'w, 'a, Q> SystemParamFetch<'a> for Query<'w, Q>
where
    Q: QueryPrepare + 'static,
{
    type Output = Query<'a, Q>;
    fn get(prepared: &'a mut Self::Prepared, resources: &'a Resources) -> Self::Output
    where
        Q: 'a,
    {
        prepared.query(resources)
    }
}
