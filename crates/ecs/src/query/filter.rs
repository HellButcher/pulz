use std::marker::PhantomData;

use pulz_schedule::resource::ResourceAccess;

use crate::{
    archetype::Archetype,
    component::{Component, Components},
    query::{QryRefState, QueryParam, QueryParamFetch, QueryParamState},
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

impl Filter for () {
    type State = ();
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct Or<T>(pub T);

macro_rules! impl_filter_param {
    ([]) => ();
    ([$(($name:ident,$index:tt)),+]) => (

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
    )
}

pulz_functional_utils::generate_variadic_array! {[T,#] impl_filter_param!{}}

pub struct Without<F, Q>(PhantomData<fn(Q, F)>);

impl<F, Q> QueryParam for Without<F, Q>
where
    F: Filter,
    Q: QueryParam,
{
    type State = QryWithoutFilterState<F::State, Q::State>;
    type Fetch<'w> = QryWithoutFilterFetch<F, Q::Fetch<'w>>;
}

#[doc(hidden)]
pub struct QryWithoutFilterState<F, Q> {
    filter: F,
    query: Q,
}

unsafe impl<F: QueryParamState, Q: QueryParamState> QueryParamState
    for QryWithoutFilterState<F, Q>
{
    #[inline]
    fn init(resources: &Resources, components: &Components) -> Self {
        Self {
            filter: F::init(resources, components),
            query: Q::init(resources, components),
        }
    }

    #[inline]
    fn update_access(&self, access: &mut ResourceAccess) {
        // TODO: special handling for sparse filter components
        self.query.update_access(access);
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
    type Item<'a>
        = Q::Item<'a>
    where
        Self: 'a;

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

    #[inline(always)]
    fn get(&mut self, archetype: &Archetype, index: usize) -> Self::Item<'_> {
        self.query.get(archetype, index)
    }
}

pub struct With<F, Q>(PhantomData<fn(Q, F)>);

impl<F, Q> QueryParam for With<F, Q>
where
    F: Filter,
    Q: QueryParam,
{
    type State = QryWithFilterState<F::State, Q::State>;
    type Fetch<'w> = QryWithFilterFetch<F, Q::Fetch<'w>>;
}

#[doc(hidden)]
pub struct QryWithFilterState<F, S> {
    filter: F,
    query: S,
}

unsafe impl<F: QueryParamState, S: QueryParamState> QueryParamState for QryWithFilterState<F, S> {
    #[inline]
    fn init(resources: &Resources, components: &Components) -> Self {
        Self {
            filter: F::init(resources, components),
            query: S::init(resources, components),
        }
    }

    #[inline]
    fn update_access(&self, access: &mut ResourceAccess) {
        // TODO: special handling for sparce filter components
        self.query.update_access(access);
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
    type Item<'a>
        = Q::Item<'a>
    where
        Self: 'a;

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

    #[inline(always)]
    fn get(&mut self, archetype: &Archetype, index: usize) -> Self::Item<'_> {
        self.query.get(archetype, index)
    }
}
