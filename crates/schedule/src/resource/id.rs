//! Phantom-typed resource identifier.

use std::{hash::Hash, marker::PhantomData};

use crate::Void;

/// A phantom-typed index that identifies a resource stored in [`Resources`].
///
/// The type parameter `T` is erased at runtime but prevents mixing up ids at compile time.
/// Use [`untyped`](ResourceId::untyped) to erase the type and [`typed`](ResourceId::typed) to restore it.
#[repr(transparent)]
pub struct ResourceId<T: ?Sized = Void>(pub(super) usize, PhantomData<fn(&T)>);

impl<T: ?Sized> std::fmt::Debug for ResourceId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ResourceId").field(&self.0).finish()
    }
}
impl<T: ?Sized> Copy for ResourceId<T> {}
impl<T: ?Sized> Clone for ResourceId<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized> Eq for ResourceId<T> {}
impl<T: ?Sized> Ord for ResourceId<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}
impl<T: ?Sized> PartialEq<Self> for ResourceId<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T: ?Sized> PartialOrd<Self> for ResourceId<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T: ?Sized> Hash for ResourceId<T> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl<T: ?Sized> ResourceId<T> {
    #[inline(always)]
    pub(super) const fn new(index: usize) -> Self {
        Self(index, PhantomData)
    }

    #[inline(always)]
    pub(super) const fn cast<X: ?Sized>(self) -> ResourceId<X> {
        ResourceId(self.0, PhantomData)
    }

    /// Erases the resource type, returning an untyped id.
    #[inline(always)]
    pub const fn untyped(self) -> ResourceId {
        self.cast()
    }
}

impl ResourceId {
    /// Restores the resource type.
    ///
    /// # Safety (logical)
    /// The caller must ensure `T` matches the original resource type.
    #[inline]
    pub fn typed<T>(self) -> ResourceId<T>
    where
        T: ?Sized + 'static,
    {
        self.cast()
    }
}
