use std::ops::{Deref, DerefMut};

pub use atomic_refcell::{AtomicRef as Res, AtomicRefMut as ResMut};

mod id;
mod resource_access;
mod resources;
mod system_state;

pub use self::{
    id::ResourceId,
    resource_access::ResourceAccess,
    resources::{Resources, ResourcesSend},
    system_state::{ResMutState, ResState},
};

#[doc(hidden)]
pub enum Void {}

pub struct RemovedResource<T> {
    id: ResourceId,
    value: Box<T>,
}
impl<T> RemovedResource<T> {
    #[inline]
    pub fn id(&self) -> ResourceId<T> {
        self.id.cast()
    }

    #[inline]
    pub fn into_inner(self) -> T {
        *self.value
    }
}
impl<T> Deref for RemovedResource<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl<T> DerefMut for RemovedResource<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

pub trait FromResources {
    fn from_resources(resources: &Resources) -> Self;
}
pub trait FromResourcesMut {
    fn from_resources_mut(resources: &mut Resources) -> Self;
}

impl<T: Default> FromResources for T {
    #[inline]
    fn from_resources(_resources: &Resources) -> Self {
        T::default()
    }
}

impl<T: FromResources> FromResourcesMut for T {
    #[inline]
    fn from_resources_mut(resources: &mut Resources) -> Self {
        T::from_resources(resources)
    }
}
