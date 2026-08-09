use std::marker::PhantomData;

use pulz_schedule::{
    prelude::{Res, ResMut},
    resource::{ResourceAccess, ResourceId, Resources, ResourcesSend},
};

use crate::{
    archetype::Archetype,
    component::{Component, ComponentId, Components},
    entity::Entity,
    query::data::{QueryData, QueryFetch, QueryFetchState, QueryFilter},
    storage::Storage,
};

// --- &T ---

pub struct QryRefState<T: Component> {
    pub(super) storage_id: ResourceId<T::Storage>,
    pub(super) component_id: ComponentId<T>,
}

impl<T: Component> QueryFetchState for QryRefState<T> {
    fn init(_res: &Resources, components: &Components) -> Self {
        let component_id = components.expect_id::<T>();
        let component = components.get(component_id).unwrap();
        Self {
            storage_id: component.storage_id.typed(),
            component_id,
        }
    }

    fn update_access(&self, access: &mut ResourceAccess) {
        access.add_shared_checked(self.storage_id);
    }

    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        T::Storage::SPARSE || archetype.contains(self.component_id)
    }
}

impl<T: Component> QueryFilter for &T {
    type State = QryRefState<T>;
    type Fetch<'w> = QryRefFetch<'w, T>;
}

impl<T: Component> QueryData for &T {
    type Item<'a>
        = &'a T
    where
        Self: 'a;
}

#[repr(transparent)]
pub struct QryRefFetch<'w, T: Component>(Res<'w, T::Storage>);

impl<'w, T: Component> QueryFetch<'w> for QryRefFetch<'w, T> {
    type State = QryRefState<T>;
    type Item<'a>
        = &'a T
    where
        Self: 'a;

    #[inline]
    fn fetch(res: &'w ResourcesSend, state: &Self::State) -> Self {
        Self(
            res.borrow_res_id(state.storage_id)
                .expect("component storage not initialized"),
        )
    }

    #[inline]
    fn set_archetype(&mut self, _state: &Self::State, _archetype: &Archetype) {}

    #[inline]
    fn get<'a>(&'a mut self, archetype: &Archetype, index: usize) -> &'a T {
        self.0
            .get(archetype.entities()[index], archetype.id, index)
            .expect("component missing in archetype")
    }
}

// --- &mut T ---

pub struct QryRefMutState<T: Component> {
    pub(super) storage_id: ResourceId<T::Storage>,
    pub(super) component_id: ComponentId<T>,
}

impl<T: Component> QueryFetchState for QryRefMutState<T> {
    fn init(_res: &Resources, components: &Components) -> Self {
        let component_id = components.expect_id::<T>();
        let component = components.get(component_id).unwrap();
        Self {
            storage_id: component.storage_id.typed(),
            component_id,
        }
    }

    fn update_access(&self, access: &mut ResourceAccess) {
        access.add_exclusive_checked(self.storage_id);
    }

    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        T::Storage::SPARSE || archetype.contains(self.component_id)
    }
}

impl<T: Component> QueryFilter for &mut T {
    type State = QryRefMutState<T>;
    type Fetch<'w> = QryRefMutFetch<'w, T>;
}

impl<T: Component> QueryData for &mut T {
    type Item<'a>
        = &'a mut T
    where
        Self: 'a;
}

#[repr(transparent)]
pub struct QryRefMutFetch<'w, T: Component>(ResMut<'w, T::Storage>);

impl<'w, T: Component> QueryFetch<'w> for QryRefMutFetch<'w, T> {
    type State = QryRefMutState<T>;
    type Item<'a>
        = &'a mut T
    where
        Self: 'a;

    #[inline]
    fn fetch(res: &'w ResourcesSend, state: &Self::State) -> Self {
        Self(
            res.borrow_res_mut_id(state.storage_id)
                .expect("component storage not initialized"),
        )
    }

    #[inline]
    fn set_archetype(&mut self, _state: &Self::State, _archetype: &Archetype) {}

    #[inline]
    fn get<'a>(&'a mut self, archetype: &Archetype, index: usize) -> &'a mut T {
        self.0
            .get_mut(archetype.entities()[index], archetype.id, index)
            .expect("component missing in archetype")
    }
}

// --- Entity ---

impl QueryFilter for Entity {
    type State = ();
    type Fetch<'w> = QryEntityFetch;
}

impl QueryData for Entity {
    type Item<'a> = Self;
}

pub struct QryEntityFetch;

impl<'w> QueryFetch<'w> for QryEntityFetch {
    type State = ();
    type Item<'a> = Entity;

    #[inline]
    fn fetch(_res: &'w ResourcesSend, _state: &()) -> Self {
        Self
    }
    #[inline]
    fn set_archetype(&mut self, _state: &(), _archetype: &Archetype) {}
    #[inline]
    fn get(&mut self, archetype: &Archetype, index: usize) -> Entity {
        archetype.entities()[index]
    }
}

// --- Option<Q> ---
//
// `QryOptionFetch` is parameterized by `Q: QueryFilter` so `set_archetype` can call
// `Q::Fetch::matches_archetype` to decide per-archetype availability.

pub struct QryOptionState<S: QueryFetchState>(pub(super) S);

impl<S: QueryFetchState> QueryFetchState for QryOptionState<S> {
    fn init(res: &Resources, components: &Components) -> Self {
        Self(S::init(res, components))
    }
    fn update_access(&self, access: &mut ResourceAccess) {
        self.0.update_access(access);
    }
    fn matches_archetype(&self, _archetype: &Archetype) -> bool {
        true
    }
}

impl<Q: QueryFilter> QueryFilter for Option<Q> {
    type State = QryOptionState<Q::State>;
    type Fetch<'w> = QryOptionFetch<'w, Q>;
}

impl<Q: QueryData> QueryData for Option<Q> {
    type Item<'a>
        = Option<Q::Item<'a>>
    where
        Self: 'a;
}

pub struct QryOptionFetch<'w, Q: QueryFilter> {
    available: bool,
    inner: Q::Fetch<'w>,
    _phantom: PhantomData<fn() -> Q>,
}

// SAFETY: Q::Fetch<'w> is Send; PhantomData<fn() -> Q> is always Send
unsafe impl<Q: QueryFilter> Send for QryOptionFetch<'_, Q> where for<'w> Q::Fetch<'w>: Send {}

impl<'w, Q: QueryFilter> QueryFetch<'w> for QryOptionFetch<'w, Q> {
    type State = QryOptionState<Q::State>;
    type Item<'a>
        = Option<<Q::Fetch<'w> as QueryFetch<'w>>::Item<'a>>
    where
        Self: 'a;

    #[inline]
    fn fetch(res: &'w ResourcesSend, state: &Self::State) -> Self {
        Self {
            available: false,
            inner: <Q::Fetch<'w> as QueryFetch<'w>>::fetch(res, &state.0),
            _phantom: PhantomData,
        }
    }

    #[inline]
    fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype) {
        self.available = state.0.matches_archetype(archetype);
        if self.available {
            self.inner.set_archetype(&state.0, archetype);
        }
    }

    #[inline]
    fn get<'a>(&'a mut self, archetype: &Archetype, index: usize) -> Self::Item<'a> {
        if self.available {
            Some(self.inner.get(archetype, index))
        } else {
            None
        }
    }
}
