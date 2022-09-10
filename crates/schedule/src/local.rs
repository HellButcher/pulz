use std::ops::{Deref, DerefMut};

use crate::{
    prelude::{FromResources, Resources},
    resource::ResourceAccess,
    system::param::{SystemParam, SystemParamFetch, SystemParamState},
};

pub struct Local<'l, T>(&'l mut T);

impl<'l, T> Deref for Local<'l, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'l, T> DerefMut for Local<'l, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

pub struct LocalState<T>(T);

unsafe impl<'a, T: FromResources + Sized + Send + Sync + 'static> SystemParam for Local<'a, T> {
    type Fetch = LocalState<T>;
}

// SAFETY: only local state is accessed
unsafe impl<T: FromResources + Sized + Send + Sync + 'static> SystemParamState for LocalState<T> {
    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(T::from_resources(resources))
    }

    fn update_access(&self, _resources: &Resources, _access: &mut ResourceAccess) {}
}

unsafe impl<'r, T: FromResources + Sized + Send + Sync + 'static> SystemParamFetch<'r>
    for LocalState<T>
{
    type Item = Local<'r, T>;
    fn fetch(&'r mut self, _resources: &'r Resources) -> Self::Item {
        Local(&mut self.0)
    }
}
