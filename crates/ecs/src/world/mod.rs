use std::{
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
};

use pulz_schedule::{
    prelude::{Res, Resources},
    resource::Taken,
};

use crate::{
    ResourcesExt, WorldInner, WorldMutInnerTemp,
    archetype::Archetypes,
    component::{Component, ComponentId, Components},
    entity::{Entities, Entity},
};

mod inner;

/// A shared (read-only) view of the ECS world, borrowing from a [`Resources`] container.
///
/// Provides access to entities, components, and archetypes.
/// Obtain one via [`ResourcesExt::world`].
pub struct World<'a> {
    pub(crate) res: &'a Resources,
    pub(crate) world: Res<'a, WorldInner>,
}

impl<'a> World<'a> {
    #[inline]
    pub(super) fn from_resources(res: &'a Resources) -> Self {
        let world = res
            .borrow_res::<WorldInner>()
            .expect("EcsModule not initialized");
        World { res, world }
    }

    /// Returns the archetype registry for this world.
    #[inline]
    pub fn archetypes(&self) -> &Archetypes {
        &self.world.archetypes
    }

    /// Returns the component registry for this world.
    #[inline]
    pub fn components(&self) -> &Components {
        &self.world.components
    }

    /// Returns the entity registry for this world.
    #[inline]
    pub fn entities(&self) -> &Entities {
        &self.world.entities
    }
}

impl Clone for World<'_> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            res: self.res,
            world: Res::clone(&self.world),
        }
    }
}

impl Deref for World<'_> {
    type Target = Resources;
    #[inline]
    fn deref(&self) -> &Resources {
        self.res
    }
}

/// An exclusive (read-write) view of the ECS world, mutably borrowing from a [`Resources`] container.
///
/// Takes [`WorldInner`] out of resources via [`Taken<T>`] while alive, preventing
/// other exclusive borrows. Obtain one via [`ResourcesExt::world_mut`].
pub struct WorldMut<'a> {
    pub(crate) res: &'a mut Resources,
    pub(crate) world: ManuallyDrop<Taken<WorldInner>>,
    pub(crate) world_tmp: ManuallyDrop<Taken<WorldMutInnerTemp>>,
}

impl<'a> WorldMut<'a> {
    #[inline]
    pub(super) fn from_resources_mut(res: &'a mut Resources) -> Self {
        let world_id = res.init::<WorldInner>();
        let world_tmp_id = res.init::<WorldMutInnerTemp>();
        let world = res.take_id(world_id).unwrap();
        let world_tmp = res.take_id(world_tmp_id).unwrap();
        WorldMut {
            res,
            world: ManuallyDrop::new(world),
            world_tmp: ManuallyDrop::new(world_tmp),
        }
    }

    /// Returns the archetype registry for this world.
    #[inline]
    pub fn archetypes(&self) -> &Archetypes {
        &self.world.archetypes
    }

    /// Returns the component registry for this world.
    #[inline]
    pub fn components(&self) -> &Components {
        &self.world.components
    }

    /// Returns the entity registry for this world.
    #[inline]
    pub fn entities(&self) -> &Entities {
        &self.world.entities
    }

    /// Registers component `T` and its storage, returning `Ok(id)` on first call and `Err(id)` if already registered.
    ///
    /// # Errors
    ///
    /// Returns `Err(id)` if the component type `T` is already registered. The returned id matches the existing registration.
    #[inline]
    pub fn try_init<T>(&mut self) -> Result<ComponentId<T>, ComponentId<T>>
    where
        T: Component,
    {
        self.world.try_init_component::<T>(self.res)
    }

    /// Registers component `T`, returning its id regardless of whether it was already registered.
    #[inline]
    pub fn init<T>(&mut self) -> ComponentId<T>
    where
        T: Component,
    {
        match self.try_init() {
            Ok(id) | Err(id) => id,
        }
    }

    /// Removes the entity and all its components from the world.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        let Some(ent) = self.entity_mut(entity) else {
            return false;
        };
        ent.despawn();
        true
    }
}

impl Drop for WorldMut<'_> {
    fn drop(&mut self) {
        // SAFETY: only deconstructed here
        let world = unsafe { ManuallyDrop::take(&mut self.world) };
        self.res.put_back(world);
        let world_tmp = unsafe { ManuallyDrop::take(&mut self.world_tmp) };
        self.res.put_back(world_tmp);
    }
}

impl Deref for WorldMut<'_> {
    type Target = Resources;
    #[inline]
    fn deref(&self) -> &Resources {
        self.res
    }
}

impl DerefMut for WorldMut<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Resources {
        self.res
    }
}

impl ResourcesExt for Resources {
    #[inline]
    fn world(&self) -> World<'_> {
        World::from_resources(self)
    }

    #[inline]
    fn world_mut(&mut self) -> WorldMut<'_> {
        WorldMut::from_resources_mut(self)
    }
}

#[cfg(test)]
mod tests {
    use pulz_schedule::prelude::Resources;

    use crate::{EcsModule, ResourcesExt};

    // --- World ---

    #[test]
    fn world_archetypes_len() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        let world = resources.world_mut();
        assert_eq!(world.archetypes().len(), 1);
    }

    #[test]
    fn world_components_empty() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        let world = resources.world_mut();
        assert!(world.components().is_empty());
    }

    #[test]
    fn world_entities_empty() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        let world = resources.world();
        assert!(world.entities().is_empty());
    }

    // --- WorldMut ---

    #[test]
    fn world_mut_archetypes_len() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        let world = resources.world_mut();
        assert_eq!(world.archetypes().len(), 1);
    }

    #[test]
    fn world_mut_components_empty() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        let world = resources.world_mut();
        assert!(world.components().is_empty());
    }

    #[test]
    fn world_mut_entities_empty() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        let world = resources.world_mut();
        assert!(world.entities().is_empty());
    }
}
