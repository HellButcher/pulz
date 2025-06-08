use std::pin::Pin;

use pulz_schedule::{
    resource::ResourcesSend,
    system::data::{SystemDataFetch, SystemDataFetchSend},
};

use super::QueryParamState;
use crate::{
    WorldInner,
    archetype::{Archetype, ArchetypeId, ArchetypeSet, ArchetypeSetIter},
    entity::Entity,
    query::{QueryItem, QueryParam, QueryParamFetch, QueryState},
    resource::{Res, ResourceAccess, ResourceId, Resources},
    system::data::SystemData,
};

pub struct Query<'w, Q>
where
    Q: QueryParam + 'w,
{
    world: Res<'w, WorldInner>,
    state: Res<'w, QueryState<Q::State>>,
    fetch: Q::Fetch<'w>,
}

pub struct QueryIter<'w, 'a, Q>
where
    Q: QueryParam + 'a,
{
    world: &'a WorldInner,
    state: &'a QueryState<Q::State>,
    fetch: &'a mut Q::Fetch<'w>,
    cursor: Cursor<'a>,
}

pub struct QueryIntoIter<'w, Q>
where
    Q: QueryParam + 'w,
{
    world: Pin<Res<'w, WorldInner>>,
    state: Pin<Res<'w, QueryState<Q::State>>>,
    fetch: Q::Fetch<'w>,
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
    Q: QueryParam + 'w,
{
    pub(crate) fn new(res: &'w mut Resources) -> Self {
        let state_resource_id = res.init::<QueryState<Q::State>>();
        Self::new_id(res, state_resource_id)
    }

    fn new_id(res: &'w ResourcesSend, resource_id: ResourceId<QueryState<Q::State>>) -> Self {
        let state = res.borrow_res_id(resource_id).expect("query-state");
        let world = res.borrow_res_id(state.world_resource_id).unwrap();
        state.update_archetypes(&world);
        let fetch = Q::Fetch::fetch(res, &state.param_state);
        Self {
            state,
            world,
            fetch,
        }
    }

    #[inline]
    pub fn iter<'a>(&'a mut self) -> QueryIter<'w, 'a, Q> {
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
        for<'a> F: FnMut(QueryItem<'w, 'a, Q>),
    {
        for item in self.iter() {
            f(item);
        }
    }

    pub fn get<'a>(&'a mut self, entity: Entity) -> Option<QueryItem<'w, 'a, Q>> {
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
    Q: QueryParam + 'a,
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
    Q: QueryParam + 'a,
{
    type Item = QueryItem<'w, 'a, Q>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let fetch: *mut _ = self.fetch;
        let fetch = unsafe { &mut *fetch }; // found no better way to deal with the lifetimes
        let (archetype, index) = self.cursor.next(self.world)?;
        if index == 0 {
            fetch.set_archetype(&self.state.param_state, archetype);
        }
        let item = fetch.get(archetype, index);
        Some(item)
    }
}

impl<'w, Q> Iterator for QueryIntoIter<'w, Q>
where
    Q: QueryParam + 'w,
{
    type Item = QueryItem<'w, 'w, Q>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let world: *const WorldInner = self.world.as_ref().get_ref();
        let world = unsafe { &*world }; // found no better way to deal with the lifetimes
        let fetch: *mut _ = &mut self.fetch;
        let fetch = unsafe { &mut *fetch }; // found no better way to deal with the lifetimes
        let (archetype, index) = self.cursor.next(world)?;
        if index == 0 {
            fetch.set_archetype(&self.state.param_state, archetype);
        }
        let item = fetch.get(archetype, index);
        Some(item)
    }
}

#[doc(hidden)]
pub struct QuerySystemParamData<S: QueryParamState>(ResourceId<QueryState<S>>);

#[doc(hidden)]
pub struct QuerySystemParamFetch<'r, S: QueryParamState>(
    &'r ResourcesSend,
    ResourceId<QueryState<S>>,
);

impl<Q> SystemData for Query<'_, Q>
where
    Q: QueryParam + 'static,
{
    type Data = QuerySystemParamData<Q::State>;
    type Fetch<'r> = QuerySystemParamFetch<'r, Q::State>;
    type Arg<'a> = Query<'a, Q>;

    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Arg<'a> {
        Query::new_id(fetch.0, fetch.1)
    }
}

impl<'r, S: QueryParamState> SystemDataFetch<'r> for QuerySystemParamFetch<'r, S> {
    type Data = QuerySystemParamData<S>;

    #[inline]
    fn init(res: &mut Resources) -> Self::Data {
        QuerySystemParamData(res.init::<QueryState<S>>())
    }

    #[inline]
    fn update_access(res: &Resources, access: &mut ResourceAccess, data: &Self::Data) {
        let state = res.borrow_res_id(data.0).unwrap();
        access.add_shared(data.0);
        access.add_shared(state.world_resource_id);
        state.param_state.update_access(access)
    }

    fn fetch(res: &'r Resources, data: &'r mut Self::Data) -> Self {
        Self(res, data.0)
    }
}

impl<'r, S: QueryParamState> SystemDataFetchSend<'r> for QuerySystemParamFetch<'r, S> {
    #[inline]
    fn fetch_send(res: &'r ResourcesSend, data: &'r mut Self::Data) -> Self {
        Self(res, data.0)
    }
}
