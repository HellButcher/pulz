use std::{
    cmp::Ordering,
    collections::BTreeMap,
    ops::{Index, IndexMut},
};

use crate::{
    component::{Components,ComponentMap, ComponentSet},
    entity::Entity,
    storage::ArchetypeStorage,
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
    id: ArchetypeId,
    pub(crate) entities: Vec<Entity>,
    pub(crate) dense_storage: ComponentMap<Box<dyn ArchetypeStorage>>,
}

impl Archetype {
    fn new(id: ArchetypeId) -> Self {
        Self {
            id,
            entities: Vec::new(),
            dense_storage: ComponentMap::new(),
        }
    }

    fn extend(&mut self, dense_components: &ComponentSet, components: &Components) {
        for id in dense_components.iter(components) {
            self.dense_storage
                .get_or_insert_with(id, || components.new_archetype_storage(id));
        }
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
            .push(Archetype::new(ArchetypeId::EMPTY));
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
        let id1 = id1.index();
        let id2 = id2.index();
        match id1.cmp(&id2) {
            Ordering::Less => {
                let (a, b) = self.archetypes.split_at_mut(id2);
                Some((&mut a[id1], &mut b[0]))
            }
            Ordering::Greater => {
                let (a, b) = self.archetypes.split_at_mut(id1);
                Some((&mut b[0], &mut a[id2]))
            }
            Ordering::Equal => None,
        }
    }

    pub fn get_or_insert(
        &mut self,
        dense_ids: ComponentSet,
        components: &Components,
    ) -> ArchetypeId {
        let archetypes = &mut self.archetypes;
        *self
            .archetype_ids
            .entry(dense_ids)
            .or_insert_with_key(|dense_ids| {
                let new_id = ArchetypeId::new(archetypes.len());
                let mut new_archetype = Archetype::new(new_id);
                new_archetype.extend(dense_ids, components);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_archetype_should_have_empty_id() {
        let components = Components::new();
        let mut archetypes = Archetypes::new();
        assert_eq!(
            ArchetypeId::EMPTY,
            archetypes.get_or_insert(ComponentSet::new(), &components)
        );
        assert_eq!(ArchetypeId::EMPTY, archetypes[ArchetypeId::EMPTY].id)
    }
}
