//! Archetype types for grouping entities with the same set of components.
//!
//! An [`Archetype`] is a column-store for a specific [`ComponentSet`].
//! [`Archetypes`] is the registry of all archetypes in a world, keyed by their component sets.

use std::{
    collections::HashMap,
    hash::{BuildHasherDefault, Hash, Hasher},
    ops::{Deref, Index, IndexMut},
    pin::Pin,
};

use pulz_schedule::PreHashedHasher;
use roaring::RoaringBitmap;

use crate::{
    component::{ComponentId, set::ComponentSet},
    entity::Entity,
};

/// An internal unsafe pointer wrapper for ComponentSet for key lookup in the [`Archetypes::archetype_ids`] HashMap.
/// This removes the lifetime of the ComponentSet, which is owned by the Archetype, to cope with
/// self-referential lifetime in `Archetypes`.
///
/// The [`ComponentSet`] is owned exactly once (as a `Pin<Box<ComponentSet>>` inside the  [`Archetype`]).
/// This wrapper hashes and compares by content (via `&ComponentSet`) so that lookups find matches
/// correctly, while the actual key stored in the HashMap is a pointer to the owner.
///
/// THis is an unsafe pointer wrapper because the [`ComponentSet`] must never move in memory, and the
/// pointer must always point to a valid ]`ComponentSet`]. This is usually guaranteed by [`Archetypes`].
#[derive(Eq)]
struct ComponentSetKey(*const ComponentSet, u64);

unsafe impl Send for ComponentSetKey {}
unsafe impl Sync for ComponentSetKey {}

impl ComponentSetKey {
    /// SAFETY: Reference must stay valid fot the entire lifetime (Pinned)
    unsafe fn new(set: &ComponentSet) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        set.hash(&mut hasher);
        let hash = hasher.finish();
        Self(set, hash)
    }
}

impl PartialEq for ComponentSetKey {
    fn eq(&self, other: &Self) -> bool {
        // SAFETY: both pointers are valid and point to live `ComponentSet` values.
        // This is guaranteed by the `Archetypes` struct, which owns the `ComponentSet` values in pinned boxes.
        unsafe { (*self.0).eq(&*other.0) }
    }
}

impl Hash for ComponentSetKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // use the precomputed hash
        self.1.hash(state);
    }
}

/// A densely packed index identifying an archetype within [`Archetypes`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct ArchetypeId(u32);

impl ArchetypeId {
    /// The id of the archetype that contains no components (all entities start here).
    pub const EMPTY: Self = Self(0);
}

/// A group of entities that all share the exact same set of component types.
///
/// Stores the ordered list of entities belonging to this archetype; their component
/// data lives in per-component storage resources indexed by the same row order.
pub struct Archetype {
    pub(crate) id: ArchetypeId,
    pub(crate) entities: Vec<Entity>,
    components: Pin<Box<ComponentSet>>,
}

impl Deref for Archetype {
    type Target = ComponentSet;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.components
    }
}

impl Archetype {
    fn new(id: ArchetypeId, components: ComponentSet) -> Self {
        // SAFETY (for ComponentSetKey): Box is pinned so the inner ComponentSet will never move.
        let boxed = Box::pin(components);
        Self {
            id,
            entities: Vec::new(),
            components: boxed,
        }
    }

    /// Returns `true` if this archetype includes the given component.
    #[inline]
    pub fn contains<X>(&self, component_id: ComponentId<X>) -> bool {
        self.components.contains(component_id)
    }

    /// Returns the set of component ids that define this archetype.
    #[inline]
    pub fn components(&self) -> &ComponentSet {
        &self.components
    }

    /// Returns the ordered slice of entity ids stored in this archetype.
    #[inline]
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Returns the number of entities in this archetype.
    #[inline]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Returns `true` if this archetype contains no entities.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
}

/// Registry of all archetypes in a world.
///
/// Archetypes are interned by their [`ComponentSet`]: the same set always maps to the same id.
/// [`ArchetypeId::EMPTY`] is always present at index 0.
pub struct Archetypes {
    archetypes: Vec<Archetype>,
    archetype_ids: HashMap<ComponentSetKey, ArchetypeId, BuildHasherDefault<PreHashedHasher>>,
    archetype_components: ComponentSet,
}

impl Archetypes {
    /// Creates a new registry, pre-inserting the empty archetype.
    #[inline]
    pub fn new() -> Self {
        let mut archetypes = Self {
            archetypes: Vec::new(),
            archetype_ids: HashMap::default(),
            archetype_components: ComponentSet::new(),
        };

        // always add the EMPTY archetype at index 0
        let empty_components = ComponentSet::new();
        let a = Archetype::new(ArchetypeId::EMPTY, empty_components);
        // SAFETY:(for ComponentSetKey) we just pushed `a` into `archetypes`, and we won't remove it again;
        // so its lifetime is tied to `archetypes`. Its components field is pinned and won't move.
        let key = unsafe { ComponentSetKey::new(a.components()) };
        archetypes.archetypes.push(a);
        archetypes.archetype_ids.insert(key, ArchetypeId::EMPTY);
        archetypes
    }

