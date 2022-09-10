use std::pin::Pin;

use pulz_schedule::{
    resource::{ResourceAccess, ResourceId},
    system::param::SystemParamState,
};

use super::QueryParamWithFetch;
use crate::{
    archetype::{Archetype, ArchetypeId, ArchetypeSet, ArchetypeSetIter},
    entity::Entity,
    query::{QueryItem, QueryParam, QueryParamFetch, QueryParamFetchGet, QueryState},
    resource::{Res, Resources},
    system::param::{SystemParam, SystemParamFetch},
    WorldInner,
};

pub struct Query<'w, Q>
where
    Q: QueryParam,
{
    world: Res<'w, WorldInner>,
    state: Res<'w, QueryState<Q::State>>,
    fetch: <Q as QueryParamWithFetch<'w>>::Fetch,
}

pub struct QueryIter<'w, 'a, Q>
where
    Q: QueryParam,
{
    world: &'a WorldInner,
    state: &'a QueryState<Q::State>,
    fetch: &'a mut <Q as QueryParamWithFetch<'w>>::Fetch,
    cursor: Cursor<'a>,
}

pub struct QueryIntoIter<'w, Q>
where
    Q: QueryParam,
{
    world: Pin<Res<'w, WorldInner>>,
    state: Pin<Res<'w, QueryState<Q::State>>>,
    fetch: <Q as QueryParamWithFetch<'w>>::Fetch,
    cursor: Cursor<'w>,
}

struct Cursor<'a> {
    matching_archetypes: ArchetypeSetIter<'a>,
    current_archetype_id: ArchetypeId,
    current_archetype_len: usize,
    current_archetype_index: usize,
}

impl<'w, Q> Query<'w, Q>
where
    Q: QueryParam,
    <Q as QueryParamWithFetch<'w>>::Fetch: QueryParamFetch<'w, State = Q::State>,
{
    pub(crate) fn new(res: &'w mut Resources) -> Self {
        let state_resource_id = res.init::<QueryState<Q::State>>();
        Self::new_id(res, state_resource_id)
    }

    fn new_id(res: &'w Resources, resource_id: ResourceId<QueryState<Q::State>>) -> Self {
        let state = res.borrow_res_id(resource_id).expect("query-state");
        let world = res.borrow_res_id(state.world_resource_id).unwrap();
        state.update_archetypes(&world);
        let fetch = <Q::Fetch as QueryParamFetch<'w>>::fetch(res.as_send(), &state.param_state);
        Self {
            state,
            world,
            fetch,
        }
    }

    #[inline]
    pub fn iter<'a>(&'a mut self) -> QueryIter<'w, 'a, Q>
    where
        <Q as QueryParamWithFetch<'w>>::Fetch: QueryParamFetchGet<'w, 'a>,
    {
        let world = &self.world;
        let state = &self.state;
        let matching_archetypes: *const _ = state.matching_archetypes();
        let fetch = &mut self.fetch;
        QueryIter {
            world,
            state,
            fetch,
            // SAFETY: self reference to state
            cursor: Cursor::new(unsafe { &*matching_archetypes }),
        }
    }

    pub fn for_each<F>(&'w mut self, mut f: F)
    where
        for<'a> <Q as QueryParamWithFetch<'w>>::Fetch: QueryParamFetchGet<'w, 'a, State = Q::State>,
        for<'a> F: FnMut(QueryItem<'w, 'a, Q>),
    {
        for item in self.iter() {
            f(item);
        }
    }

    pub fn get<'a>(&'a mut self, entity: Entity) -> Option<QueryItem<'w, 'a, Q>>
    where
        <Q as QueryParamWithFetch<'w>>::Fetch: QueryParamFetchGet<'w, 'a, State = Q::State>,
    {
        let location = self.world.entities.get(entity)?;
        if !self
            .state
            .matching_archetypes()
            .contains(location.archetype_id)
        {
            return None;
        }
        let archetype = &self.world.archetypes[location.archetype_id];
        self.fetch.set_archetype(&self.state.param_state, archetype);
        let item = self.fetch.get(archetype, location.index);
        Some(item)
    }
}

impl<'a> Cursor<'a> {
    #[inline]
    fn new(matching_archetypes: &'a ArchetypeSet) -> Self {
        Self {
            matching_archetypes: matching_archetypes.iter(),
            current_archetype_id: ArchetypeId::EMPTY,
            current_archetype_len: 0,
            current_archetype_index: 0,
        }
    }

