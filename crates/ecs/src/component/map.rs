//! A sorted-array map keyed by [`ComponentId`], searched via binary search.

use crate::component::{ComponentId, set::ComponentSet};

/// A sorted-array map keyed by untyped [`ComponentId`], using binary search for lookups.
pub struct ComponentMap<T>(Vec<(ComponentId, T)>);

impl<T> ComponentMap<T> {
    /// Creates an empty map.
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Removes all entries from the map.
    pub fn clear(&mut self) {
        self.0.clear()
    }

    #[inline]
    fn search<X>(&self, id: ComponentId<X>) -> Result<usize, usize> {
        self.0.binary_search_by(|(item_id, _)| item_id.0.cmp(&id.0))
    }

    /// Returns `true` if the map contains an entry for `id`.
    #[inline]
    pub fn contains<X>(&self, id: ComponentId<X>) -> bool {
        self.search(id).is_ok()
    }

    /// Returns a reference to the value for `id`, or `None` if absent.
    #[inline]
    pub fn get<X>(&self, id: ComponentId<X>) -> Option<&T> {
        let Ok(index) = self.search(id) else {
            return None;
        };
        // SAFETY: index was found by search
        Some(unsafe { &self.0.get_unchecked(index).1 })
    }

    /// Returns a mutable reference to the value for `id`, or `None` if absent.
    #[inline]
    pub fn get_mut<X>(&mut self, id: ComponentId<X>) -> Option<&mut T> {
        let Ok(index) = self.search(id) else {
            return None;
        };
        // SAFETY: index was found by search
        Some(unsafe { &mut self.0.get_unchecked_mut(index).1 })
    }

    /// Removes and returns the value for `id`, or `None` if absent.
    #[inline]
    pub fn remove<X>(&mut self, id: ComponentId<X>) -> Option<T> {
        match self.search(id) {
            Ok(index) => Some(self.0.remove(index).1),
            Err(_) => None,
        }
    }

    /// Inserts or replaces the value for `id`, returning a mutable reference to the stored value.
    #[inline]
    pub fn insert<X>(&mut self, id: ComponentId<X>, value: T) -> &mut T {
        match self.search(id) {
            Ok(index) => {
                // SAFETY: index was found by search
                let item = unsafe { &mut self.0.get_unchecked_mut(index).1 };
                *item = value;
                item
            }
            Err(index) => {
                self.0.insert(index, (id.untyped(), value));
                // SAFETY: index was inserted
                unsafe { &mut self.0.get_unchecked_mut(index).1 }
            }
        }
    }

    /// Returns a mutable reference for `id`, inserting the result of `create` if absent.
    #[inline]
    pub fn get_or_insert_with<X, F>(&mut self, id: ComponentId<X>, create: F) -> &mut T
    where
        F: FnOnce() -> T,
    {
        match self.search(id) {
            Ok(index) => {
                // SAFETY: index was found by search
                unsafe { &mut self.0.get_unchecked_mut(index).1 }
            }
            Err(index) => {
                self.0.insert(index, (id.untyped(), create()));
                // SAFETY: index was inserted
                unsafe { &mut self.0.get_unchecked_mut(index).1 }
            }
        }
    }

    /// Returns an iterator over `(ComponentId, &T)` pairs.
    #[inline]
    pub fn entries(&self) -> impl Iterator<Item = (ComponentId, &'_ T)> + '_ {
        self.0.iter().map(|(id, value)| (*id, value))
    }

    /// Returns a mutable iterator over `(ComponentId, &mut T)` pairs.
    #[inline]
    pub fn entries_mut(&mut self) -> impl Iterator<Item = (ComponentId, &'_ mut T)> + '_ {
        self.0.iter_mut().map(|(id, value)| (*id, value))
    }

    /// Consumes the map and returns an iterator of `(ComponentId, T)` pairs.
    #[inline]
    pub fn into_entries(self) -> impl Iterator<Item = (ComponentId, T)> {
        self.0.into_iter()
    }

    /// Returns an iterator over all keys in insertion order.
    #[inline]
    pub fn keys(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.0.iter().map(|(id, _)| *id)
    }

    /// Returns a [`ComponentSet`] of all keys in the map.
    #[inline]
    pub fn key_set(&self) -> ComponentSet {
        let mut set = ComponentSet::new();
        if let Some(((last_id, _), rest)) = self.0.split_last() {
            set.insert(*last_id); // add last id first, for allocating only once
            for (id, _) in rest {
                set.insert(*id);
            }
        }
        set
    }
}

impl<T: Default> ComponentMap<T> {
    /// Returns a mutable reference for `id`, inserting `T::default()` if absent.
    #[inline]
    pub fn get_or_insert_default(&mut self, id: ComponentId) -> &mut T {
        self.get_or_insert_with(id, Default::default)
    }
}

impl<T> Default for ComponentMap<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
