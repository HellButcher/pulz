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

    /// Returns `true` if the map contains no entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the number of entries in the map.
    pub fn len(&self) -> usize {
        self.0.len()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentId;

    #[test]
    fn component_map_new_is_empty() {
        let map: ComponentMap<i32> = ComponentMap::new();
        assert!(map.get(ComponentId::<u8>::new(0)).is_none());
    }

    #[test]
    fn component_map_default_is_empty() {
        let map: ComponentMap<i32> = ComponentMap::default();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn component_map_insert_and_get() {
        let mut map = ComponentMap::new();
        let id = ComponentId::<u8>::new(42);
        map.insert(id, 100);
        assert_eq!(map.get(id), Some(&100));
    }

    #[test]
    fn component_map_insert_returns_mutable_ref() {
        let mut map = ComponentMap::new();
        let id = ComponentId::<u8>::new(1);
        let val = map.insert(id, 10);
        *val = 20;
        assert_eq!(map.get(id), Some(&20));
    }

    #[test]
    fn component_map_insert_replaces_existing() {
        let mut map = ComponentMap::new();
        let id = ComponentId::<u8>::new(1);
        map.insert(id, 1);
        map.insert(id, 2);
        assert_eq!(map.get(id), Some(&2));
    }

    #[test]
    fn component_map_get_mut() {
        let mut map = ComponentMap::new();
        let id = ComponentId::<u8>::new(1);
        map.insert(id, 42);
        if let Some(val) = map.get_mut(id) {
            *val = 99;
        }
        assert_eq!(map.get(id), Some(&99));
    }

    #[test]
    fn component_map_get_nonexistent() {
        let map: ComponentMap<i32> = ComponentMap::new();
        let id = ComponentId::<u8>::new(1);
        assert!(map.get(id).is_none());
    }

    #[test]
    fn component_map_contains() {
        let mut map = ComponentMap::new();
        let id1 = ComponentId::<u8>::new(1);
        let id2 = ComponentId::<u16>::new(2);
        assert!(!map.contains(id1));
        map.insert(id1, "hello");
        assert!(map.contains(id1));
        assert!(!map.contains(id2));
    }

    #[test]
    fn component_map_remove() {
        let mut map = ComponentMap::new();
        let id = ComponentId::<u8>::new(1);
        map.insert(id, "value");
        assert_eq!(map.remove(id), Some("value"));
        assert!(map.get(id).is_none());
    }

    #[test]
    fn component_map_remove_nonexistent() {
        let mut map: ComponentMap<&'static str> = ComponentMap::new();
        let id = ComponentId::<u8>::new(1);
        assert!(map.remove(id).is_none());
    }

    #[test]
    fn component_map_clear() {
        let mut map = ComponentMap::new();
        map.insert(ComponentId::<u8>::new(1), "a");
        map.insert(ComponentId::<u16>::new(2), "b");
        map.clear();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn component_map_get_or_insert_with_existing() {
        let mut map = ComponentMap::new();
        let id = ComponentId::<u8>::new(1);
        map.insert(id, 50);
        let val = map.get_or_insert_with(id, || 99);
        assert_eq!(*val, 50); // should return existing, not call closure
    }

    #[test]
    fn component_map_get_or_insert_with_new() {
        let mut map: ComponentMap<i32> = ComponentMap::new();
        let id = ComponentId::<u8>::new(1);
        let val = map.get_or_insert_with(id, || 42);
        assert_eq!(*val, 42);
        assert_eq!(map.get(id), Some(&42));
    }

    #[test]
    fn component_map_get_or_insert_default() {
        let mut map = ComponentMap::new();
        let id = ComponentId::<u8>::new(1);
        let val = map.get_or_insert_default(id.untyped());
        assert_eq!(*val, 0);
        assert_eq!(map.get(id), Some(&0));
    }

    #[test]
    fn component_map_keys() {
        let mut map = ComponentMap::new();
        let id1 = ComponentId::<u8>::new(3);
        let id2 = ComponentId::<u16>::new(7);
        map.insert(id1, "a");
        map.insert(id2, "b");
        let keys: Vec<_> = map.keys().collect();
        assert_eq!(keys.len(), 2);
        // Keys are untyped ComponentId — compare via .untyped()
        assert!(keys.iter().any(|k| k.untyped() == id1.untyped()));
        assert!(keys.iter().any(|k| k.untyped() == id2.untyped()));
    }

    #[test]
    fn component_map_into_entries() {
        let mut map = ComponentMap::new();
        let id = ComponentId::<u8>::new(1);
        map.insert(id, "hello");
        let entries: Vec<_> = map.into_entries().collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0.untyped(), id.untyped());
        assert_eq!(entries[0].1, "hello");
    }

    #[test]
    fn component_map_key_set() {
        let mut map = ComponentMap::new();
        let id1 = ComponentId::<u8>::new(1);
        let id2 = ComponentId::<u8>::new(3);
        map.insert(id1, "a");
        map.insert(id2, "b");
        let set = map.key_set();
        assert_eq!(set.len(), 2);
        assert!(set.contains(id1.untyped()));
        assert!(set.contains(id2.untyped()));
    }

    #[test]
    fn component_map_key_set_empty() {
        let map: ComponentMap<i32> = ComponentMap::new();
        let set = map.key_set();
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn component_map_multiple_insert_ordered() {
        let mut map: ComponentMap<&'static str> = ComponentMap::new();
        // Insert in non-sorted order to verify binary search works
        let id3 = ComponentId::<u8>::new(3);
        let id1 = ComponentId::<u8>::new(1);
        let id2 = ComponentId::<u8>::new(2);

        map.insert(id3, "c");
        map.insert(id1, "a");
        map.insert(id2, "b");

        assert_eq!(map.get(id1), Some(&"a"));
        assert_eq!(map.get(id2), Some(&"b"));
        assert_eq!(map.get(id3), Some(&"c"));
    }

    #[test]
    fn component_map_remove_preserves_order() {
        let mut map = ComponentMap::new();
        let id1 = ComponentId::<u8>::new(1);
        let id2 = ComponentId::<u8>::new(2);
        let id3 = ComponentId::<u8>::new(3);

        map.insert(id1, "a");
        map.insert(id2, "b");
        map.insert(id3, "c");

        // Remove middle element
        map.remove(id2);

        assert_eq!(map.get(id1), Some(&"a"));
        assert!(map.get(id2).is_none());
        assert_eq!(map.get(id3), Some(&"c"));
        assert_eq!(map.keys().count(), 2);
    }
}
