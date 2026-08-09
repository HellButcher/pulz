//! Archetype types for grouping entities with the same set of components.
//!
//! An [`Archetype`] is a column-store for a specific [`ComponentSet`].
//! [`Archetypes`] is the registry of all archetypes in a world, keyed by their component sets.

use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    ops::{Deref, Index, IndexMut},
    pin::Pin,
};

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
struct ComponentSetKey(*const ComponentSet);

unsafe impl Send for ComponentSetKey {}
unsafe impl Sync for ComponentSetKey {}

impl PartialEq for ComponentSetKey {
    fn eq(&self, other: &Self) -> bool {
        // SAFETY: both pointers are valid and point to live `ComponentSet` values.
        // This is guaranteed by the `Archetypes` struct, which owns the `ComponentSet` values in pinned boxes.
        unsafe { (*self.0).eq(&*other.0) }
    }
}

impl Hash for ComponentSetKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // SAFETY: pointer is valid and points to a live `ComponentSet`.
        // This is guaranteed by the `Archetypes` struct, which owns the `ComponentSet` values in pinned boxes.
        unsafe {
            (*self.0).hash(state);
        }
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

    /// Returns a pointer to the owned [`ComponentSet`] for use in dedup lookup keys.
    #[inline]
    pub(crate) fn components_ptr(&self) -> *const ComponentSet {
        &raw const *self.components
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
    archetype_ids: HashMap<ComponentSetKey, ArchetypeId>,
    archetype_components: ComponentSet,
}

impl Archetypes {
    /// Creates a new registry, pre-inserting the empty archetype.
    #[inline]
    pub fn new() -> Self {
        let mut archetypes = Self {
            archetypes: Vec::new(),
            archetype_ids: HashMap::new(),
            archetype_components: ComponentSet::new(),
        };

        // always add the EMPTY archetype at index 0
        let empty_components = ComponentSet::new();
        let a = Archetype::new(ArchetypeId::EMPTY, empty_components);
        let ptr = a.components_ptr();
        archetypes.archetypes.push(a);
        // SAFETY:(for ComponentSetKey) we just pushed `a` into `archetypes`, and we won't remove it again;
        // so its lifetime is tied to `archetypes`. Its components field is pinned and won't move.
        archetypes
            .archetype_ids
            .insert(ComponentSetKey(ptr), ArchetypeId::EMPTY);
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
    pub fn is_archetype_component<X>(&self, id: ComponentId<X>) -> bool {
        self.archetype_components.contains(id)
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

    pub(crate) fn get_or_insert(&mut self, mut ids: ComponentSet) -> ArchetypeId {
        ids.intersect_with(&self.archetype_components);
        ids.optimize();

        // Build a temporary key to look up by content (hash + eq delegate to &ComponentSet).
        if let Some(&id) = self.archetype_ids.get(&ComponentSetKey(&raw const ids)) {
            return id;
        }

        // Not found — create a new archetype with the owned ComponentSet.

        let new_id = ArchetypeId(self.archetypes.len() as u32);
        let a = Archetype::new(new_id, ids);
        let ptr = a.components_ptr();
        self.archetypes.push(a);
        // Insert the key pointing to the newly added archetype's pinned component set.
        // SAFETY:(for ComponentSetKey) we just pushed `a` into `archetypes`, and we won't remove it again;
        // so its lifetime is tied to `archetypes`. Its components field is pinned and won't move.
        self.archetype_ids.insert(ComponentSetKey(ptr), new_id);
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
    fn empty_archetype_should_have_empty_id() {
        let mut archetypes = Archetypes::new();
        assert_eq!(
            ArchetypeId::EMPTY,
            archetypes.get_or_insert(ComponentSet::new())
        );
        assert_eq!(1, archetypes.len());
        assert_eq!(ArchetypeId::EMPTY, archetypes[ArchetypeId::EMPTY].id);
    }
}
