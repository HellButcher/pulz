//! Component registry that maps types to their metadata and storage resources.

use std::{any::TypeId, borrow::Cow, collections::hash_map::Entry, ops::Index};

use pulz_schedule::{
    TypeIdMap,
    prelude::{Res, Resources},
    resource::{ResMut, ResourceId},
};

use super::{Component, ComponentId};
use crate::{
    archetype::Archetype,
    storage::{AnyStorage, Storage},
};

pub enum ComponentVariant {
    Simple,
    Archetype,
    ArchetypeDependent(Vec<ComponentId>),
}

/// Metadata for a registered component type, including its storage resource id.
pub struct ComponentData {
    id: ComponentId,
    name: Cow<'static, str>,
    type_id: TypeId,
    /// whether this component requires asking the storage for checking, if an entity contains it
    pub(crate) requires_detailed_storage_check: bool,
    pub(crate) variant: ComponentVariant,
    pub(crate) storage_id: ResourceId,
    borrow_mut_any_storage_fn: fn(res: &Resources, id: ResourceId) -> ResMut<'_, dyn AnyStorage>,
}

impl ComponentData {
    /// Returns the untyped id of this component.
    #[inline]
    pub fn id(&self) -> ComponentId {
        self.id
    }

    /// Returns the Rust type name of this component.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the [`TypeId`] of this component's Rust type.
    #[inline]
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Returns `true` if this component is present in the given archetype.
    pub fn check_archetype(&self, archetype: &Archetype) -> bool {
        match self.variant {
            ComponentVariant::Simple => true,
            ComponentVariant::Archetype => archetype.contains(self.id),
            ComponentVariant::ArchetypeDependent(ref components) => {
                components.iter().all(|id| archetype.contains(*id))
            }
        }
    }

    #[inline]
    pub(crate) fn borrow_mut_any_storage<'a>(
        &self,
        res: &'a Resources,
    ) -> ResMut<'a, dyn AnyStorage> {
        (self.borrow_mut_any_storage_fn)(res, self.storage_id)
    }

    pub(crate) fn borrow_res_storage<'a, T>(
        &self,
        res: &'a Resources,
    ) -> Option<Res<'a, T::Storage>>
    where
        T: Component,
    {
        debug_assert_eq!(TypeId::of::<T>(), self.type_id);
        let storage_id: ResourceId<T::Storage> = self.storage_id.typed();
        res.borrow_res_id(storage_id)
    }

    pub(crate) fn borrow_mut_storage<'a, T>(
        &self,
        res: &'a Resources,
    ) -> Option<ResMut<'a, T::Storage>>
    where
        T: Component,
    {
        debug_assert_eq!(TypeId::of::<T>(), self.type_id);
        let storage_id: ResourceId<T::Storage> = self.storage_id.typed();
        res.borrow_res_mut_id(storage_id)
    }
}

/// Registry of all component types known to the world, indexed by [`ComponentId`].
pub struct Components {
    pub(crate) components: Vec<ComponentData>,
    by_type_id: TypeIdMap<ComponentId>,
}

impl Default for Components {
    fn default() -> Self {
        Self::new()
    }
}

impl Components {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            components: Vec::new(),
            by_type_id: TypeIdMap::default(),
        }
    }

    /// Returns the typed id of component `T` if it has been registered, otherwise `None`.
    #[inline]
    pub fn id<T>(&self) -> Option<ComponentId<T>>
    where
        T: Component,
    {
        let type_id = TypeId::of::<T>();
        self.by_type_id
            .get(&type_id)
            .copied()
            .map(ComponentId::typed)
    }

    /// Returns the typed id of component `T`.
    ///
    /// # Panics
    /// Panics if `T` has not been registered.
    #[inline]
    pub fn expect_id<T>(&self) -> ComponentId<T>
    where
        T: Component,
    {
        let Some(id) = self.id::<T>() else {
            panic!("component {} not initialized", std::any::type_name::<T>());
        };
        id
    }

    /// Returns the metadata for the given component id, or `None` if out of range.
    pub fn get<T>(&self, component_id: ComponentId<T>) -> Option<&ComponentData> {
        self.components.get(component_id.0 as usize)
    }

    pub(crate) fn try_init<T>(
        &mut self,
        storage_id: ResourceId<T::Storage>,
    ) -> Result<ComponentId<T>, ComponentId<T>>
    where
        T: Component,
    {
        let type_id = TypeId::of::<T>();
        let components = &mut self.components;
        match self.by_type_id.entry(type_id) {
            Entry::Vacant(entry) => {
                let index = components.len();
                let id = ComponentId::new(index as u32);
                components.push(ComponentData {
                    id,
                    name: Cow::Borrowed(std::any::type_name::<T>()),
                    type_id,
                    requires_detailed_storage_check: T::Storage::SPARSE,
                    variant: if T::Storage::SPARSE {
                        ComponentVariant::Simple
                    } else {
                        ComponentVariant::Archetype
                    },
                    storage_id: storage_id.untyped().typed(),
                    borrow_mut_any_storage_fn: T::Storage::borrow_mut_any,
                });
                entry.insert(id);
                Ok(id.typed())
            }
            Entry::Occupied(entry) => Err((*entry.get()).typed()),
        }
    }

    /// Returns the number of registered component types.
    #[inline]
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Returns `true` if no components have been registered yet.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

