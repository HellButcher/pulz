use pulz_arena::{Arena, Index};

use crate::archetype::ArchetypeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Entity(Index);

impl Entity {
    #[inline]
    pub fn offset(self) -> u32 {
        self.0.offset()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct EntityLocation {
    pub archetype_id: ArchetypeId,
    pub index: usize,
}

impl EntityLocation {
    pub const EMPTY: Self = Self {
        archetype_id: ArchetypeId::EMPTY,
        index: usize::MAX,
    };
}

#[derive(Clone)]
pub struct Entities {
    arena: Arena<EntityLocation>,
}

impl Entities {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            arena: Arena::new(),
        }
    }

    #[inline]
    pub fn create(&mut self) -> Entity {
        Entity(self.arena.insert(EntityLocation::EMPTY))
    }

    #[inline]
    pub fn remove(&mut self, entity: Entity) -> Option<EntityLocation> {
        self.arena.remove(entity.0)
    }

    #[inline]
    pub fn len(&self) -> u32 {
        self.arena.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    #[inline]
    pub fn reserve(&mut self, additional_capacity: usize) {
        self.arena.reserve(additional_capacity)
    }

    #[inline]
    pub fn reserve_exact(&mut self, additional_capacity: usize) {
        self.arena.reserve_exact(additional_capacity)
    }

    #[inline]
    pub fn clear(&mut self) {
        self.arena.clear()
    }

    #[inline]
    pub fn contains(&self, entity: Entity) -> bool {
        self.arena.contains(entity.0)
    }

    #[inline]
    pub fn get(&self, entity: Entity) -> Option<&EntityLocation> {
        self.arena.get(entity.0)
    }

    #[inline]
    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut EntityLocation> {
        self.arena.get_mut(entity.0)
    }

    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.arena.iter().map(|(i, _)| Entity(i))
    }
}

impl std::ops::Index<Entity> for Entities {
    type Output = EntityLocation;
    #[inline]
    fn index(&self, entity: Entity) -> &EntityLocation {
        &self.arena[entity.0]
    }
}

impl std::ops::IndexMut<Entity> for Entities {
    #[inline]
    fn index_mut(&mut self, entity: Entity) -> &mut EntityLocation {
        &mut self.arena[entity.0]
    }
}
