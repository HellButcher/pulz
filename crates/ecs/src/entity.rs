use slotmap::{SlotMap, new_key_type};

use crate::archetype::ArchetypeId;
pub use crate::entity_ref::{EntityMut, EntityRef};

new_key_type! {
    pub struct Entity;
}

pub type Iter<'a> = slotmap::basic::Keys<'a, Entity, EntityLocation>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct EntityLocation {
    pub archetype_id: ArchetypeId,
    pub index: usize,
}

impl EntityLocation {
    pub const VACANT: Self = Self {
        archetype_id: ArchetypeId::EMPTY,
        index: usize::MAX,
    };
    #[inline]
    pub fn is_vacant(&self) -> bool {
        self.index == usize::MAX
    }
    #[inline]
    pub fn is_occupied(&self) -> bool {
        self.index != usize::MAX
    }
}

#[derive(Clone)]
pub struct Entities(SlotMap<Entity, EntityLocation>);

impl Entities {
    #[inline]
    pub(crate) fn new() -> Self {
        Self(SlotMap::with_key())
    }

    #[inline]
    pub(crate) fn create(&mut self) -> Entity {
        self.0.insert(EntityLocation::VACANT)
    }

    #[inline]
    pub(crate) fn remove(&mut self, entity: Entity) -> Option<EntityLocation> {
        self.0.remove(entity)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn reserve(&mut self, additional_capacity: usize) {
        self.0.reserve(additional_capacity)
    }

    #[inline]
    pub fn contains(&self, entity: Entity) -> bool {
        self.0.contains_key(entity)
    }

    #[inline]
    pub fn get(&self, entity: Entity) -> Option<EntityLocation> {
        self.0.get(entity).copied()
    }

    #[inline]
    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut EntityLocation> {
        self.0.get_mut(entity)
    }

    pub fn iter(&self) -> Iter<'_> {
        self.0.keys()
    }
}

impl std::ops::Index<Entity> for Entities {
    type Output = EntityLocation;
    #[inline]
    fn index(&self, entity: Entity) -> &EntityLocation {
        &self.0[entity]
    }
}
