//! Query execution: [`Query`] and its iterator.

use pulz_schedule::{
    prelude::Res,
    resource::{ResourceAccess, ResourceId, Resources, ResourcesSend},
    system::{SystemData, SystemDataSend},
};

use crate::{
    WorldInner,
    archetype::{Archetype, ArchetypeId, ArchetypeSet},
    entity::Entity,
    query::{
        data::{QueryData, QueryFetch, QueryFetchState, QueryFilter, QueryItem},
        state::QueryState,
    },
};

/// A system parameter that iterates entities matching a data type `Q` and filter `F`.
///
/// `Q` is typically a tuple of component references (`&T`, `&mut T`, `Entity`, `Option<&T>`).
/// `F` narrows which archetypes are visited (`With<T>`, `Without<T>`, or `()`).
pub struct Query<'w, Q, F = ()>
where
    Q: QueryData,
    F: QueryFilter,
{
    world: Res<'w, WorldInner>,
    state: Res<'w, QueryState<Q, F>>,
    fetch: Q::Fetch<'w>,
}

struct Cursor<'a> {
    iter: crate::archetype::ArchetypeSetIter<'a>,
    current_id: ArchetypeId,
    current_len: usize,
    current_index: usize,
}

impl<'a> Cursor<'a> {
    fn new(archetypes: &'a ArchetypeSet) -> Self {
        Self {
            iter: archetypes.iter(),
            current_id: ArchetypeId::EMPTY,
            current_len: 0,
            current_index: 0,
        }
    }

    fn next<'w>(&mut self, world: &'w WorldInner) -> Option<(&'w Archetype, usize)> {
        loop {
            if self.current_index < self.current_len {
                let archetype = &world.archetypes[self.current_id];
                let index = self.current_index;
                self.current_index += 1;
                return Some((archetype, index));
            }
            self.current_id = self.iter.next()?;
            let archetype = &world.archetypes[self.current_id];
            self.current_index = 0;
            self.current_len = archetype.len();
        }
    }
}

impl<'w, Q, F> Query<'w, Q, F>
where
    Q: QueryData + 'w,
    F: QueryFilter + 'w,
{
    /// Creates a new query, initializing its state in the given resources.
    pub fn new(res: &'w mut Resources) -> Self
    where
        Q: 'static,
        F: 'static,
    {
        let state_id = res.init::<QueryState<Q, F>>();
        Self::from_id(res, state_id)
    }

    fn from_id(res: &'w ResourcesSend, state_id: ResourceId<QueryState<Q, F>>) -> Self
    where
        Q: 'static,
        F: 'static,
    {
        let state = res
            .borrow_res_id(state_id)
            .expect("QueryState not initialized");
        let world = res
            .borrow_res_id(state.world_id)
            .expect("WorldInner not initialized");
        state.update_archetypes(&world);
        let fetch = Q::Fetch::fetch(res, &state.q_state);
        Self {
            world,
            state,
            fetch,
        }
    }

    /// Returns an iterator over all matching entities and their components.
    pub fn iter(&mut self) -> QueryIter<'_, 'w, Q, F> {
        let archetypes: *const ArchetypeSet = self.state.matching_archetypes();
        // SAFETY: self-reference to state which is pinned by the Res borrow
        let archetypes = unsafe { &*archetypes };
        QueryIter {
            query: self,
            cursor: Cursor::new(archetypes),
        }
    }

    /// Calls `f` for every matching entity.
    pub fn for_each<Func>(&mut self, mut f: Func)
    where
        Func: FnMut(QueryItem<'w, '_, Q>),
    {
        for item in self.iter() {
            f(item);
        }
    }

    /// Returns the query item for a specific entity, or `None` if it does not match.
    pub fn get(&mut self, entity: Entity) -> Option<QueryItem<'w, '_, Q>> {
        let location = self.world.entities.get(entity)?;
        let archetype = self.world.archetypes.get(location.archetype_id)?;
        if !self
            .state
            .matching_archetypes()
            .contains(location.archetype_id)
        {
            return None;
        }
        self.fetch.set_archetype(&self.state.q_state, archetype);
        Some(self.fetch.get(archetype, location.index()))
    }
}

/// An iterator yielding one [`QueryItem`] per matching entity.
///
/// Produced by [`Query::iter`].
pub struct QueryIter<'a, 'w, Q, F = ()>
where
    Q: QueryData,
    F: QueryFilter,
{
    query: &'a mut Query<'w, Q, F>,
    cursor: Cursor<'a>,
}

impl<'a, 'w, Q, F> Iterator for QueryIter<'a, 'w, Q, F>
where
    Q: QueryData + 'a,
    F: QueryFilter + 'a,
{
    type Item = QueryItem<'w, 'a, Q>;

    fn next(&mut self) -> Option<Self::Item> {
        let (archetype, index) = self.cursor.next(&self.query.world)?;
        if index == 0 {
            self.query
                .fetch
                .set_archetype(&self.query.state.q_state, archetype);
        }
        // SAFETY: reborrow from fetch; lifetime is 'a tied to &mut self
        let fetch: *mut Q::Fetch<'w> = &mut self.query.fetch;
        Some(unsafe { &mut *fetch }.get(archetype, index))
    }
}

// --- SystemData ---

pub struct QueryData_<Q: QueryData + 'static, F: QueryFilter + 'static>(
    ResourceId<QueryState<Q, F>>,
);

impl<Q, F> SystemData for Query<'_, Q, F>
where
    Q: QueryData + 'static,
    F: QueryFilter + 'static,
{
    type Data = QueryData_<Q, F>;
    type Arg<'a> = Query<'a, Q, F>;

    fn init(res: &mut Resources) -> Self::Data {
        QueryData_(res.init::<QueryState<Q, F>>())
    }

    fn update_access(res: &Resources, access: &mut ResourceAccess, data: &Self::Data) {
        let state = res
            .borrow_res_id(data.0)
            .expect("QueryState not initialized");
        access.add_shared(data.0);
        access.add_shared(state.world_id);
        state.q_state.update_access(access);
        state.f_state.update_access(access);
    }

    fn get<'a>(res: &'a Resources, data: &'a mut Self::Data) -> Self::Arg<'a> {
        Query::from_id(res, data.0)
    }
}

impl<Q, F> SystemDataSend for Query<'_, Q, F>
where
    Q: QueryData + 'static,
    F: QueryFilter + 'static,
{
    fn get_send<'a>(res: &'a ResourcesSend, data: &'a mut Self::Data) -> Self::Arg<'a> {
        Query::from_id(res, data.0)
    }
}
