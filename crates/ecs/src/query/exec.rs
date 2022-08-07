use pulz_schedule::{resource::ResourceId, system::param::SystemParamState};

use crate::{
    archetype::{ArchetypeId, ArchetypeSetIter},
    entity::Entity,
    query::{QueryFetch, QueryItem, QueryParam, QueryParamFetch, QueryState},
    resource::{Res, Resources},
    system::param::{SystemParam, SystemParamFetch},
    WorldInner,
};

pub struct Query<'w, Q>
where
    Q: QueryParam,
{
    state: Res<'w, QueryState<Q>>,
    world: Res<'w, WorldInner>,
    borrow: <Q::Borrow as QueryParamFetch<'w>>::Borrowed,
}

pub struct QueryIter<'w, 'a, Q>
where
    Q: QueryParamFetch<'w>,
{
    state: &'a QueryState<Q>,
    world: &'a WorldInner,
    borrow: &'a mut Q::Borrowed,
    fetch: Option<Q::Fetch>,
    matching_archetypes: ArchetypeSetIter<'a>,
    current_archetype_id: ArchetypeId,
    current_archetype_len: usize,
    current_archetype_index: usize,
}

impl<'w, Q> Query<'w, Q>
where
    Q: QueryParam + 'static,
{
    pub(crate) fn new(res: &'w mut Resources) -> Self {
        let state_resource_id = res.init::<QueryState<Q>>();
        Self::new_id(res, state_resource_id)
    }

    fn new_id(res: &'w Resources, resource_id: ResourceId<QueryState<Q>>) -> Self {
        let state = res.borrow_res_id(resource_id).expect("query-state");
        let world = res.borrow_res_id(state.world_resource_id).unwrap();
        state.update_archetypes(&world);
        let borrow = <Q::Borrow as QueryParamFetch<'w>>::borrow(res.as_send(), &state.param_state);
        Self {
            state,
            world,
            borrow,
        }
    }

    #[inline]
    pub fn iter<'a>(&'a mut self) -> QueryIter<'w, 'a, Q::FetchGet>
    where
        Q: QueryFetch<'w, 'a>,
        'w: 'a,
    {
        let state: &'a QueryState<Q> = &self.state;
        let matching_archetypes = self.state.matching_archetypes().iter();
        let world = &self.world;
        let borrow = &mut self.borrow;
        QueryIter {
            state,
            world,
            borrow,
            fetch: None,
            matching_archetypes,
            current_archetype_id: ArchetypeId::EMPTY,
            current_archetype_len: 0,
            current_archetype_index: 0,
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
            .state
            .matching_archetypes()
            .contains(location.archetype_id)
        {
            return None;
        }
        let archetype = &self.world.archetypes[location.archetype_id];
        let fetch = Q::fetch(&self.state.param_state, archetype);
        Some(Q::get(&mut self.borrow, fetch, archetype, location.index))
    }
}

impl<'w: 'a, 'a, Q> IntoIterator for &'a mut Query<'w, Q>
where
    Q: QueryFetch<'w, 'a> + 'static,
{
    type Item = QueryItem<'w, 'a, Q>;
    type IntoIter = QueryIter<'w, 'a, Q::FetchGet>;

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
        loop {
            if self.current_archetype_index < self.current_archetype_len {
                let fetch = self.fetch.unwrap();
                let archetype = &self.world.archetypes[self.current_archetype_id];
                let item = F::get(
                    unsafe { *borrow },
                    fetch,
                    archetype,
                    self.current_archetype_index,
                );
                self.current_archetype_index += 1;
                return Some(item);
            } else {
                // reached end, or initial state
                self.current_archetype_id = self.matching_archetypes.next()?;
                let archetype = &self.world.archetypes[self.current_archetype_id];
                self.current_archetype_index = 0;
                self.current_archetype_len = archetype.len();
                self.fetch = Some(F::fetch(&self.state.param_state, archetype));
            }
        }
    }
}

#[doc(hidden)]
pub struct FetchQuery<Q: QueryParam>(ResourceId<QueryState<Q>>);

unsafe impl<Q> SystemParam for Query<'_, Q>
where
    Q: QueryParam + 'static,
{
    type Fetch = FetchQuery<Q>;
}

unsafe impl<Q> SystemParamState for FetchQuery<Q>
where
    Q: QueryParam + 'static,
{
    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.init::<QueryState<Q>>())
    }
}

unsafe impl<'r, Q> SystemParamFetch<'r> for FetchQuery<Q>
where
    Q: QueryParam + 'static,
{
    type Item = Query<'r, Q>;
    #[inline]
    fn fetch(&'r mut self, resources: &'r Resources) -> Self::Item {
        Query::new_id(resources, self.0)
    }
}
