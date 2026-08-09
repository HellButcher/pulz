//! Component registry that maps types to their metadata and storage resources.

use std::{
    any::TypeId,
    borrow::Cow,
    collections::{BTreeMap, btree_map::Entry},
    ops::Index,
};

use pulz_schedule::{
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
    by_type_id: BTreeMap<TypeId, ComponentId>,
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
            by_type_id: BTreeMap::new(),
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
