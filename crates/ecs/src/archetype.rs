use std::{
    cmp::Ordering,
    collections::BTreeMap,
    ops::{Index, IndexMut},
};

use pulz_bitset::BitSet;

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
    pub fn empty_mut(&mut self) -> &mut Archetype {
        // SAFETY: empty archetype always exists
        unsafe {
            self.archetypes
                .get_unchecked_mut(ArchetypeId::EMPTY.index())
        }
    }

    #[inline]
    pub fn get(&self, id: ArchetypeId) -> Option<&Archetype> {
        self.archetypes.get(id.index())
    }

    #[inline]
    pub fn get_mut(&mut self, id: ArchetypeId) -> Option<&mut Archetype> {
        self.archetypes.get_mut(id.index())
    }

    #[inline]
    pub fn get_mut2(
        &mut self,
        id1: ArchetypeId,
        id2: ArchetypeId,
    ) -> Option<(&'_ mut Archetype, &'_ mut Archetype)> {
        slice_get_mut2(&mut self.archetypes, id1.index(), id2.index())
    }

    pub fn get_or_insert(&mut self, dense_ids: ComponentSet) -> ArchetypeId {
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

impl IndexMut<ArchetypeId> for Archetypes {
    #[inline]
    fn index_mut(&mut self, index: ArchetypeId) -> &mut Self::Output {
        &mut self.archetypes[index.index()]
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
        self.0.first().map(|i| ArchetypeId(i))
    }

    #[inline]
    pub fn find_next(&self, id: ArchetypeId) -> Option<ArchetypeId> {
        self.0.find_next(id.index()).map(|i| ArchetypeId(i))
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = ArchetypeId> + '_ {
        self.0.iter().map(|i| ArchetypeId(i))
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

#[inline]
fn slice_get_mut2<T>(
    slice: &mut [T],
    index1: usize,
    index2: usize,
) -> Option<(&'_ mut T, &'_ mut T)> {
    match index1.cmp(&index2) {
        Ordering::Less => {
            let (a, b) = slice.split_at_mut(index2);
            Some((&mut a[index1], &mut b[0]))
        }
        Ordering::Greater => {
            let (a, b) = slice.split_at_mut(index1);
            Some((&mut b[0], &mut a[index2]))
        }
        Ordering::Equal => None,
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
        assert_eq!(ArchetypeId::EMPTY, archetypes[ArchetypeId::EMPTY].id)
    }
}
