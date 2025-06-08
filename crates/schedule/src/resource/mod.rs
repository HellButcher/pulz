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
};

pub struct Taken<T: ?Sized> {
    value: Box<T>,
    id: ResourceId,
    #[cfg(debug_assertions)]
    atom: crate::atom::Atom,
}

impl<T: ?Sized> Taken<T> {
    #[inline]
    pub fn id(&self) -> ResourceId<T> {
        self.id.cast()
    }

    #[inline]
    pub fn into_box(self) -> Box<T> {
        self.value
    }
}

impl<T> Taken<T> {
    #[inline]
    pub fn into_inner(self) -> T {
        *self.value
    }
}

impl<T: ?Sized> Deref for Taken<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: ?Sized> DerefMut for Taken<T> {
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
