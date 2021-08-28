use std::{
    collections::BTreeMap,
    ops::{Index, IndexMut},
};

use crate::{
    component::{ComponentId, ComponentSet},
    entity::Entity,
    storage::slice_get_mut2,
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

    #[inline]
    fn next(self) -> Self {
        Self(self.0 + 1)
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
    pub fn contains_component_id(&self, component_id: ComponentId) -> bool {
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
pub struct ArchetypeSet(Vec<u64>);

impl ArchetypeSet {
    #[inline]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    pub fn clear(&mut self) {
        self.0.clear()
    }

    #[inline]
    fn split(id: ArchetypeId) -> (usize, u64) {
        let offset = id.index();
        let index = offset / 64;
        let bits = 1u64 << (offset % 64);
        (index, bits)
    }

    #[inline]
    pub fn contains(&self, id: ArchetypeId) -> bool {
        let (index, bits) = Self::split(id);
        if let Some(value) = self.0.get(index) {
            *value & bits != 0
        } else {
            false
        }
    }

    pub fn insert(&mut self, id: ArchetypeId) {
        let (index, bits) = Self::split(id);
        if index >= self.0.len() {
            self.0.resize(index + 1, 0);
        }
        // SAFETY: vec was extended to contain index
        let value = unsafe { self.0.get_unchecked_mut(index) };
        *value |= bits;
    }

    pub fn remove(&mut self, id: ArchetypeId) {
        let (index, bits) = Self::split(id);
        if let Some(value) = self.0.get_mut(index) {
            *value &= !bits;
        }
    }

    pub fn find_next(&self, id: ArchetypeId) -> Option<ArchetypeId> {
        let mut id = id.next();
        let (mut index, mut bits) = Self::split(id);
        while let Some(value) = self.0.get(index) {
            if *value & bits != 0 {
                return Some(id);
            }
            id = id.next();
            if bits > (!0 >> 1) {
                index += 1;
                bits = 1;
            } else {
                bits <<= 1;
            }
        }
        None
    }

    fn sub_iter(start: usize, mut value: u64) -> impl Iterator<Item = ArchetypeId> {
        let mut i = start;
        std::iter::from_fn(move || {
            while value != 0 {
                if value & 1 == 1 {
                    let result = i;
                    i += 1;
                    value >>= 1;
                    return Some(ArchetypeId(result));
                }
                i += 1;
                value >>= 1;
            }
            None
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = ArchetypeId> + '_ {
        self.0
            .iter()
            .copied()
            .enumerate()
            .flat_map(|(i, value)| Self::sub_iter(i * 64, value))
    }

    pub fn into_iter(self) -> impl Iterator<Item = ArchetypeId> {
        self.0
            .into_iter()
            .enumerate()
            .flat_map(|(i, value)| Self::sub_iter(i * 64, value))
    }
}

impl Default for ArchetypeSet {
    #[inline]
    fn default() -> Self {
        Self::new()
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