impl Index<ComponentId> for Components {
    type Output = ComponentData;

    #[inline]
    fn index(&self, index: ComponentId) -> &Self::Output {
        &self.components[index.0 as usize]
    }
}

#[cfg(test)]
mod tests {
    use pulz_schedule::prelude::Resources;

    use super::*;
    use crate::{EcsModule, ResourcesExt, component::Component};

    #[derive(Component)]
    struct TestPos {
        x: f32,
        y: f32,
    }

    #[derive(Component)]
    struct TestVel {
        dx: f32,
        dy: f32,
    }

    #[test]
    fn components_new_is_empty() {
        let components = Components::new();
        assert!(components.is_empty());
        assert_eq!(components.len(), 0);
    }

    #[test]
    fn components_try_init_new_component() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        let mut world = resources.world_mut();
        let id = world.init::<TestPos>();
        assert_eq!(id, world.components().expect_id::<TestPos>());
    }

    #[test]
    fn components_try_init_duplicate_returns_err() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        let mut world = resources.world_mut();
        let first = world.init::<TestPos>();
        drop(world);

        // Second init with a fresh storage id for the same component type
        let mut world = resources.world_mut();
        let second = world.try_init::<TestPos>();
        assert!(second.is_err());
        assert_eq!(second.unwrap_err(), first);
    }

    #[test]
    fn components_id_lookup_before_and_after() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        // Before init, id returns None
        assert!(resources.world().components().id::<TestPos>().is_none());

        let mut world = resources.world_mut();
        let id = world.init::<TestPos>();
        // After init, id returns Some
        assert_eq!(world.components().id::<TestPos>(), Some(id));
    }

    #[test]
    #[should_panic]
    fn components_expect_id_panics_when_not_registered() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        let world = resources.world();
        // Should panic because TestPos is not registered
        world.components().expect_id::<TestPos>();
    }

    #[test]
    fn components_len_incremental() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        let mut world = resources.world_mut();

        assert_eq!(world.components().len(), 0);

        world.init::<TestPos>();
        assert_eq!(world.components().len(), 1);

        world.init::<TestVel>();
        assert_eq!(world.components().len(), 2);
    }

    #[test]
    fn components_get_metadata() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        let mut world = resources.world_mut();
        let id = world.init::<TestPos>();

        let meta = world.components().get(id).expect("metadata should exist");
        assert_eq!(meta.type_id(), TypeId::of::<TestPos>());
        assert!(meta.name().contains("TestPos"));
    }

    #[test]
    fn components_get_returns_none_for_invalid_index() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        let world = resources.world();
        // Use a ComponentId with an index that doesn't exist
        let fake_id = ComponentId::<u8>::new(999);
        assert!(world.components().get(fake_id).is_none());
    }

    #[test]
    fn components_archetype_variant_for_dense_storage() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        let mut world = resources.world_mut();
        let id = world.init::<TestPos>();

        let meta = world.components().get(id).expect("metadata");
        // ArchetypeStorage is not sparse, so variant should be Archetype
        assert!(matches!(meta.variant, ComponentVariant::Archetype));
    }

    #[test]
    fn components_index_operator() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        let mut world = resources.world_mut();
        let id = world.init::<TestPos>();

        // Should not panic — index uses untyped ComponentId
        let _meta = &world.components()[id.untyped()];
    }

    #[test]
    fn components_different_types_get_different_ids() {
        let mut resources = Resources::new();
        resources.install(EcsModule);
        let mut world = resources.world_mut();

        let id1 = world.init::<TestPos>();
        let id2 = world.init::<TestVel>();

        // Different component types should get different ComponentId values
        assert_ne!(id1.untyped(), id2.untyped());
    }
}
