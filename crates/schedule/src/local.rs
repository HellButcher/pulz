use std::ops::{Deref, DerefMut};

use crate::{
    resource::{FromResources, ResourceAccess, Resources},
    system::data::{SystemData, SystemDataFetch, SystemDataState},
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

#[doc(hidden)]
pub struct LocalState<T>(T);
#[doc(hidden)]
pub struct LocalFetch<'r, T>(&'r mut T);

impl<T: FromResources + Sized + Send + Sync + 'static> SystemData for Local<'_, T> {
    type State = LocalState<T>;
    type Fetch<'r> = LocalFetch<'r, T>;
    type Item<'a> = Local<'a, T>;

    #[inline]
    fn get<'a>(fetch: &'a mut Self::Fetch<'_>) -> Self::Item<'a> {
        Local(fetch.0)
    }
}

// SAFETY: only local state is accessed
unsafe impl<T: FromResources + Sized + Send + Sync + 'static> SystemDataState for LocalState<T> {
    #[inline]
    fn init(resources: &mut Resources) -> Self {
        Self(T::from_resources(resources))
    }

    fn update_access(&self, _resources: &Resources, _access: &mut ResourceAccess) {}
}

impl<'r, T: FromResources + Send + Sync + 'static> SystemDataFetch<'r> for LocalFetch<'r, T> {
    type State = LocalState<T>;
    #[inline]
    fn fetch(_res: &'r Resources, state: &'r mut Self::State) -> Self {
        Self(&mut state.0)
    }
}
