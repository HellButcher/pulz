use std::{marker::PhantomData, ops::Deref};

use pulz_schedule::{
    prelude::*,
    resource::{ResourceAccess, ResourcesSend},
    system::data::{SystemData, SystemDataFetch, SystemDataFetchSend},
};

use crate::{Component, Entity, storage::Tracked};

// tracks removed components
pub struct RemovedComponents<'a, C>(&'a [Entity], PhantomData<fn(C)>);

#[doc(hidden)]
pub struct RemovedComponentsFetch<'a, C: Component>(Res<'a, C::Storage>);

impl<C: Component<Storage = Tracked<S>>, S: 'static> SystemData for RemovedComponents<'_, C> {
    type Data = ResourceId<C::Storage>;
    type Fetch<'r> = RemovedComponentsFetch<'r, C>;
    type Arg<'a> = RemovedComponents<'a, C>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Arg<'a> {
        RemovedComponents(&fetch.0.removed, PhantomData)
    }
}

impl<'r, C: Component<Storage = Tracked<S>>, S: 'static> SystemDataFetch<'r>
    for RemovedComponentsFetch<'r, C>
{
    type Data = ResourceId<C::Storage>;

    #[inline]
    fn init(resources: &mut Resources) -> Self::Data {
        resources.expect_id::<C::Storage>()
    }

    fn update_access(_res: &Resources, access: &mut ResourceAccess, data: &Self::Data) {
        access.add_shared_checked(*data);
    }

    #[inline]
    fn fetch(res: &'r Resources, data: &'r mut Self::Data) -> Self {
        Self(res.borrow_res_id(*data).expect("storage"))
    }
}

impl<'r, C: Component<Storage = Tracked<S>>, S: Send + Sync + 'static> SystemDataFetchSend<'r>
    for RemovedComponentsFetch<'r, C>
{
    #[inline]
    fn fetch_send(res: &'r ResourcesSend, data: &'r mut Self::Data) -> Self {
        Self(res.borrow_res_id(*data).expect("storage"))
    }
}

impl<C> Deref for RemovedComponents<'_, C> {
    type Target = [Entity];
    fn deref(&self) -> &Self::Target {
        self.0
    }
}