    fn next(&mut self, world: &'a WorldInner) -> Option<(&'a Archetype, usize)> {
        loop {
            if self.current_archetype_index < self.current_archetype_len {
                let archetype = &world.archetypes[self.current_archetype_id];
                let archetype_index = self.current_archetype_index;
                self.current_archetype_index += 1;
                return Some((archetype, archetype_index));
            } else {
                // reached end, or initial state
                self.current_archetype_id = self.matching_archetypes.next()?;
                let archetype = &world.archetypes[self.current_archetype_id];
                self.current_archetype_index = 0;
                self.current_archetype_len = archetype.len();
            }
        }
    }
}

impl<'w: 'a, 'a, Q> IntoIterator for &'a mut Query<'w, Q>
where
    Q: QueryParam,
    <Q as QueryParamWithFetch<'w>>::Fetch: QueryParamFetchGet<'w, 'a, State = Q::State> + 'static,
{
    type Item = QueryItem<'w, 'a, Q>;
    type IntoIter = QueryIter<'w, 'a, Q>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'w, Q> IntoIterator for Query<'w, Q>
where
    Q: QueryParam + 'w,
    for<'a> <Q as QueryParamWithFetch<'w>>::Fetch:
        QueryParamFetchGet<'w, 'a, State = Q::State> + 'static,
{
    type Item = QueryItem<'w, 'w, Q>;
    type IntoIter = QueryIntoIter<'w, Q>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        let Self {
            world,
            state,
            fetch,
        } = self;
        let world = unsafe { Pin::new_unchecked(world) };
        let state = unsafe { Pin::new_unchecked(state) };
        let matching_archetypes: *const _ = state.matching_archetypes();
        QueryIntoIter {
            world,
            state,
            fetch,
            // safety: self-referenceto state; state is pinned
            cursor: Cursor::new(unsafe { &*matching_archetypes }),
        }
    }
}

impl<'w: 'a, 'a, Q> Iterator for QueryIter<'w, 'a, Q>
where
    Q: QueryParam,
    <Q as QueryParamWithFetch<'w>>::Fetch: QueryParamFetchGet<'w, 'a, State = Q::State>,
{
    type Item = QueryItem<'w, 'a, Q>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let fetch: *mut _ = self.fetch;
        let fetch = unsafe { &mut *fetch }; // found no better way to deal with the lifetimes
        if let Some((archetype, index)) = self.cursor.next(self.world) {
            if index == 0 {
                fetch.set_archetype(&self.state.param_state, archetype);
            }
            let item = fetch.get(archetype, index);
            Some(item)
        } else {
            None
        }
    }
}

impl<'w, Q> Iterator for QueryIntoIter<'w, Q>
where
    Q: QueryParam + 'w,
    <Q as QueryParamWithFetch<'w>>::Fetch: QueryParamFetchGet<'w, 'w, State = Q::State>,
{
    type Item = QueryItem<'w, 'w, Q>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let world: *const WorldInner = self.world.as_ref().get_ref();
        let world = unsafe { &*world }; // found no better way to deal with the lifetimes
        let fetch: *mut _ = &mut self.fetch;
        let fetch = unsafe { &mut *fetch }; // found no better way to deal with the lifetimes
        if let Some((archetype, index)) = self.cursor.next(world) {
            if index == 0 {
                fetch.set_archetype(&self.state.param_state, archetype);
            }
            let item = fetch.get(archetype, index);
            Some(item)
        } else {
            None
        }
    }
}

#[doc(hidden)]
pub struct FetchQuery<Q: QueryParam>(ResourceId<QueryState<Q::State>>);

unsafe impl<Q> SystemParam for Query<'_, Q>
where
    Q: QueryParam + 'static,
    for<'w> <Q as QueryParamWithFetch<'w>>::Fetch: QueryParamFetch<'w, State = Q::State>,
{
    type Fetch = FetchQuery<Q>;
}

unsafe impl<Q> SystemParamState for FetchQuery<Q>
where
    Q: QueryParam + 'static,
{
    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.init::<QueryState<Q::State>>())
    }

    #[inline]
    fn update_access(&self, resources: &Resources, access: &mut ResourceAccess) {
        use super::QueryParamState;
        let state = resources.borrow_res_id(self.0).unwrap();
        access.add_shared(self.0);
        access.add_shared(state.world_resource_id);
        state.param_state.update_access(access)
    }
}

unsafe impl<'r, Q> SystemParamFetch<'r> for FetchQuery<Q>
where
    Q: QueryParam + 'static,
    for<'w> <Q as QueryParamWithFetch<'w>>::Fetch: QueryParamFetch<'w, State = Q::State>,
{
    type Item = Query<'r, Q>;
    #[inline]
    fn fetch(&'r mut self, resources: &'r Resources) -> Self::Item {
        Query::new_id(resources, self.0)
    }
}
