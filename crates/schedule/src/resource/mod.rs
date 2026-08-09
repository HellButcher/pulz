//! Resource id, borrow types, and initialization traits.
//!
//! [`Resources`] is the primary container; resources are looked up by [`ResourceId`].
//! [`Res`]/[`ResMut`] are borrow guards; [`Taken`] provides owned exclusive access.

use std::ops::{Deref, DerefMut};

mod id;
mod resource_access;
mod resources;
mod system_state;

pub use self::{
    id::ResourceId,
    resource_access::ResourceAccess,
    resources::{Resources, ResourcesSend},
};

/// A shared borrow of a resource, backed by an atomic ref-cell guard.
pub type Res<'w, T> = atomic_refcell::AtomicRef<'w, T>;
/// An exclusive borrow of a resource, backed by an atomic ref-cell guard.
pub type ResMut<'w, T> = atomic_refcell::AtomicRefMut<'w, T>;

/// An owned, exclusive handle to a resource that has been temporarily removed from [`Resources`].
///
/// Must be returned via [`Resources::put_back`] before the resources container is used again.
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

/// Constructs a value from a shared reference to [`Resources`].
pub trait FromResources {
    /// Creates an instance using shared access to the resource container.
    fn from_resources(resources: &Resources) -> Self;
}

/// Constructs a value from a mutable reference to [`Resources`].
pub trait FromResourcesMut {
    /// Creates an instance using exclusive access to the resource container.
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
