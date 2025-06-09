use super::{Res, ResMut, ResourceAccess, ResourceId, Resources};
use crate::{
    resource::ResourcesSend,
    system::{SystemData, SystemDataSend},
};

#[allow(clippy::use_self)]
impl<T> SystemData for Res<'_, T>
where
    T: 'static,
{
    type Data = ResourceId<T>;
    type Arg<'a> = Res<'a, T>;

    #[inline]
    fn init(res: &mut Resources) -> Self::Data {
        res.expect_id::<T>()
    }

    #[inline]
    fn get<'a>(res: &'a Resources, data: &'a mut Self::Data) -> Self::Arg<'a> {
        res.borrow_res_id(*data).unwrap()
    }

    #[inline]
    fn update_access(_res: &Resources, access: &mut ResourceAccess, data: &Self::Data) {
        access.add_shared_checked(*data);
    }
}

impl<T: Send + Sync + 'static> SystemDataSend for Res<'_, T> {
    #[inline]
    fn get_send<'a>(res: &'a ResourcesSend, data: &'a mut Self::Data) -> Self::Arg<'a> {
        res.borrow_res_id(*data).unwrap()
    }
}

#[allow(clippy::use_self)]
impl<T> SystemData for ResMut<'_, T>
where
    T: 'static,
{
    type Data = ResourceId<T>;
    type Arg<'a> = ResMut<'a, T>;
    #[inline]
    fn init(res: &mut Resources) -> Self::Data {
        res.expect_id::<T>()
    }

    #[inline]
    fn update_access(_res: &Resources, access: &mut ResourceAccess, data: &Self::Data) {
        access.add_exclusive_checked(*data);
    }

    #[inline]
    fn get<'a>(res: &'a Resources, data: &'a mut Self::Data) -> Self::Arg<'a> {
        res.borrow_res_mut_id(*data).unwrap()
    }
}

impl<T: Send + Sync + 'static> SystemDataSend for ResMut<'_, T> {
    #[inline]
    fn get_send<'a>(res: &'a ResourcesSend, data: &'a mut Self::Data) -> Self::Arg<'a> {
        res.borrow_res_mut_id(*data).unwrap()
    }
}

#[allow(clippy::use_self)]
impl<T> SystemData for Option<Res<'_, T>>
where
    T: 'static,
{
    type Data = Option<ResourceId<T>>;
    type Arg<'a> = Option<Res<'a, T>>;

    #[inline]
    fn init(res: &mut Resources) -> Self::Data {
        res.id::<T>()
    }

    #[inline]
    fn update_access(_res: &Resources, access: &mut ResourceAccess, data: &Self::Data) {
        if let Some(resource_id) = data {
            access.add_shared_checked(*resource_id);
        }
    }

    #[inline]
    fn get<'a>(res: &'a Resources, data: &'a mut Self::Data) -> Self::Arg<'a> {
        if let Some(resource_id) = *data {
            res.borrow_res_id(resource_id)
        } else {
            None
        }
    }
}

impl<T: Send + Sync + 'static> SystemDataSend for Option<Res<'_, T>> {
    #[inline]
    fn get_send<'a>(res: &'a ResourcesSend, data: &'a mut Self::Data) -> Self::Arg<'a> {
        if let Some(resource_id) = *data {
            res.borrow_res_id(resource_id)
        } else {
            None
        }
    }
}

#[allow(clippy::use_self)]
impl<T> SystemData for Option<ResMut<'_, T>>
where
    T: 'static,
{
    type Data = Option<ResourceId<T>>;
    type Arg<'a> = Option<ResMut<'a, T>>;

    #[inline]
    fn init(res: &mut Resources) -> Self::Data {
        res.id::<T>()
    }

    #[inline]
    fn update_access(_res: &Resources, access: &mut ResourceAccess, data: &Self::Data) {
        if let Some(resource_id) = data {
            access.add_exclusive_checked(*resource_id);
        }
    }

    #[inline]
    fn get<'a>(res: &'a Resources, data: &'a mut Self::Data) -> Self::Arg<'a> {
        if let Some(resource_id) = data {
            res.borrow_res_mut_id(*resource_id)
        } else {
            None
        }
    }
}

impl<T: Send + Sync + 'static> SystemDataSend for Option<ResMut<'_, T>> {
    #[inline]
    fn get_send<'a>(res: &'a ResourcesSend, data: &'a mut Self::Data) -> Self::Arg<'a> {
        if let Some(resource_id) = data {
            res.borrow_res_mut_id(*resource_id)
        } else {
            None
        }
    }
}
