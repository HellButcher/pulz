use std::{marker::PhantomData, ops::Deref};

use pulz_schedule::{
    prelude::*,
    resource::ResourceAccess,
    system::data::{SystemData, SystemDataFetch, SystemDataState},
};

use crate::{storage::Tracked, Component, Entity};

// tracks removed components
pub struct RemovedComponents<'a, C>(&'a [Entity], PhantomData<fn(C)>);

#[doc(hidden)]
pub struct RemovedComponentsState<C: Component>(ResourceId<C::Storage>);

#[doc(hidden)]
pub struct RemovedComponentsFetch<'a, C: Component>(Res<'a, C::Storage>);

impl<C: Component<Storage = Tracked<S>>, S: 'static> SystemData for RemovedComponents<'_, C> {
    type State = RemovedComponentsState<C>;
    type Fetch<'r> = RemovedComponentsFetch<'r, C>;
    type Item<'a> = RemovedComponents<'a, C>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Item<'a> {
        RemovedComponents(&fetch.0.removed, PhantomData)
    }
}

// SAFETY: storage is marked as accessed
unsafe impl<C: Component<Storage = Tracked<S>>, S: 'static> SystemDataState
    for RemovedComponentsState<C>
{
    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(resources.expect_id::<C::Storage>())
    }

    fn update_access(&self, _resources: &Resources, access: &mut ResourceAccess) {
        access.add_shared_checked(self.0);
    }
}

impl<'r, C: Component<Storage = Tracked<S>>, S: 'static> SystemDataFetch<'r>
    for RemovedComponentsFetch<'r, C>
{
    type State = RemovedComponentsState<C>;
    #[inline]
    fn fetch(res: &'r Resources, state: &'r mut Self::State) -> Self {
        Self(res.borrow_res_id(state.0).expect("storage"))
    }
}

impl<C> Deref for RemovedComponents<'_, C> {
    type Target = [Entity];
    fn deref(&self) -> &Self::Target {
        self.0
    }
}
