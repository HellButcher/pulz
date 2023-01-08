use std::ops::{Deref, DerefMut};

use crate::{
    prelude::{FromResources, Resources},
    resource::ResourceAccess,
    system::param::{SystemParam, SystemParamState},
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
    type State = LocalState<T>;
}

// SAFETY: only local state is accessed
unsafe impl<T: FromResources + Sized + Send + Sync + 'static> SystemParamState for LocalState<T> {
    type Item<'r> = Local<'r, T>;

    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(T::from_resources(resources))
    }

    fn update_access(&self, _resources: &Resources, _access: &mut ResourceAccess) {}

    fn fetch<'r>(&'r mut self, _resources: &'r Resources) -> Self::Item<'r> {
        Local(&mut self.0)
    }
}
