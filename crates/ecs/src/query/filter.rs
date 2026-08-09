use std::marker::PhantomData;

use pulz_schedule::resource::{ResourceAccess, Resources, ResourcesSend};

use crate::{
    archetype::Archetype,
    component::{Component, ComponentId, Components},
    query::data::{QueryFetch, QueryFetchState, QueryFilter},
};

// --- With<T> ---

/// Filter: matches archetypes that contain `T`.
/// Use as the second argument to `Query`: `Query<&A, With<B>>`.
pub struct With<T>(PhantomData<fn(T)>);

pub struct WithState<T: Component> {
    component_id: ComponentId<T>,
}

impl<T: Component> QueryFetchState for WithState<T> {
    fn init(_res: &Resources, components: &Components) -> Self {
        Self {
            component_id: components.expect_id::<T>(),
        }
    }
    fn update_access(&self, _access: &mut ResourceAccess) {}
    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        archetype.contains(self.component_id)
    }
}

impl<T: Component> QueryFilter for With<T> {
    type State = WithState<T>;
    type Fetch<'w> = WithFetch<T>;
}

pub struct WithFetch<T>(PhantomData<fn(T)>);

impl<'w, T: Component> QueryFetch<'w> for WithFetch<T> {
    type State = WithState<T>;
    type Item<'a> = ();

    #[inline]
    fn fetch(_res: &'w ResourcesSend, _state: &Self::State) -> Self {
        Self(PhantomData)
    }
    #[inline]
    fn set_archetype(&mut self, _state: &Self::State, _archetype: &Archetype) {}
    #[inline]
    fn get(&mut self, _archetype: &Archetype, _index: usize) {}
}

// --- Without<T> ---

/// Filter: matches archetypes that do NOT contain `T`.
/// Use as the second argument to `Query`: `Query<&A, Without<B>>`.
pub struct Without<T>(PhantomData<fn(T)>);

pub struct WithoutState<T: Component> {
    component_id: ComponentId<T>,
}

impl<T: Component> QueryFetchState for WithoutState<T> {
    fn init(_res: &Resources, components: &Components) -> Self {
        Self {
            component_id: components.expect_id::<T>(),
        }
    }
    fn update_access(&self, _access: &mut ResourceAccess) {}
    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        !archetype.contains(self.component_id)
    }
}

impl<T: Component> QueryFilter for Without<T> {
    type State = WithoutState<T>;
    type Fetch<'w> = WithoutFetch<T>;
}

pub struct WithoutFetch<T>(PhantomData<fn(T)>);

impl<'w, T: Component> QueryFetch<'w> for WithoutFetch<T> {
    type State = WithoutState<T>;
    type Item<'a> = ();

    #[inline]
    fn fetch(_res: &'w ResourcesSend, _state: &Self::State) -> Self {
        Self(PhantomData)
    }
    #[inline]
    fn set_archetype(&mut self, _state: &Self::State, _archetype: &Archetype) {}
    #[inline]
    fn get(&mut self, _archetype: &Archetype, _index: usize) {}
}
