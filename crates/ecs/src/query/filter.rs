use std::marker::PhantomData;

use super::{QryRefState, QueryParamState};
use crate::{
    archetype::Archetype,
    component::{Component, ComponentSet, Components},
    query::{QueryFetch, QueryParam, QueryParamFetch},
    resource::{Resources, ResourcesSend},
};

pub trait Filter {
    type State: QueryParamState;
}

impl<T> Filter for &'_ T
where
    T: Component,
{
    type State = QryRefState<T>;
}

impl<T> Filter for &'_ mut T
where
    T: Component,
{
    type State = QryRefState<T>;
}

impl Filter for () {
    type State = ();
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
}

impl<$($name),+> Filter for Or<($($name,)+)>
where
    $($name: Filter,)+
{
    type State = ($($name::State,)+);
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
    type State = QryWithoutFilterState<F::State, Q::State>;
    type Fetch = Q::Fetch;
    type Borrow = Without<F, Q::Borrow>;

    #[inline]
    fn update_access(state: &Self::State, shared: &mut ComponentSet, exclusive: &mut ComponentSet) {
        // TODO: special handling for sparse filter components
        Q::update_access(&state.query, shared, exclusive);
    }

    #[inline(always)]
    fn fetch(state: &Self::State, archetype: &Archetype) -> Q::Fetch {
        Q::fetch(&state.query, archetype)
    }
}

#[doc(hidden)]
pub struct QryWithoutFilterState<F, S> {
    filter: F,
    query: S,
}

impl<F: QueryParamState, S: QueryParamState> QueryParamState for QryWithoutFilterState<F, S> {
    #[inline]
    fn init(resources: &Resources, components: &Components) -> Self {
        Self {
            filter: F::init(resources, components),
            query: S::init(resources, components),
        }
    }

    #[inline]
    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        // TODO: special handling for sparse filter components
        !self.filter.matches_archetype(archetype) && self.query.matches_archetype(archetype)
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
        Q::borrow(res, &state.query)
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
    type State = QryWithFilterState<F::State, Q::State>;
    type Fetch = Q::Fetch;
    type Borrow = With<F, Q::Borrow>;

    #[inline]
    fn update_access(state: &Self::State, shared: &mut ComponentSet, exclusive: &mut ComponentSet) {
        // TODO: special handling for sparce filter components
        Q::update_access(&state.query, shared, exclusive);
    }

    #[inline(always)]
    fn fetch(state: &Self::State, archetype: &Archetype) -> Q::Fetch {
        Q::fetch(&state.query, archetype)
    }
}

#[doc(hidden)]
pub struct QryWithFilterState<F, S> {
    filter: F,
    query: S,
}

impl<F: QueryParamState, S: QueryParamState> QueryParamState for QryWithFilterState<F, S> {
    #[inline]
    fn init(resources: &Resources, components: &Components) -> Self {
        Self {
            filter: F::init(resources, components),
            query: S::init(resources, components),
        }
    }

    #[inline]
    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        self.filter.matches_archetype(archetype) && self.query.matches_archetype(archetype)
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
        Q::borrow(res, &state.query)
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
