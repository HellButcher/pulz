//! Phantom-typed component identifier.

use std::{hash::Hash, marker::PhantomData};

use super::Component;
use crate::Void;

/// A phantom-typed index that identifies a registered component type.
///
/// The type parameter `T` carries the component type at compile time for type-safe
/// lookups. Use [`untyped`](ComponentId::untyped) to erase the type and
/// [`typed`](ComponentId::typed) to restore it.
#[repr(transparent)]
pub struct ComponentId<T: ?Sized = Void>(pub(super) u32, PhantomData<fn(&T)>);

impl<T: ?Sized> std::fmt::Debug for ComponentId<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ComponentId").field(&self.0).finish()
    }
}
impl<T: ?Sized> Copy for ComponentId<T> {}
impl<T: ?Sized> Clone for ComponentId<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized> Eq for ComponentId<T> {}
impl<T: ?Sized> Ord for ComponentId<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}
impl<T: ?Sized> PartialEq<Self> for ComponentId<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T: ?Sized> PartialOrd<Self> for ComponentId<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T: ?Sized> Hash for ComponentId<T> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl<T: ?Sized> ComponentId<T> {
    #[inline(always)]
    pub(super) const fn new(index: u32) -> Self {
        Self(index, PhantomData)
    }

    #[inline(always)]
    const fn cast<X: ?Sized>(self) -> ComponentId<X> {
        ComponentId(self.0, PhantomData)
    }

    /// Erases the component type, returning an untyped id.
    #[inline]
    pub const fn untyped(self) -> ComponentId {
        self.cast()
    }
}

impl ComponentId {
    /// Restores the component type, returning a typed id.
    ///
    /// # Safety (logical)
    /// The caller is responsible for ensuring `T` matches the original component type.
    #[inline]
    pub const fn typed<T>(self) -> ComponentId<T>
    where
        T: ?Sized + Component,
    {
        self.cast()
    }
}
