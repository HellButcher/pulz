use super::{QryRefState, QueryParamState, QueryParamWithFetch};
use crate::{
    archetype::Archetype,
    component::{Component, ComponentSet, Components},
    entity::Entity,
    query::{QueryParam, QueryParamFetch, QueryParamFetchGet},
    resource::{Res, ResMut, Resources, ResourcesSend},
    storage::Storage,
};

impl<T: Component> QueryParam for &'_ T {
    type State = QryRefState<T>;

    #[inline]
    fn update_access(
        state: &Self::State,
        shared: &mut ComponentSet,
        _exclusive: &mut ComponentSet,
    ) {
        shared.insert(state.component_id);
    }
}

#[doc(hidden)]
#[repr(transparent)]
pub struct QryRefFetch<'w, T: Component>(Res<'w, T::Storage>);

impl<'w, T: Component> QueryParamWithFetch<'w> for &'_ T {
    type Fetch = QryRefFetch<'w, T>;
}

impl<'w, T: Component> QueryParamFetch<'w> for QryRefFetch<'w, T> {
    type State = QryRefState<T>;

    #[inline]
    fn fetch(res: &'w ResourcesSend, state: &QryRefState<T>) -> Self {
        Self(
            res.borrow_res_id(state.storage_id)
                .expect("unable to borrow component"),
        )
    }

    #[inline(always)]
    fn set_archetype(&mut self, _state: &Self::State, _archetype: &Archetype) {}
}

impl<'w, 'a, T: Component> QueryParamFetchGet<'w, 'a> for QryRefFetch<'w, T> {
    type Item = &'a T;

    #[inline]
    fn get(&'a mut self, archetype: &Archetype, index: usize) -> Self::Item {
        self.0
            .get(archetype.entities[index], archetype.id, index)
            .expect("unable to get component item")
    }
}

impl<T: Component> QueryParam for &'_ mut T {
    type State = QryRefState<T>;

    #[inline]
    fn update_access(
        state: &QryRefState<T>,
        _shared: &mut ComponentSet,
        exclusive: &mut ComponentSet,
    ) {
        exclusive.insert(state.component_id);
    }
}

impl<'w, T: Component> QueryParamWithFetch<'w> for &'_ mut T {
    type Fetch = QryRefMutFetch<'w, T>;
}

#[doc(hidden)]
#[repr(transparent)]
pub struct QryRefMutFetch<'w, T: Component>(ResMut<'w, T::Storage>);

impl<'w, T: Component> QueryParamFetch<'w> for QryRefMutFetch<'w, T> {
    type State = QryRefState<T>;

    #[inline]
    fn fetch(res: &'w ResourcesSend, state: &QryRefState<T>) -> Self {
        Self(
            res.borrow_res_mut_id(state.storage_id)
                .expect("unable to borrow mut component"),
        )
    }

    #[inline(always)]
    fn set_archetype(&mut self, _state: &Self::State, _archetype: &Archetype) {}
}

impl<'w, 'a, T: Component> QueryParamFetchGet<'w, 'a> for QryRefMutFetch<'w, T> {
    type Item = &'a mut T;

    #[inline]
    fn get(&'a mut self, archetype: &Archetype, index: usize) -> Self::Item {
        self.0
            .get_mut(archetype.entities[index], archetype.id, index)
            .expect("unable to get component item")
    }
}

impl QueryParam for Entity {
    type State = ();

    #[inline(always)]
    fn update_access(_state: &(), _shared: &mut ComponentSet, _exclusive: &mut ComponentSet) {}
}

impl QueryParamWithFetch<'_> for Entity {
    type Fetch = QryEntityFetch;
}

#[doc(hidden)]
pub struct QryEntityFetch;

impl QueryParamFetch<'_> for QryEntityFetch {
    type State = ();

    #[inline(always)]
    fn fetch(_res: &ResourcesSend, _state: &()) -> Self {
        QryEntityFetch
    }

    #[inline(always)]
    fn set_archetype(&mut self, _state: &Self::State, _archetype: &Archetype) {}
}

impl QueryParamFetchGet<'_, '_> for QryEntityFetch {
    type Item = Entity;

    #[inline]
    fn get(&mut self, archetype: &Archetype, index: usize) -> Self::Item {
        archetype.entities[index]
    }
}

impl<Q> QueryParam for Option<Q>
where
    Q: QueryParam,
{
    type State = QryOptionState<Q::State>;

    #[inline]
    fn update_access(state: &Self::State, shared: &mut ComponentSet, exclusive: &mut ComponentSet) {
        Q::update_access(&state.0, shared, exclusive);
    }
}

impl<'w, Q: QueryParamWithFetch<'w>> QueryParamWithFetch<'w> for Option<Q> {
    type Fetch = QryOptionFetch<Q::Fetch>;
}

