//! System-local state that persists between runs of the same system instance.

use std::ops::{Deref, DerefMut};

use crate::{
    resource::{FromResources, ResourceAccess, Resources, ResourcesSend},
    system::{SystemData, SystemDataSend},
};

/// System parameter for mutable per-system-instance state that persists across frames.
///
/// The value is initialised via [`FromResources`] the first time the system runs and stored
/// alongside the system's [`Data`](SystemData::Data) — it is not a shared resource.
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

impl<T: FromResources + Sized + 'static> SystemData for Local<'_, T> {
    type Data = T;
    type Arg<'a> = Local<'a, T>;

    #[inline]
    fn init(res: &mut Resources) -> Self::Data {
        T::from_resources(res)
    }

    fn update_access(_res: &Resources, _access: &mut ResourceAccess, _data: &Self::Data) {}

    #[inline]
    fn get<'a>(_res: &'a Resources, data: &'a mut Self::Data) -> Self::Arg<'a> {
        Local(data)
    }
}

impl<T: FromResources + Send + Sync + 'static> SystemDataSend for Local<'_, T> {
    #[inline]
    fn get_send<'a>(_res: &'a ResourcesSend, data: &'a mut Self::Data) -> Self::Arg<'a> {
        Local(data)
    }
}
