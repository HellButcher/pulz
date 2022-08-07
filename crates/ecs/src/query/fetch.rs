use crate::{
    archetype::Archetype,
    component::{Component, ComponentId, ComponentSet, Components},
    entity::Entity,
    get_or_init_component,
    query::{QueryFetch, QueryParam, QueryParamFetch},
    resource::{Res, ResMut, ResourceId, Resources, ResourcesSend},
    storage::Storage,
};

impl<T: Component> QueryParam for &'_ T {
    type State = (ResourceId<T::Storage>, ComponentId<T>);
    type Fetch = ();
    type Borrow = Self;

    #[inline]
    fn init(res: &mut Resources, components: &mut Components) -> Self::State {
        get_or_init_component::<T>(res, components)
    }

    #[inline]
    fn update_access(
        state: &Self::State,
        shared: &mut ComponentSet,
        _exclusive: &mut ComponentSet,
    ) {
        shared.insert(state.1);
    }

    #[inline]
    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
        state.1.is_sparse() || archetype.contains_component_id(state.1)
    }

    #[inline(always)]
    fn fetch(_prepared: &Self::State, _archetype: &Archetype) {}
}

impl<'w, T: Component> QueryParamFetch<'w> for &'_ T {
    type Borrowed = Res<'w, T::Storage>;
    type FetchGet = Self;

    #[inline]
    fn borrow(res: &'w ResourcesSend, state: &Self::State) -> Self::Borrowed {
        res.borrow_res_id(state.0)
            .expect("unable to borrow component")
    }
}

impl<'w, 'a, T: Component> QueryFetch<'w, 'a> for &'_ T {
    type Item = &'a T;

    #[inline]
    fn get(
        this: &'a mut Self::Borrowed,
        _state: (),
        archetype: &Archetype,
        index: usize,
    ) -> Self::Item {
        this.get(archetype.entities[index], archetype.id, index)
            .expect("unable to get component item")
    }
}

impl<T: Component> QueryParam for &'_ mut T {
    type State = (ResourceId<T::Storage>, ComponentId<T>);
    type Fetch = ();
    type Borrow = Self;

    #[inline]
    fn init(res: &mut Resources, components: &mut Components) -> Self::State {
        get_or_init_component::<T>(res, components)
    }

    #[inline]
    fn update_access(
        state: &Self::State,
        _shared: &mut ComponentSet,
        exclusive: &mut ComponentSet,
    ) {
        exclusive.insert(state.1);
    }

    #[inline]
    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
        state.1.is_sparse() || archetype.contains_component_id(state.1)
    }

    #[inline(always)]
    fn fetch(_prepared: &Self::State, _archetype: &Archetype) {}
}

impl<'w, T: Component> QueryParamFetch<'w> for &'_ mut T {
    type Borrowed = ResMut<'w, T::Storage>;
    type FetchGet = Self;

    #[inline]
    fn borrow(res: &'w ResourcesSend, state: &Self::State) -> Self::Borrowed {
        res.borrow_res_mut_id(state.0)
            .expect("unable to borrow mut component")
    }
}

impl<'w, 'a, T: Component> QueryFetch<'w, 'a> for &'_ mut T {
    type Item = &'a mut T;

    #[inline]
    fn get(
        this: &'a mut Self::Borrowed,
        _state: (),
        archetype: &Archetype,
        index: usize,
    ) -> Self::Item {
        this.get_mut(archetype.entities[index], archetype.id, index)
            .expect("unable to get component item")
    }
}

impl QueryParam for Entity {
    type State = ();
    type Fetch = ();
    type Borrow = Self;

    #[inline(always)]
    fn init(_res: &mut Resources, _components: &mut Components) {}

    #[inline(always)]
    fn update_access(_prepared: &(), _shared: &mut ComponentSet, _exclusive: &mut ComponentSet) {}

    #[inline(always)]
    fn matches_archetype(_prepared: &(), _archetype: &Archetype) -> bool {
        true
    }

    #[inline(always)]
    fn fetch(_prepared: &Self::State, _archetype: &Archetype) {}
}

impl QueryParamFetch<'_> for Entity {
    type Borrowed = ();
    type FetchGet = Self;

    #[inline(always)]
    fn borrow(_res: &ResourcesSend, _prepared: &()) {}
}

impl QueryFetch<'_, '_> for Entity {
    type Item = Self;

    #[inline(always)]
    fn get(_this: &mut Self::Borrowed, _state: (), archetype: &Archetype, index: usize) -> Self {
        archetype.entities[index]
    }
}

