use super::{Res, ResMut, ResourceAccess, ResourceId, Resources};
use crate::{
    resource::ResourcesSend,
    system::data::{SystemData, SystemDataFetch, SystemDataFetchSend},
};

impl<T> SystemData for &'_ T
where
    T: 'static,
{
    type Data = ResourceId<T>;
    type Fetch<'r> = Res<'r, T>;
    type Arg<'a> = &'a T;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Arg<'a> {
        fetch
    }
}

impl<'a, T: 'static> SystemDataFetch<'a> for Res<'a, T> {
    type Data = ResourceId<T>;

    #[inline]
    fn init(res: &mut Resources) -> Self::Data {
        res.expect_id::<T>()
    }

    #[inline]
    fn fetch(res: &'a Resources, data: &'a mut Self::Data) -> Self {
        res.borrow_res_id(*data).unwrap()
    }

    #[inline]
    fn update_access(_res: &Resources, access: &mut ResourceAccess, data: &Self::Data) {
        access.add_shared_checked(*data);
    }
}

impl<'a, T: Send + Sync + 'static> SystemDataFetchSend<'a> for Res<'a, T> {
    #[inline]
    fn fetch_send(res: &'a ResourcesSend, data: &'a mut Self::Data) -> Self {
        res.borrow_res_id(*data).unwrap()
    }
}

impl<T> SystemData for &'_ mut T
where
    T: 'static,
{
    type Data = ResourceId<T>;
    type Fetch<'r> = ResMut<'r, T>;
    type Arg<'a> = &'a mut T;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Arg<'a> {
        fetch
    }
}

impl<'a, T: 'static> SystemDataFetch<'a> for ResMut<'a, T> {
    type Data = ResourceId<T>;

    #[inline]
    fn init(res: &mut Resources) -> Self::Data {
        res.expect_id::<T>()
    }

    #[inline]
    fn fetch(res: &'a Resources, data: &'a mut Self::Data) -> Self {
        res.borrow_res_mut_id(*data).unwrap()
    }

    #[inline]
    fn update_access(_res: &Resources, access: &mut ResourceAccess, data: &Self::Data) {
        access.add_exclusive_checked(*data);
    }
}

impl<'a, T: Send + Sync + 'static> SystemDataFetchSend<'a> for ResMut<'a, T> {
    #[inline]
    fn fetch_send(res: &'a ResourcesSend, data: &'a mut Self::Data) -> Self {
        res.borrow_res_mut_id(*data).unwrap()
    }
}

impl<T> SystemData for Option<&'_ T>
where
    T: 'static,
{
    type Data = Option<ResourceId<T>>;
    type Fetch<'r> = Option<Res<'r, T>>;
    type Arg<'a> = Option<&'a T>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Arg<'a> {
        fetch.as_deref()
    }
}

#[allow(clippy::use_self)]
impl<T> SystemData for Option<Res<'_, T>>
where
    T: 'static,
{
    type Data = Option<ResourceId<T>>;
    type Fetch<'r> = Option<Res<'r, T>>;
    type Arg<'a> = Option<Res<'a, T>>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Arg<'a> {
        fetch.take()
    }
}

impl<'a, T: 'static> SystemDataFetch<'a> for Option<Res<'a, T>> {
    type Data = Option<ResourceId<T>>;

    #[inline]
    fn init(res: &mut Resources) -> Self::Data {
        res.id::<T>()
    }

    #[inline]
    fn fetch(res: &'a Resources, data: &'a mut Self::Data) -> Self {
        if let Some(resource_id) = data {
            res.borrow_res_id(*resource_id)
        } else {
            None
        }
    }

    #[inline]
    fn update_access(_res: &Resources, access: &mut ResourceAccess, data: &Self::Data) {
        if let Some(resource_id) = data {
            access.add_shared_checked(*resource_id);
        }
    }
}

#[allow(clippy::use_self)]
impl<T> SystemData for Option<&'_ mut T>
where
    T: 'static,
{
    type Data = Option<ResourceId<T>>;
    type Fetch<'r> = Option<ResMut<'r, T>>;
    type Arg<'a> = Option<&'a mut T>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Arg<'a> {
        fetch.as_deref_mut()
    }
}

#[allow(clippy::use_self)]
impl<T> SystemData for Option<ResMut<'_, T>>
where
    T: 'static,
{
    type Data = Option<ResourceId<T>>;
    type Fetch<'r> = Option<ResMut<'r, T>>;
    type Arg<'a> = Option<ResMut<'a, T>>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Arg<'a> {
        fetch.take()
    }
}

impl<'a, T: 'static> SystemDataFetch<'a> for Option<ResMut<'a, T>> {
    type Data = Option<ResourceId<T>>;
    #[inline]
    fn init(res: &mut Resources) -> Self::Data {
        res.id::<T>()
    }

    #[inline]
    fn fetch(res: &'a Resources, data: &'a mut Self::Data) -> Self {
        if let Some(resource_id) = data {
            res.borrow_res_mut_id(*resource_id)
        } else {
            None
        }
    }

    #[inline]
    fn update_access(_res: &Resources, access: &mut ResourceAccess, data: &Self::Data) {
        if let Some(resource_id) = data {
            access.add_exclusive_checked(*resource_id);
        }
    }
}

#[doc(hidden)]
pub struct OwnedWrapper<T>(Option<T>);

impl<T> OwnedWrapper<T> {
    #[inline]
    pub fn take_once(&mut self) -> T {
        self.0.take().unwrap()
    }
}

impl<'a, T: SystemDataFetch<'a>> SystemDataFetch<'a> for OwnedWrapper<T> {
    type Data = T::Data;

    #[inline]
    fn init(res: &mut Resources) -> Self::Data {
        T::init(res)
    }

    #[inline]
    fn fetch(res: &'a Resources, data: &'a mut Self::Data) -> Self {
        Self(Some(T::fetch(res, data)))
    }

    #[inline]
    fn update_access(res: &Resources, access: &mut ResourceAccess, data: &Self::Data) {
        T::update_access(res, access, data);
    }
}

impl<'a, T: SystemDataFetchSend<'a>> SystemDataFetchSend<'a> for OwnedWrapper<T> {
    #[inline]
    fn fetch_send(res: &'a ResourcesSend, data: &'a mut Self::Data) -> Self {
        Self(Some(T::fetch_send(res, data)))
    }
}

#[allow(clippy::use_self)]
impl<T> SystemData for Res<'_, T>
where
    T: 'static,
{
    type Data = ResourceId<T>;
    type Fetch<'r> = OwnedWrapper<Res<'r, T>>;
    type Arg<'a> = Res<'a, T>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Arg<'a> {
        fetch.take_once()
    }
}

impl<T> SystemData for ResMut<'_, T>
where
    T: 'static,
{
    type Data = ResourceId<T>;
    type Fetch<'r> = OwnedWrapper<ResMut<'r, T>>;
    type Arg<'a> = ResMut<'a, T>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Arg<'a> {
        fetch.take_once()
    }
}
