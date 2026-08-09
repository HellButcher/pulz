use crate::archetype::ArchetypeId;

/// The position of an entity within the archetype storage.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct EntityLocation {
    /// The archetype this entity belongs to.
    pub archetype_id: ArchetypeId,
    /// The row index within that archetype's component columns, or `u32::MAX` when vacant.
    pub index: u32,
}

impl EntityLocation {
    /// Sentinel value used for entities that have no archetype placement (not yet spawned or just despawned).
    pub const VACANT: Self = Self {
        archetype_id: ArchetypeId::EMPTY,
        index: u32::MAX,
    };

    /// Returns the row index as `usize` for use with slice/Vec operations.
    #[inline]
    pub fn index(self) -> usize {
        self.index as usize
    }

    /// Returns `true` if this location does not point to an archetype row.
    #[inline]
    pub fn is_vacant(&self) -> bool {
        self.index == u32::MAX
    }

    /// Returns `true` if this location points to a valid archetype row.
    #[inline]
    pub fn is_occupied(&self) -> bool {
        self.index != u32::MAX
    }
}
