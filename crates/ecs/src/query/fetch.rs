use pulz_schedule::resource::{ResourceAccess, ResourceId};

use crate::{
    archetype::Archetype,
    component::{Component, ComponentId, Components},
    entity::Entity,
    query::{QueryParam, QueryParamFetch, QueryParamState},
    resource::{Res, ResMut, Resources, ResourcesSend},
    storage::Storage,
};

impl<T: Component> QueryParam for &'_ T {
    type State = QryRefState<T>;
    type Fetch<'w> = QryRefFetch<'w, T>;
}

#[doc(hidden)]
pub struct QryRefState<T: Component> {
    storage_id: ResourceId<T::Storage>,
    component_id: ComponentId<T>,
}

impl<T: Component> QueryParamState for QryRefState<T> {
    #[inline]
    fn init(_res: &Resources, components: &Components) -> Self {
        let component_id = components.expect_id::<T>();
        let component = components.get(component_id).unwrap();
        Self {
            storage_id: component.storage_id.typed(),
            component_id,
        }
    }

    #[inline]
    fn update_access(&self, access: &mut ResourceAccess) {
        access.add_shared_checked(self.storage_id);
    }

    #[inline]
    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        self.component_id.is_sparse() || archetype.contains_component_id(self.component_id)
    }
}

#[doc(hidden)]
#[repr(transparent)]
pub struct QryRefFetch<'w, T: Component>(Res<'w, T::Storage>);

impl<'w, T: Component> QueryParamFetch<'w> for QryRefFetch<'w, T> {
    type State = QryRefState<T>;
    type Item<'a> = &'a T where Self: 'a;

    #[inline]
    fn fetch(res: &'w ResourcesSend, state: &QryRefState<T>) -> Self {
        Self(
            res.borrow_res_id(state.storage_id)
                .expect("unable to borrow component"),
        )
    }

    #[inline(always)]
    fn set_archetype(&mut self, _state: &Self::State, _archetype: &Archetype) {}

    #[inline]
    fn get(&mut self, archetype: &Archetype, index: usize) -> Self::Item<'_> {
        self.0
            .get(archetype.entities[index], archetype.id, index)
            .expect("unable to get component item")
    }
}

impl<T: Component> QueryParam for &'_ mut T {
    type State = QryRefMutState<T>;
    type Fetch<'w> = QryRefMutFetch<'w, T>;
}

#[doc(hidden)]
pub struct QryRefMutState<T: Component> {
    storage_id: ResourceId<T::Storage>,
    component_id: ComponentId<T>,
}

impl<T: Component> QueryParamState for QryRefMutState<T> {
    #[inline]
    fn init(_res: &Resources, components: &Components) -> Self {
        let component_id = components.expect_id::<T>();
        let component = components.get(component_id).unwrap();
        Self {
            storage_id: component.storage_id.typed(),
            component_id,
        }
    }

    #[inline]
    fn update_access(&self, access: &mut ResourceAccess) {
        access.add_exclusive_checked(self.storage_id);
    }

    #[inline]
    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        self.component_id.is_sparse() || archetype.contains_component_id(self.component_id)
    }
}

#[doc(hidden)]
#[repr(transparent)]
pub struct QryRefMutFetch<'w, T: Component>(ResMut<'w, T::Storage>);

impl<'w, T: Component> QueryParamFetch<'w> for QryRefMutFetch<'w, T> {
    type State = QryRefMutState<T>;
    type Item<'a> = &'a mut T where Self: 'a;

    #[inline]
    fn fetch(res: &'w ResourcesSend, state: &QryRefMutState<T>) -> Self {
        Self(
            res.borrow_res_mut_id(state.storage_id)
                .expect("unable to borrow mut component"),
        )
    }

    #[inline(always)]
    fn set_archetype(&mut self, _state: &Self::State, _archetype: &Archetype) {}

    #[inline]
    fn get(&mut self, archetype: &Archetype, index: usize) -> Self::Item<'_> {
        self.0
            .get_mut(archetype.entities[index], archetype.id, index)
            .expect("unable to get component item")
    }
}

impl QueryParam for Entity {
    type State = ();
    type Fetch<'w> = QryEntityFetch;
}

#[doc(hidden)]
pub struct QryEntityFetch;

impl QueryParamFetch<'_> for QryEntityFetch {
    type State = ();
    type Item<'a> = Entity;

    #[inline(always)]
    fn fetch(_res: &ResourcesSend, _state: &()) -> Self {
        Self
    }

    #[inline(always)]
    fn set_archetype(&mut self, _state: &Self::State, _archetype: &Archetype) {}

    #[inline]
    fn get(&mut self, archetype: &Archetype, index: usize) -> Entity {
        archetype.entities[index]
    }
}

impl<Q> QueryParam for Option<Q>
where
    Q: QueryParam,
{
    type State = QryOptionState<Q::State>;
    type Fetch<'w> = QryOptionFetch<Q::Fetch<'w>>;
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
    fn update_access(&self, access: &mut ResourceAccess) {
        self.0.update_access(access);
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
    type Item<'a> = Option<F::Item<'a>> where Self: 'a;

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

    #[inline]
    fn get(&mut self, archetype: &Archetype, index: usize) -> Self::Item<'_> {
        if self.available {
            Some(self.sub_fetch.get(archetype, index))
        } else {
            None
        }
    }
}

impl QueryParam for () {
    type State = ();
    type Fetch<'w> = ();
}

impl QueryParamState for () {
    #[inline]
    fn init(_res: &Resources, _components: &Components) -> Self {}

    #[inline(always)]
    fn update_access(&self, _access: &mut ResourceAccess) {}

    #[inline(always)]
    fn matches_archetype(&self, _archetype: &Archetype) -> bool {
        true
    }
}

impl QueryParamFetch<'_> for () {
    type State = ();
    type Item<'a> = ();

    #[inline(always)]
    fn fetch(_res: &ResourcesSend, _state: &Self::State) {}

    #[inline(always)]
    fn set_archetype(&mut self, _state: &Self::State, _archetype: &Archetype) {}

    #[inline(always)]
    fn get(&mut self, _archetype: &Archetype, _index: usize) {}
}

macro_rules! impl_query_param {
    ([]) => ();
    ([$(($name:ident,$index:tt)),+]) => (

        impl<$($name),+> QueryParam for ($($name,)+)
        where
            $($name: QueryParam,)+
        {
            type State = ($($name::State,)+);
            type Fetch<'w> = ($($name::Fetch<'w>,)+);
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
            fn update_access(
                &self,
                access: &mut ResourceAccess,
            ) {
                $(self.$index.update_access(access);)+
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
            type Item<'a> = ($($name::Item<'a>,)+) where Self: 'a;

            #[inline]
            fn fetch(res: &'w ResourcesSend, state: &Self::State) -> Self {
                ($($name::fetch(res, &state.$index),)+)
            }

            #[inline]
            fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype) {
                $(self.$index.set_archetype(&state.$index, archetype);)+
            }

            #[inline(always)]
            fn get(&mut self, archetype: &Archetype, index: usize) -> Self::Item<'_> {
                ($(self.$index.get(archetype, index),)+)
            }
        }

    )
}

pulz_functional_utils::generate_variadic_array! {[T,#] impl_query_param!{}}
