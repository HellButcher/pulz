use pulz_schedule::{resource::ResourceId, system::param::SystemParamState};

use crate::{
    archetype::{ArchetypeId, ArchetypeSetIter},
    entity::Entity,
    query::{QueryBorrow, QueryFetch, QueryItem, QueryParam, QueryState},
    resource::{Res, Resources},
    system::param::{SystemParam, SystemParamFetch},
    WorldInner,
};

pub struct Query<'w, Q>
where
    Q: QueryParam,
{
    prepared: Res<'w, QueryState<Q>>,
    world: Res<'w, WorldInner>,
    borrow: <Q::Borrow as QueryBorrow<'w>>::Borrowed,
}

pub struct QueryIter<'w, 'a, Q>
where
    Q: QueryBorrow<'w>,
{
    prepared: &'a QueryState<Q>,
    world: &'a WorldInner,
    borrow: &'a mut Q::Borrowed,
    state: Option<Q::State>,
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
        let prepared = res.borrow_res_id(resource_id).expect("query-state");
        let world = res.borrow_res_id(prepared.resource_id).unwrap();
        prepared.update_archetypes(&world);
        let borrow = <Q::Borrow as QueryBorrow<'w>>::borrow(res.as_send(), prepared.prepared);
        Self {
            prepared,
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
        let prepared: &'a QueryState<Q> = &self.prepared;
        let matching_archetypes = self.prepared.matching_archetypes().iter();
        let world = &self.world;
        let borrow = &mut self.borrow;
        QueryIter {
            prepared,
            world,
            borrow,
            state: None,
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
            .prepared
            .matching_archetypes()
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
    Q: QueryFetch<'w, 'a> + 'static,
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
        loop {
            if self.current_archetype_index < self.current_archetype_len {
                let state = self.state.unwrap();
                let archetype = &self.world.archetypes[self.current_archetype_id];
                let item = F::get(
                    unsafe { *borrow },
                    state,
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
                self.state = Some(F::state(self.prepared.prepared, archetype));
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
