use std::marker::PhantomData;

use crate::{
    archetype::Archetype,
    component::{Component, ComponentId, ComponentSet, Components},
    get_or_init_component,
    query::{QueryFetch, QueryParam, QueryParamFetch},
    resource::{Resources, ResourcesSend},
};

pub trait Filter {
    type State: Send + Sync + 'static;
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::State;

    /// Checks if the archetype matches the query
    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool;
}

impl<T> Filter for &'_ T
where
    T: Component,
{
    type State = ComponentId<T>;
    #[inline]
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::State {
        get_or_init_component::<T>(res, components).1
    }

    #[inline]
    fn matches_archetype(component_id: &ComponentId<T>, archetype: &Archetype) -> bool {
        archetype.contains_component_id(*component_id)
    }
}

impl<T> Filter for &'_ mut T
where
    T: Component,
{
    type State = ComponentId<T>;
    #[inline]
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::State {
        get_or_init_component::<T>(res, components).1
    }

    #[inline]
    fn matches_archetype(component_id: &ComponentId<T>, archetype: &Archetype) -> bool {
        archetype.contains_component_id(*component_id)
    }
}

impl Filter for () {
    type State = ();
    #[inline(always)]
    fn prepare(_res: &mut Resources, _components: &mut Components) {}

    #[inline(always)]
    fn matches_archetype(_prepared: &(), _archetype: &Archetype) -> bool {
        true
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct Or<T>(pub T);

macro_rules! tuple {
    () => ();
    ( $($name:ident.$index:tt,)+ ) => (

impl<$($name),+> Filter for ($($name,)+)
where
    $($name: Filter,)+
{
    type State = ($($name::State,)+);

    #[inline]
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::State {
        ($($name::prepare(res, components),)+)
    }

    #[inline(always)]
    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
        $($name::matches_archetype(&state.$index, archetype))&&+
    }
}

impl<$($name),+> Filter for Or<($($name,)+)>
where
    $($name: Filter,)+
{
    type State = ($($name::State,)+);

    #[inline]
    fn prepare(res: &mut Resources, components: &mut Components) -> Self::State {
        ($($name::prepare(res, components),)+)
    }

    #[inline(always)]
    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
        $($name::matches_archetype(&state.$index, archetype))||+
    }
}

    peel! { tuple [] $($name.$index,)+ }
)
}

tuple! { T0.0, T1.1, T2.2, T3.3, T4.4, T5.5, T6.6, T7.7, T8.8, T9.9, T10.10, T11.11, }

pub struct Without<F, Q>(PhantomData<(Q, fn(F))>);

impl<F, Q> QueryParam for Without<F, Q>
where
    F: Filter,
    Q: QueryParam,
{
    type State = (F::State, Q::State);
    type Fetch = Q::Fetch;
    type Borrow = Without<F, Q::Borrow>;

    #[inline]
    fn init(res: &mut Resources, components: &mut Components) -> Self::State {
        (F::prepare(res, components), Q::init(res, components))
    }

    #[inline]
    fn update_access(state: &Self::State, shared: &mut ComponentSet, exclusive: &mut ComponentSet) {
        Q::update_access(&state.1, shared, exclusive);
    }

    #[inline(always)]
    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
        !F::matches_archetype(&state.0, archetype) && Q::matches_archetype(&state.1, archetype)
    }

    #[inline(always)]
    fn fetch(state: &Self::State, archetype: &Archetype) -> Q::Fetch {
        Q::fetch(&state.1, archetype)
    }
}

impl<'w, F, Q> QueryParamFetch<'w> for Without<F, Q>
where
    F: Filter,
    Q: QueryParamFetch<'w>,
{
    type Borrowed = Q::Borrowed;
    type FetchGet = Without<F, Q::FetchGet>;

    #[inline(always)]
    fn borrow(res: &'w ResourcesSend, state: &Self::State) -> Self::Borrowed {
        Q::borrow(res, &state.1)
    }
}

impl<'w, 'a, F, Q> QueryFetch<'w, 'a> for Without<F, Q>
where
    F: Filter,
    Q: QueryFetch<'w, 'a>,
{
    type Item = Q::Item;

    #[inline(always)]
    fn get(
        this: &'a mut Self::Borrowed,
        fetch: Q::Fetch,
        archetype: &Archetype,
        index: usize,
    ) -> Self::Item
    where
        'w: 'a,
    {
        Q::get(this, fetch, archetype, index)
    }
}

pub struct With<F, Q>(PhantomData<(Q, fn(F))>);

impl<F, Q> QueryParam for With<F, Q>
where
    F: Filter,
    Q: QueryParam,
{
    type State = (F::State, Q::State);
    type Fetch = Q::Fetch;
    type Borrow = With<F, Q::Borrow>;

    #[inline]
    fn init(res: &mut Resources, components: &mut Components) -> Self::State {
        (F::prepare(res, components), Q::init(res, components))
    }

    #[inline]
    fn update_access(state: &Self::State, shared: &mut ComponentSet, exclusive: &mut ComponentSet) {
        Q::update_access(&state.1, shared, exclusive);
    }

    #[inline(always)]
    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool {
        F::matches_archetype(&state.0, archetype) && Q::matches_archetype(&state.1, archetype)
    }

    #[inline(always)]
    fn fetch(state: &Self::State, archetype: &Archetype) -> Q::Fetch {
        Q::fetch(&state.1, archetype)
    }
}

impl<'w, F, Q> QueryParamFetch<'w> for With<F, Q>
where
    F: Filter,
    Q: QueryParamFetch<'w>,
{
    type Borrowed = Q::Borrowed;
    type FetchGet = With<F, Q::FetchGet>;

    #[inline(always)]
    fn borrow(res: &'w ResourcesSend, state: &Self::State) -> Self::Borrowed {
        Q::borrow(res, &state.1)
    }
}

impl<'w, 'a, F, Q> QueryFetch<'w, 'a> for With<F, Q>
where
    F: Filter,
    Q: QueryFetch<'w, 'a>,
{
    type Item = Q::Item;

    #[inline(always)]
    fn get(
        this: &'a mut Self::Borrowed,
        fetch: Q::Fetch,
        archetype: &Archetype,
        index: usize,
    ) -> Self::Item
    where
        'w: 'a,
    {
        Q::get(this, fetch, archetype, index)
    }
}
