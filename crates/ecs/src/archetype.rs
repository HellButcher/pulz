use std::{collections::BTreeMap, ops::Index};

use pulz_bitset::{BitSet, BitSetIter};

use crate::{
    component::{ComponentId, ComponentSet},
    entity::Entity,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct ArchetypeId(usize);

impl ArchetypeId {
    pub const EMPTY: Self = Self(0);

    pub(crate) const fn new(index: usize) -> Self {
        Self(index)
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0
    }
}

pub struct Archetype {
    pub(crate) id: ArchetypeId,
    pub(crate) entities: Vec<Entity>,
    pub(crate) components: ComponentSet,
}

impl Archetype {
    fn new(id: ArchetypeId, components: ComponentSet) -> Self {
        Self {
            id,
            entities: Vec::new(),
            components,
        }
    }

    #[inline]
    pub fn contains_component_id<X>(&self, component_id: ComponentId<X>) -> bool {
        self.components.contains(component_id)
    }

    #[inline]
    pub fn id(&self) -> ArchetypeId {
        self.id
    }

    #[inline]
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
}

pub struct Archetypes {
    archetypes: Vec<Archetype>,
    archetype_ids: BTreeMap<ComponentSet, ArchetypeId>,
}

impl Default for Archetypes {
    fn default() -> Self {
        let mut archetypes = Self {
            archetypes: Vec::new(),
            archetype_ids: BTreeMap::new(),
        };

        // always add the EMPTY archetype at index 0
        archetypes
            .archetypes
            .push(Archetype::new(ArchetypeId::EMPTY, ComponentSet::new()));
        archetypes
            .archetype_ids
            .insert(ComponentSet::new(), ArchetypeId::EMPTY);
        archetypes
    }
}

impl Archetypes {
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.archetypes.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.archetypes.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Archetype> {
        self.archetypes.iter()
    }

    #[inline]
    pub fn empty(&self) -> &Archetype {
        // SAFETY: empty archetype always exists
        unsafe { self.archetypes.get_unchecked(ArchetypeId::EMPTY.index()) }
    }

    #[inline]
    pub fn get(&self, id: ArchetypeId) -> Option<&Archetype> {
        self.archetypes.get(id.index())
    }

    #[inline]
    pub(crate) fn get_mut(&mut self, id: ArchetypeId) -> Option<&mut Archetype> {
        self.archetypes.get_mut(id.index())
    }

    #[inline]
    pub(crate) fn get_disjoint_array_mut<const N: usize>(
        &mut self,
        ids: [ArchetypeId; N],
    ) -> Option<[&'_ mut Archetype; N]> {
        let indices = ids.map(|a| a.index());
        slice_get_disjoint_array_mut(&mut self.archetypes, indices)
    }

    pub(crate) fn get_or_insert(&mut self, dense_ids: ComponentSet) -> ArchetypeId {
        let archetypes = &mut self.archetypes;
        *self
            .archetype_ids
            .entry(dense_ids)
            .or_insert_with_key(|dense_ids| {
                let new_id = ArchetypeId::new(archetypes.len());
                let new_archetype = Archetype::new(new_id, dense_ids.clone());
                archetypes.push(new_archetype);
                new_id
            })
    }
}

impl Index<ArchetypeId> for Archetypes {
    type Output = Archetype;

    #[inline]
    fn index(&self, index: ArchetypeId) -> &Self::Output {
        &self.archetypes[index.index()]
    }
}

/// Bit-Set like structure
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArchetypeSet(BitSet);

impl ArchetypeSet {
    #[inline]
    pub const fn new() -> Self {
        Self(BitSet::new())
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0.clear()
    }

    #[inline]
    pub fn contains(&self, id: ArchetypeId) -> bool {
        self.0.contains(id.index())
    }

    #[inline]
    pub fn insert(&mut self, id: ArchetypeId) -> bool {
        self.0.insert(id.index())
    }

    #[inline]
    pub fn remove(&mut self, id: ArchetypeId) -> bool {
        self.0.remove(id.index())
    }

    #[inline]
    pub fn first(&self) -> Option<ArchetypeId> {
        self.0.first().map(ArchetypeId)
    }

    #[inline]
    pub fn find_next(&self, id: ArchetypeId) -> Option<ArchetypeId> {
        self.0.find_next(id.index()).map(ArchetypeId)
    }

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

pub struct ArchetypeSetIter<'l>(BitSetIter<'l>);

impl<'l> Iterator for ArchetypeSetIter<'l> {
    type Item = ArchetypeId;
    #[inline]
    fn next(&mut self) -> Option<ArchetypeId> {
        Some(ArchetypeId(self.0.next()?))
    }
}

impl<'l> IntoIterator for &'l ArchetypeSet {
    type Item = ArchetypeId;
    type IntoIter = ArchetypeSetIter<'l>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[inline]
fn slice_get_disjoint_array_mut<const N: usize, T>(
    slice: &mut [T],
    indices: [usize; N],
) -> Option<[&'_ mut T; N]> {
    // check duplicates & length
    let len = slice.len();
    for i in 0..N {
        let index = indices[i];
        if index >= len {
            // out of range
            return None;
        }
        for j in 0..i {
            if index == indices[j] {
                // found duplicate index
                return None;
            }
        }
    }

    let ptr = slice.as_mut_ptr();
    // SAFETY: we have checked the following preconditions:
    //  - index range: returned references point to valid locations
    //  - diplicates: returned mutable references are not overlapping
    unsafe { Some(indices.map(|i| &mut *ptr.add(i))) }
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
        assert_eq!(ArchetypeId::EMPTY, archetypes[ArchetypeId::EMPTY].id);
    }
}
