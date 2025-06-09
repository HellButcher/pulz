use std::{marker::PhantomData, ops::Deref};

use pulz_schedule::{
    prelude::*,
    resource::{ResourceAccess, ResourcesSend},
    system::{SystemData, SystemDataSend},
};

use crate::{Component, Entity, storage::Tracked};

// tracks removed components
pub struct RemovedComponents<'a, C>(Res<'a, [Entity]>, PhantomData<fn(C)>);

impl<C: Component<Storage = Tracked<S>>, S: 'static> SystemData for RemovedComponents<'_, C> {
    type Data = ResourceId<C::Storage>;
    type Arg<'a> = RemovedComponents<'a, C>;

    #[inline]
    fn init(resources: &mut Resources) -> Self::Data {
        resources.expect_id::<C::Storage>()
    }

    fn update_access(_res: &Resources, access: &mut ResourceAccess, data: &Self::Data) {
        access.add_shared_checked(*data);
    }

    #[inline]
    fn get<'a>(res: &'a Resources, data: &'a mut Self::Data) -> Self::Arg<'a> {
        let storage = res.borrow_res_id(*data).expect("storage");
        RemovedComponents(Res::map(storage, |s| s.removed.as_slice()), PhantomData)
    }
}

impl<C: Component<Storage = Tracked<S>>, S: Send + Sync + 'static> SystemDataSend
    for RemovedComponents<'_, C>
{
    #[inline]
    fn get_send<'a>(res: &'a ResourcesSend, data: &'a mut Self::Data) -> Self::Arg<'a> {
        let storage = res.borrow_res_id(*data).expect("storage");
        RemovedComponents(Res::map(storage, |s| s.removed.as_slice()), PhantomData)
    }
}

impl<C> Deref for RemovedComponents<'_, C> {
    type Target = [Entity];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