impl<Q> QueryParam for Option<Q>
where
    Q: QueryParam,
{
    type State = Q::State;
    type Fetch = (bool, Q::Fetch);
    type Borrow = Option<Q::Borrow>;

    #[inline]
    fn init(res: &mut Resources, components: &mut Components) -> Self::State {
        Q::init(res, components)
    }

    #[inline]
    fn update_access(state: &Self::State, shared: &mut ComponentSet, exclusive: &mut ComponentSet) {
        Q::update_access(state, shared, exclusive);
    }

    #[inline(always)]
    fn matches_archetype(_prepared: &Self::State, _archetype: &Archetype) -> bool {
        true
    }

    #[inline(always)]
    fn fetch(state: &Self::State, archetype: &Archetype) -> (bool, Q::Fetch) {
        (
            Q::matches_archetype(state, archetype),
            Q::fetch(state, archetype),
        )
    }
}

impl<'w, Q> QueryParamFetch<'w> for Option<Q>
where
    Q: QueryParamFetch<'w>,
{
    type Borrowed = Q::Borrowed;
    type FetchGet = Option<Q::FetchGet>;

    #[inline]
    fn borrow(res: &'w ResourcesSend, state: &Self::State) -> Self::Borrowed {
        Q::borrow(res, state)
    }
}

impl<'w, 'a, F> QueryFetch<'w, 'a> for Option<F>
where
    F: QueryFetch<'w, 'a>,
{
    type Item = Option<F::Item>;

    #[inline]
    fn get(
        this: &'a mut Self::Borrowed,
        fetch: (bool, F::Fetch),
        archetype: &Archetype,
        index: usize,
    ) -> Self::Item
    where
        'w: 'a,
    {
        let (available, sub_fetch) = fetch;
        if available {
            Some(F::get(this, sub_fetch, archetype, index))
        } else {
            None
        }
    }
}

impl QueryParam for () {
    type State = ();
    type Fetch = ();
    type Borrow = Self;

    #[inline(always)]
    fn init(_res: &mut Resources, _components: &mut Components) {}

    #[inline(always)]
    fn update_access(
        _prepared: &Self::State,
        _shared: &mut ComponentSet,
        _exclusive: &mut ComponentSet,
    ) {
    }

    #[inline(always)]
    fn matches_archetype(_prepared: &Self::State, _archetype: &Archetype) -> bool {
        true
    }

    #[inline(always)]
    fn fetch(_prepared: &Self::State, _archetype: &Archetype) {}
}

impl QueryParamFetch<'_> for () {
    type Borrowed = ();
    type FetchGet = ();

    #[inline(always)]
    fn borrow(_res: &ResourcesSend, _prepared: &Self::State) {}
}

impl QueryFetch<'_, '_> for () {
    type Item = ();

    #[inline(always)]
    fn get(_this: &mut Self::Borrowed, _state: (), _archetype: &Archetype, _index: usize) {}
}

macro_rules! tuple {
    () => ();
    ( $($name:ident.$index:tt,)+ ) => (

impl<$($name),+> QueryParam for ($($name,)+)
where
    $($name: QueryParam,)+
{
    type State = ($($name::State,)+);
    type Fetch = ($($name::Fetch,)+);
    type Borrow = ($($name::Borrow,)+);

    #[inline]
    fn init(res: &mut Resources, components: &mut Components) -> Self::State {
        ($($name::init(res, components),)+)
    }

    #[inline]
    fn update_access(
        state: &Self::State,
        shared: &mut ComponentSet,
        exclusive: &mut ComponentSet,
    ) {
        $($name::update_access(&state.$index, shared, exclusive);)+
    }

    #[inline]
    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
        $($name::matches_archetype(&state.$index, archetype))&&+
    }

    #[inline]
    fn fetch(state: &Self::State, archetype: &Archetype) -> Self::Fetch {
        ($($name::fetch(&state.$index, archetype),)+)
    }
}

impl<'w, $($name),+> QueryParamFetch<'w> for ($($name,)+)
where
    $($name: QueryParamFetch<'w>,)+
{
    type Borrowed = ($($name::Borrowed,)+);
    type FetchGet = ($($name::FetchGet,)+);

    #[inline]
    fn borrow(res: &'w ResourcesSend, state: &Self::State) -> Self::Borrowed {
        ($($name::borrow(res, &state.$index),)+)
    }
}

impl<'w, 'a, $($name),+> QueryFetch<'w, 'a> for ($($name,)+)
where
    $($name: QueryFetch<'w,'a>,)+
{
    type Item = ($($name::Item,)+);

    #[inline(always)]
    fn get(this: &'a mut Self::Borrowed, fetch: Self::Fetch, archetype: &Archetype, index: usize) -> Self::Item where 'w: 'a {
        ($($name::get(&mut this.$index, fetch.$index, archetype, index),)+)
    }
}


        peel! { tuple [] $($name.$index,)+ }
    )
}

tuple! { T0.0, T1.1, T2.2, T3.3, T4.4, T5.5, T6.6, T7.7, T8.8, T9.9, T10.10, T11.11, }