    /// Returns the number of archetypes in the registry.
    #[inline]
    pub fn len(&self) -> usize {
        self.archetypes.len()
    }

    /// Returns `true` if no archetypes are registered.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.archetypes.is_empty()
    }

    /// Iterates over all archetypes in insertion order.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Archetype> {
        self.archetypes.iter()
    }

    /// Returns the always-present empty archetype (contains no components).
    #[inline]
    pub fn empty(&self) -> &Archetype {
        // SAFETY: empty archetype always exists
        unsafe { self.archetypes.get_unchecked(ArchetypeId::EMPTY.0 as usize) }
    }

    /// Returns the archetype for the given id, or `None` if out of range.
    #[inline]
    pub fn get(&self, id: ArchetypeId) -> Option<&Archetype> {
        self.archetypes.get(id.0 as usize)
    }

    /// Returns `true` if the component is tracked as an archetype-influencing component.
    #[inline]
    pub fn is_archetype_defining_component<X>(&self, id: ComponentId<X>) -> bool {
        self.archetype_components.contains(id)
    }

    /// Registers a component as affecting archetype membership.
    /// Called when a non-sparse (dense/ArchetypeStorage) component is first registered.
    #[inline]
    pub(crate) fn register_archetype_component<X>(&mut self, id: ComponentId<X>) {
        self.archetype_components.insert(id);
    }

    #[inline]
    pub(crate) fn get_mut(&mut self, id: ArchetypeId) -> Option<&mut Archetype> {
        self.archetypes.get_mut(id.0 as usize)
    }

    #[inline]
    pub(crate) fn get_disjoint_mut<const N: usize>(
        &mut self,
        ids: [ArchetypeId; N],
    ) -> Result<[&'_ mut Archetype; N], std::slice::GetDisjointMutError> {
        let indices = ids.map(|a| a.0 as usize);
        self.archetypes.get_disjoint_mut(indices)
    }

    pub(crate) fn get_or_insert_by_components(&mut self, mut ids: ComponentSet) -> ArchetypeId {
        ids.intersect_with(&self.archetype_components);
        ids.optimize();

        // Build a temporary key to look up by content (hash + eq delegate to &ComponentSet).
        let hash = {
            let key = unsafe { ComponentSetKey::new(&ids) };
            if let Some(&id) = self.archetype_ids.get(&key) {
                return id;
            }
            key.1
        };

        // Not found — create a new archetype with the owned ComponentSet.

        let new_id = ArchetypeId(self.archetypes.len() as u32);
        let a = Archetype::new(new_id, ids);
        // Insert the key pointing to the newly added archetype's pinned component set.
        // SAFETY:(for ComponentSetKey) we just pushed `a` into `archetypes`, and we won't remove it again;
        // so its lifetime is tied to `archetypes`. Its components field is pinned and won't move.
        let key = ComponentSetKey(a.components(), hash);
        self.archetypes.push(a);
        self.archetype_ids.insert(key, new_id);
        new_id
    }
}

impl Default for Archetypes {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Index<ArchetypeId> for Archetypes {
    type Output = Archetype;

    #[inline]
    fn index(&self, index: ArchetypeId) -> &Self::Output {
        &self.archetypes[index.0 as usize]
    }
}

impl IndexMut<ArchetypeId> for Archetypes {
    #[inline]
    fn index_mut(&mut self, index: ArchetypeId) -> &mut Self::Output {
        &mut self.archetypes[index.0 as usize]
    }
}

/// A compact bitset of [`ArchetypeId`]s, used to track which archetypes match a query.
#[derive(Clone)]
pub struct ArchetypeSet(RoaringBitmap);

impl ArchetypeSet {
    /// Creates an empty set.
    #[inline]
    pub fn new() -> Self {
        Self(RoaringBitmap::new())
    }

    /// Removes all entries from the set.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }

    /// Returns the number of distinct archetypes in the set.
    pub fn len(&self) -> usize {
        self.0.len() as usize
    }

    /// Returns `true` if the set contains no archetypes.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns `true` if the set contains `id`.
    #[inline]
    pub fn contains(&self, id: ArchetypeId) -> bool {
        self.0.contains(id.0)
    }

    /// Inserts `id`, returning `true` if it was not already present.
    #[inline]
    pub fn insert(&mut self, id: ArchetypeId) -> bool {
        self.0.insert(id.0)
    }

    /// Removes `id`, returning `true` if it was present.
    #[inline]
    pub fn remove(&mut self, id: ArchetypeId) -> bool {
        self.0.remove(id.0)
    }

