use std::ops::{Deref, DerefMut};

use crate::{
    resource::{FromResources, ResourceAccess, Resources, ResourcesSend},
    system::data::{SystemData, SystemDataFetch, SystemDataFetchSend},
};

pub struct Local<'l, T>(&'l mut T);

impl<T> Deref for Local<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<T> DerefMut for Local<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

impl<T: FromResources + Sized + Send + Sync + 'static> SystemData for Local<'_, T> {
    type Data = T;
    type Fetch<'a> = Local<'a, T>;
    type Arg<'a> = Local<'a, T>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Arg<'a> {
        Local(fetch.0)
    }
}

impl<'a, T: FromResources + 'static> SystemDataFetch<'a> for Local<'a, T> {
    type Data = T;

    #[inline]
    fn init(res: &mut Resources) -> Self::Data {
        T::from_resources(res)
    }
    #[inline]
    fn fetch(_res: &'a Resources, data: &'a mut Self::Data) -> Self {
        Self(data)
    }

    fn update_access(_res: &Resources, _access: &mut ResourceAccess, _data: &Self::Data) {}
}

impl<'a, T: FromResources + Send + Sync + 'static> SystemDataFetchSend<'a> for Local<'a, T> {
    #[inline]
    fn fetch_send(_res: &'a ResourcesSend, data: &'a mut Self::Data) -> Self {
        Self(data)
    }
}
