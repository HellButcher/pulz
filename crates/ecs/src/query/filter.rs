use std::marker::PhantomData;

use pulz_schedule::resource::ResourceAccess;

use super::{QryRefState, QueryParamState, QueryParamWithFetch};
use crate::{
    archetype::Archetype,
    component::{Component, Components},
    query::{QueryParam, QueryParamFetch, QueryParamFetchGet},
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

    #[inline]
    fn update_access(state: &Self::State, access: &mut ResourceAccess) {
        // TODO: special handling for sparse filter components
        Q::update_access(&state.query, access);
    }
}

impl<'w, F, Q> QueryParamWithFetch<'w> for Without<F, Q>
where
    F: Filter,
    Q: QueryParamWithFetch<'w>,
{
    type Fetch = QryWithoutFilterFetch<F, Q::Fetch>;
}

#[doc(hidden)]
pub struct QryWithoutFilterState<F, Q> {
    filter: F,
    query: Q,
}

impl<F: QueryParamState, Q: QueryParamState> QueryParamState for QryWithoutFilterState<F, Q> {
    #[inline]
    fn init(resources: &Resources, components: &Components) -> Self {
        Self {
            filter: F::init(resources, components),
            query: Q::init(resources, components),
        }
    }

    #[inline]
    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        // TODO: special handling for sparse filter components
        !self.filter.matches_archetype(archetype) && self.query.matches_archetype(archetype)
    }
}

#[doc(hidden)]
pub struct QryWithoutFilterFetch<F, Q> {
    filter: PhantomData<fn(F)>,
    query: Q,
}

impl<'w, F, Q> QueryParamFetch<'w> for QryWithoutFilterFetch<F, Q>
where
    F: Filter,
    Q: QueryParamFetch<'w>,
{
    type State = QryWithoutFilterState<F::State, Q::State>;

    #[inline]
    fn fetch(res: &'w ResourcesSend, state: &Self::State) -> Self {
        Self {
            filter: PhantomData,
            query: Q::fetch(res, &state.query),
        }
    }

    #[inline(always)]
    fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype) {
        self.query.set_archetype(&state.query, archetype);
    }
}

impl<'w, 'a, F, Q> QueryParamFetchGet<'w, 'a> for QryWithoutFilterFetch<F, Q>
where
    F: Filter,
    Q: QueryParamFetchGet<'w, 'a>,
{
    type Item = Q::Item;

    #[inline(always)]
    fn get(&'a mut self, archetype: &Archetype, index: usize) -> Self::Item
    where
        'w: 'a,
    {
        self.query.get(archetype, index)
    }
}

pub struct With<F, Q>(PhantomData<(Q, fn(F))>);

impl<F, Q> QueryParam for With<F, Q>
where
    F: Filter,
    Q: QueryParam,
{
    type State = QryWithFilterState<F::State, Q::State>;

    #[inline]
    fn update_access(state: &Self::State, access: &mut ResourceAccess) {
        // TODO: special handling for sparce filter components
        Q::update_access(&state.query, access);
    }
}

impl<'w, F, Q> QueryParamWithFetch<'w> for With<F, Q>
where
    F: Filter,
    Q: QueryParamWithFetch<'w>,
{
    type Fetch = QryWithFilterFetch<F, Q::Fetch>;
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

#[doc(hidden)]
pub struct QryWithFilterFetch<F, Q> {
    filter: PhantomData<fn(F)>,
    query: Q,
}

impl<'w, F, Q> QueryParamFetch<'w> for QryWithFilterFetch<F, Q>
where
    F: Filter,
    Q: QueryParamFetch<'w>,
{
    type State = QryWithFilterState<F::State, Q::State>;

    #[inline(always)]
    fn fetch(res: &'w ResourcesSend, state: &Self::State) -> Self {
        Self {
            filter: PhantomData,
            query: Q::fetch(res, &state.query),
        }
    }

    #[inline(always)]
    fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype) {
        self.query.set_archetype(&state.query, archetype);
    }
}

impl<'w, 'a, F, Q> QueryParamFetchGet<'w, 'a> for QryWithFilterFetch<F, Q>
where
    F: Filter,
    Q: QueryParamFetchGet<'w, 'a>,
{
    type Item = Q::Item;

    #[inline(always)]
    fn get(&'a mut self, archetype: &Archetype, index: usize) -> Self::Item
    where
        'w: 'a,
    {
        self.query.get(archetype, index)
    }
}