    /// Returns an iterator over all ids in the set.
    #[inline]
    pub fn iter(&self) -> ArchetypeSetIter<'_> {
        ArchetypeSetIter(self.0.iter())
    }
}

impl Default for ArchetypeSet {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Extend<ArchetypeId> for ArchetypeSet {
    fn extend<I: IntoIterator<Item = ArchetypeId>>(&mut self, iter: I) {
        for t in iter {
            self.insert(t);
        }
    }
}

impl<T> FromIterator<T> for ArchetypeSet
where
    Self: Extend<T>,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut bitset = Self::new();
        bitset.extend(iter);
        bitset
    }
}

/// Borrowing iterator over an [`ArchetypeSet`].
pub struct ArchetypeSetIter<'a>(roaring::bitmap::Iter<'a>);
/// Owning iterator over an [`ArchetypeSet`].
pub struct ArchetypeSetIntoIter(roaring::bitmap::IntoIter);

impl Iterator for ArchetypeSetIter<'_> {
    type Item = ArchetypeId;
    #[inline]
    fn next(&mut self) -> Option<ArchetypeId> {
        Some(ArchetypeId(self.0.next()?))
    }
}

impl Iterator for ArchetypeSetIntoIter {
    type Item = ArchetypeId;
    #[inline]
    fn next(&mut self) -> Option<ArchetypeId> {
        Some(ArchetypeId(self.0.next()?))
    }
}

impl IntoIterator for ArchetypeSet {
    type Item = ArchetypeId;
    type IntoIter = ArchetypeSetIntoIter;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        ArchetypeSetIntoIter(self.0.into_iter())
    }
}

impl<'l> IntoIterator for &'l ArchetypeSet {
    type Item = ArchetypeId;
    type IntoIter = ArchetypeSetIter<'l>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        ArchetypeSetIter(self.0.iter())
    }
}

/// A dense `Vec`-backed map keyed by [`ArchetypeId`].
pub struct ArchetypeMap<T>(Vec<T>);

impl<T> ArchetypeMap<T> {
    /// Creates an empty map.
    #[inline]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Returns a reference to the value for `id`, or `None` if absent.
    #[inline]
    pub fn get(&self, id: ArchetypeId) -> Option<&T> {
        self.0.get(id.0 as usize)
    }

    /// Returns a mutable reference to the value for `id`, or `None` if absent.
    #[inline]
    pub fn get_mut(&mut self, id: ArchetypeId) -> Option<&mut T> {
        self.0.get_mut(id.0 as usize)
    }

    /// Returns a mutable reference for `id`, inserting a default value if absent.
    #[inline]
    pub fn get_or_insert_default(&mut self, id: ArchetypeId) -> &mut T
    where
        T: Default,
    {
        if id.0 as usize >= self.0.len() {
            self.0.resize_with(id.0 as usize + 1, || Default::default());
        }
        // SAFETY: was resized if length was to short
        unsafe { self.0.get_unchecked_mut(id.0 as usize) }
    }

    /// Returns `true` if the map contains no entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Removes all entries from the map.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl<T> Default for ArchetypeMap<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Index<ArchetypeId> for ArchetypeMap<T> {
    type Output = T;

    #[inline]
    fn index(&self, id: ArchetypeId) -> &Self::Output {
        &self.0[id.0 as usize]
    }
}

impl<T> IndexMut<ArchetypeId> for ArchetypeMap<T> {
    #[inline]
    fn index_mut(&mut self, id: ArchetypeId) -> &mut Self::Output {
        &mut self.0[id.0 as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archetypes_contains_empty_initially() {
        let mut archetypes = Archetypes::new();
        let empty = archetypes.get(ArchetypeId::EMPTY).unwrap();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert_eq!(ArchetypeId::EMPTY, empty.id);

        assert_eq!(&raw const *empty, &raw const *archetypes.empty());

        assert_eq!(
            ArchetypeId::EMPTY,
            archetypes.get_or_insert_by_components(ComponentSet::new())
        );
        assert_eq!(1, archetypes.len());
    }

    #[test]
    fn archetype_id_empty_is_zero() {
        let id1 = ArchetypeId::EMPTY;
        let id2 = ArchetypeId(0);
        assert_eq!(id1, id2);
        assert_eq!(id1.0, 0);
    }

    #[test]
    fn archetypes_get_by_id() {
        let a = Archetypes::new();
        assert!(a.get(ArchetypeId::EMPTY).is_some());
        assert!(a.get(ArchetypeId(99)).is_none());
    }

    #[test]
    fn archetypes_index_operator() {
        let a = Archetypes::new();
        let empty = &a[ArchetypeId::EMPTY];
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.id, ArchetypeId::EMPTY);
    }

    #[test]
    fn archetypes_index_mut() {
        let mut a = Archetypes::new();
        let empty = &mut a[ArchetypeId::EMPTY];
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.id, ArchetypeId::EMPTY);
    }