#[doc(hidden)]
#[repr(transparent)]
pub struct QryOptionState<S>(S);

impl<S: QueryParamState> QueryParamState for QryOptionState<S> {
    #[inline]
    fn init(resources: &Resources, components: &Components) -> Self {
        Self(S::init(resources, components))
    }
    #[inline]
    fn matches_archetype(&self, _archetype: &Archetype) -> bool {
        true
    }
}

#[doc(hidden)]
pub struct QryOptionFetch<F> {
    available: bool,
    sub_fetch: F,
}

impl<'w, F> QueryParamFetch<'w> for QryOptionFetch<F>
where
    F: QueryParamFetch<'w>,
{
    type State = QryOptionState<F::State>;

    #[inline]
    fn fetch(res: &'w ResourcesSend, state: &Self::State) -> Self {
        Self {
            available: false,
            sub_fetch: F::fetch(res, &state.0),
        }
    }

    #[inline]
    fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype) {
        self.available = state.0.matches_archetype(archetype);
        self.sub_fetch.set_archetype(&state.0, archetype);
    }
}

impl<'w, 'a, F> QueryParamFetchGet<'w, 'a> for QryOptionFetch<F>
where
    F: QueryParamFetchGet<'w, 'a>,
{
    type Item = Option<F::Item>;

    #[inline]
    fn get(&'a mut self, archetype: &Archetype, index: usize) -> Self::Item
    where
        'w: 'a,
    {
        if self.available {
            Some(self.sub_fetch.get(archetype, index))
        } else {
            None
        }
    }
}

impl QueryParam for () {
    type State = ();

    #[inline(always)]
    fn update_access(
        _state: &Self::State,
        _shared: &mut ComponentSet,
        _exclusive: &mut ComponentSet,
    ) {
    }
}

impl QueryParamWithFetch<'_> for () {
    type Fetch = ();
}

impl QueryParamState for () {
    #[inline]
    fn init(_res: &Resources, _components: &Components) -> Self {}

    #[inline(always)]
    fn matches_archetype(&self, _archetype: &Archetype) -> bool {
        true
    }
}

impl QueryParamFetch<'_> for () {
    type State = ();

    #[inline(always)]
    fn fetch(_res: &ResourcesSend, _state: &Self::State) {}

    #[inline(always)]
    fn set_archetype(&mut self, _state: &Self::State, _archetype: &Archetype) {}
}

impl QueryParamFetchGet<'_, '_> for () {
    type Item = ();

    #[inline(always)]
    fn get(&mut self, _archetype: &Archetype, _index: usize) {}
}

macro_rules! tuple {
    () => ();
    ( $($name:ident.$index:tt,)+ ) => (

impl<$($name),+> QueryParam for ($($name,)+)
where
    $($name: QueryParam,)+
{
    type State = ($($name::State,)+);

    #[inline]
    fn update_access(
        state: &Self::State,
        shared: &mut ComponentSet,
        exclusive: &mut ComponentSet,
    ) {
        $($name::update_access(&state.$index, shared, exclusive);)+
    }
}

impl<'w, $($name),+> QueryParamWithFetch<'w> for ($($name,)+)
where
    $($name: QueryParamWithFetch<'w>,)+
{
    type Fetch = ($($name::Fetch,)+);
}

impl<$($name),+> QueryParamState for ($($name,)+)
where
    $($name: QueryParamState,)+
{
    #[inline]
    fn init(res: &Resources, components: &Components) -> Self {
        ($($name::init(res, components),)+)
    }

    #[inline]
    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        $(self.$index.matches_archetype(archetype))&&+
    }

}

impl<'w, $($name),+> QueryParamFetch<'w> for ($($name,)+)
where
    $($name: QueryParamFetch<'w>,)+
{
    type State = ($($name::State,)+);

    #[inline]
    fn fetch(res: &'w ResourcesSend, state: &Self::State) -> Self {
        ($($name::fetch(res, &state.$index),)+)
    }

    #[inline]
    fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype) {
        $(self.$index.set_archetype(&state.$index, archetype);)+
    }
}

impl<'w, 'a, $($name),+> QueryParamFetchGet<'w, 'a> for ($($name,)+)
where
    $($name: QueryParamFetchGet<'w,'a>,)+
{
    type Item = ($($name::Item,)+);

    #[inline(always)]
    fn get(&'a mut self, archetype: &Archetype, index: usize) -> Self::Item where 'w: 'a {
        ($(self.$index.get(archetype, index),)+)
    }
}


        peel! { tuple [] $($name.$index,)+ }
    )
}

tuple! { T0.0, T1.1, T2.2, T3.3, T4.4, T5.5, T6.6, T7.7, T8.8, T9.9, T10.10, T11.11, }