    // --- ArchetypeSet tests ---

    #[test]
    fn archetype_set_new_empty() {
        let s = ArchetypeSet::new();
        assert!(!s.contains(ArchetypeId(0)));
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn archetype_set_insert() {
        let mut s = ArchetypeSet::new();
        assert_eq!(s.len(), 0);
        let id = ArchetypeId(5);
        assert!(s.insert(id));
        assert_eq!(s.len(), 1);
        assert!(s.contains(id));
        assert!(!s.insert(id)); // duplicate
        assert_eq!(s.len(), 1);
        assert!(s.contains(id));
    }

    #[test]
    fn archetype_set_remove() {
        let mut s = ArchetypeSet::new();
        let id = ArchetypeId(3);
        s.insert(id);
        assert!(s.remove(id));
        assert!(!s.remove(id)); // already removed
    }

    #[test]
    fn archetype_set_clear() {
        let mut s = ArchetypeSet::new();
        assert_eq!(s.len(), 0);
        s.insert(ArchetypeId(1));
        s.insert(ArchetypeId(2));
        assert_eq!(s.len(), 2);
        s.clear();
        assert_eq!(s.len(), 0);
        assert!(!s.contains(ArchetypeId(1)));
        assert!(!s.contains(ArchetypeId(2)));
    }

    #[test]
    fn archetype_set_iter_collect() {
        let mut s = ArchetypeSet::new();
        s.insert(ArchetypeId(1));
        s.insert(ArchetypeId(3));
        s.insert(ArchetypeId(2));
        let collected: Vec<_> = s.iter().collect();
        assert_eq!(collected.len(), 3);
        // Iteration order should be sorted (RoaringBitmap)
        assert_eq!(
            collected,
            vec![ArchetypeId(1), ArchetypeId(2), ArchetypeId(3)]
        );
    }

    #[test]
    fn archetype_set_into_iter_collect() {
        let mut s = ArchetypeSet::new();
        s.insert(ArchetypeId(1));
        s.insert(ArchetypeId(2));
        let collected: Vec<_> = s.into_iter().collect();
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn archetype_set_default() {
        let s = ArchetypeSet::default();
        assert!(!s.contains(ArchetypeId(0)));
    }

    #[test]
    fn archetype_set_extend() {
        let mut s = ArchetypeSet::new();
        s.extend([ArchetypeId(1), ArchetypeId(2)]);
        assert!(s.contains(ArchetypeId(1)));
        assert!(s.contains(ArchetypeId(2)));
    }

    #[test]
    fn archetype_set_from_iterator() {
        let s: ArchetypeSet = vec![ArchetypeId(1), ArchetypeId(2)].into_iter().collect();
        assert!(s.contains(ArchetypeId(1)));
    }

    // --- ArchetypeMap tests ---

    #[test]
    fn archetype_map_new() {
        let m: ArchetypeMap<String> = ArchetypeMap::new();
        assert!(m.is_empty());
    }

    #[test]
    fn archetype_map_get_or_insert_default() {
        let mut m: ArchetypeMap<i32> = ArchetypeMap::new();
        let val = m.get_or_insert_default(ArchetypeId(0));
        assert_eq!(*val, 0);
        *val = 42;
        assert_eq!(m[ArchetypeId(0)], 42);
    }

    #[test]
    fn archetype_map_get_nonexistent() {
        let m: ArchetypeMap<i32> = ArchetypeMap::new();
        assert!(m.get(ArchetypeId(0)).is_none());
    }

    #[test]
    fn archetype_map_get_mut_existing() {
        let mut m: ArchetypeMap<i32> = ArchetypeMap::new();
        let val = m.get_or_insert_default(ArchetypeId(0));
        *val = 10;
        if let Some(v) = m.get_mut(ArchetypeId(0)) {
            assert_eq!(*v, 10);
        } else {
            panic!("expected value");
        }
    }

    #[test]
    fn archetype_map_clear() {
        let mut m: ArchetypeMap<i32> = ArchetypeMap::new();
        m.get_or_insert_default(ArchetypeId(0));
        m.clear();
        assert!(m.is_empty());
    }

    #[test]
    fn archetype_map_index_existing() {
        let mut m: ArchetypeMap<i32> = ArchetypeMap::new();
        m.get_or_insert_default(ArchetypeId(0));
        m[ArchetypeId(0)] = 99;
        assert_eq!(m[ArchetypeId(0)], 99);
    }

    #[test]
    fn archetype_map_default() {
        let m: ArchetypeMap<f64> = ArchetypeMap::default();
        assert!(m.is_empty());
    }
}
