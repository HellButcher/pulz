//! Component storage traits and built-in storage implementations.
//!
//! [`Storage`] is the typed trait; [`AnyStorage`] is its type-erased counterpart.
//! [`ArchetypeStorage`] stores data column-wise per archetype; [`SparseStorage`] uses a slot-map.

use std::any::{Any, TypeId};

use pulz_schedule::{
    impl_any_cast,
    module::system_module,
    prelude::{FromResourcesMut, ResMut, ResourceId},
    resource::Resources,
};
use slotmap::{SecondaryMap, SparseSecondaryMap};

use crate::{
    archetype::{ArchetypeId, ArchetypeMap},
    component::Component,
    entity::Entity,
};

/// Trait for typed component storage backends.
///
/// Each component type has exactly one storage resource. The storage handles
/// insert/remove/move operations driven by archetype transitions.
pub trait Storage: Send + Sync + Any + FromResourcesMut {
    /// When `true`, the storage is entity-keyed (sparse) and does not participate in archetype tracking.
    const SPARSE: bool;

    /// The component type stored by this storage.
    type Component;

    /// Returns the [`TypeId`] of the component type this storage holds.
    #[inline]
    fn component_type_id() -> TypeId {
        TypeId::of::<Self::Component>()
    }

    /// Borrows this storage as a type-erased [`AnyStorage`] via the resource container.
    fn borrow_mut_any(res: &Resources, id: ResourceId) -> ResMut<'_, dyn AnyStorage>
    where
        Self: Sized,
    {
        let storage = res
            .borrow_res_mut_id::<Self>(id.typed::<Self>())
            .expect("storage resource not initialized");
        ResMut::map(storage, |s| {
            let casted: &mut dyn AnyStorage = s;
            casted
        })
    }

    /// Returns `true` if the given entity has this component.
    fn contains(&self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool;

    /// Removes the component by swap-removing its slot and returns the value if present.
    fn swap_remove(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        index: usize,
    ) -> Option<Self::Component>;

    /// Updates the archetype of the component by swapping it from one archetype to another.
    fn swap_remove_and_push(
        &mut self,
        remove_from_archetype: ArchetypeId,
        remove_from_index: usize,
        push_to_archetype: ArchetypeId,
    ) -> Option<usize>;

    /// Non-Archetype storage operations:
    /// - `insert` directly inserts the value into the storage.
    ///
    /// Archetype storage operations:
    /// - first, `insert` is used to store the value in a temporary location;
    /// - then, the `flush_push` or `flush_replace` method is called to update the value when the final archetype is known.
    ///
    /// Stores a new component value. For archetype storage this is a temporary slot pending `flush_*`.
    fn insert(&mut self, entity: Entity, value: Self::Component);
    /// Moves the pending value from the temporary slot to an existing archetype row.
    fn flush_replace(&mut self, archetype: ArchetypeId, index: usize) -> bool;
    /// Appends the pending value to the end of the archetype's column; returns the new row index.
    fn flush_push(&mut self, archetype: ArchetypeId) -> Option<usize>;

    /// Returns a shared reference to the component value, if present.
    fn get(&self, entity: Entity, archetype: ArchetypeId, index: usize)
    -> Option<&Self::Component>;

    /// Returns an exclusive reference to the component value, if present.
    fn get_mut(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        index: usize,
    ) -> Option<&mut Self::Component>;
}

/// Type-erased counterpart of [`Storage`], used when the component type is not statically known.
pub trait AnyStorage: Send + Sync + Any {
    /// Returns `true` if the given entity has this component.
    fn contains(&self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool;

    /// Moves a component from one archetype column to another via swap-remove.
    fn swap_remove_and_push(
        &mut self,
        remove_from_archetype: ArchetypeId,
        remove_from_index: usize,
        push_to_archetype: ArchetypeId,
    ) -> Option<usize>;

    /// Removes the component in place and returns `true` if it was present.
    fn swap_remove(&mut self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool;
    /// Replaces a pending inserted value at the given archetype slot; returns `true` on success.
    fn flush_replace(&mut self, archetype: ArchetypeId, index: usize) -> bool;
    /// Moves a pending inserted value to the end of the given archetype's column; returns the new index.
    fn flush_push(&mut self, archetype: ArchetypeId) -> Option<usize>;
}

impl<S: Storage> AnyStorage for S {
    fn contains(&self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool {
        S::contains(self, entity, archetype, index)
    }

    fn swap_remove_and_push(
        &mut self,
        remove_from_archetype: ArchetypeId,
        remove_from_index: usize,
        push_to_archetype: ArchetypeId,
    ) -> Option<usize> {
        S::swap_remove_and_push(
            self,
            remove_from_archetype,
            remove_from_index,
            push_to_archetype,
        )
    }

    fn swap_remove(&mut self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool {
        S::swap_remove(self, entity, archetype, index).is_some()
    }

    fn flush_replace(&mut self, archetype: ArchetypeId, index: usize) -> bool {
        S::flush_replace(self, archetype, index)
    }

    fn flush_push(&mut self, archetype: ArchetypeId) -> Option<usize> {
        S::flush_push(self, archetype)
    }
}

impl_any_cast!(dyn AnyStorage);

/// Column-oriented storage for archetype-tracked components.
///
/// Stores component data in per-archetype `Vec<T>` columns at the same index as the entity's
/// row in the archetype. A temporary slot (`tmp`) holds a value between `insert` and `flush_*`.
pub struct ArchetypeStorage<T> {
    data: ArchetypeMap<Vec<T>>,
    tmp: Option<T>,
}

/// Dense entity-keyed storage backed by a [`SecondaryMap`].
pub type SlotStorage<T> = SecondaryMap<Entity, T>;
/// Sparse entity-keyed storage backed by a [`SparseSecondaryMap`]; component presence does not affect archetype.
pub type SparseStorage<T> = SparseSecondaryMap<Entity, T>;

impl<T> ArchetypeStorage<T> {
    /// Creates a new empty archetype storage.
    #[inline]
    pub const fn new() -> Self {
        Self {
            data: ArchetypeMap::new(),
            tmp: None,
        }
    }
}

impl<T> Default for ArchetypeStorage<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Storage for ArchetypeStorage<T>
where
    T: Component<Storage = Self>,
{
    const SPARSE: bool = false;
    type Component = T;

    #[inline]
    fn contains(&self, _entity: Entity, archetype: ArchetypeId, index: usize) -> bool {
        self.data
            .get(archetype)
            .is_some_and(|col| index < col.len())
    }

    #[inline]
    fn swap_remove(&mut self, _entity: Entity, archetype: ArchetypeId, index: usize) -> Option<T> {
        self.tmp = None;
        if let Some(col) = self.data.get_mut(archetype)
            && index < col.len()
        {
            return Some(col.swap_remove(index));
        }
        None
    }

    fn insert(&mut self, _entity: Entity, value: Self::Component) {
        self.tmp = Some(value);
    }

    fn flush_replace(&mut self, archetype_id: ArchetypeId, index: usize) -> bool {
        let Some(cell) = self
            .data
            .get_mut(archetype_id)
            .and_then(|col| col.get_mut(index))
        else {
            return false; // archetype and index not present — tmp still has the value for flush_push
        };
        let Some(value) = self.tmp.take() else {
            return false;
        };
        *cell = value;
        true
    }

    fn flush_push(&mut self, archetype: ArchetypeId) -> Option<usize> {
        let value = self.tmp.take()?;
        let col = self.data.get_or_insert_default(archetype);
        let index = col.len();
        col.push(value);
        Some(index)
    }

    fn swap_remove_and_push(
        &mut self,
        remove_from_archetype: ArchetypeId,
        remove_from_index: usize,
        push_to_archetype: ArchetypeId,
    ) -> Option<usize> {
        if remove_from_archetype == push_to_archetype {
            return None;
        }
        let col = self.data.get_mut(remove_from_archetype)?;
        if remove_from_index >= col.len() {
            return None;
        }
        let removed_value = col.swap_remove(remove_from_index);
        let col = self.data.get_or_insert_default(push_to_archetype);
        let index = col.len();
        col.push(removed_value);
        Some(index)
    }

    #[inline]
    fn get(
        &self,
        _entity: Entity,
        archetype_id: ArchetypeId,
        index: usize,
    ) -> Option<&Self::Component> {
        self.data.get(archetype_id)?.get(index)
    }

    #[inline]
    fn get_mut(
        &mut self,
        _entity: Entity,
        archetype_id: ArchetypeId,
        index: usize,
    ) -> Option<&mut Self::Component> {
        self.data.get_mut(archetype_id)?.get_mut(index)
    }
}

impl<T> Storage for SparseStorage<T>
where
    T: Component<Storage = Self>,
{
    const SPARSE: bool = true;
    type Component = T;

    #[inline]
    fn contains(&self, entity: Entity, _archetype: ArchetypeId, _index: usize) -> bool {
        self.contains_key(entity)
    }

    #[inline]
    fn swap_remove(&mut self, entity: Entity, _archetype: ArchetypeId, _index: usize) -> Option<T> {
        self.remove(entity)
    }

    #[inline]
    fn insert(&mut self, entity: Entity, value: T) {
        self.insert(entity, value);
    }

    #[inline]
    fn flush_replace(&mut self, _archetype: ArchetypeId, _index: usize) -> bool {
        true
    }

    #[inline]
    fn flush_push(&mut self, _archetype: ArchetypeId) -> Option<usize> {
        None
    }

    #[inline]
    fn swap_remove_and_push(
        &mut self,
        _remove_from_archetype: ArchetypeId,
        _remove_from_index: usize,
        _push_to_archetype: ArchetypeId,
    ) -> Option<usize> {
        None
    }

    #[inline]
    fn get(
        &self,
        entity: Entity,
        _archetype: ArchetypeId,
        _index: usize,
    ) -> Option<&Self::Component> {
        self.get(entity)
    }

    #[inline]
    fn get_mut(
        &mut self,
        entity: Entity,
        _archetype: ArchetypeId,
        _index: usize,
    ) -> Option<&mut Self::Component> {
        self.get_mut(entity)
    }
}

/// Storage wrapper that records entities whose component was removed this frame.
///
/// Wraps any [`Storage`] and appends removed entities to a sorted `removed` list on each
/// `swap_remove`. A system registered via [`Tracked::install_systems`] clears the list each frame.
pub struct Tracked<S> {
    base: S,
    pub(crate) removed: Vec<Entity>,
}

#[system_module(install_fn = install_systems_impl)]
impl<S: Storage> Tracked<S> {
    #[system]
    fn reset(&mut self) {
        self.removed.clear();
    }
}

impl<S: FromResourcesMut> FromResourcesMut for Tracked<S> {
    #[inline]
    fn from_resources_mut(resources: &mut Resources) -> Self {
        Self {
            base: S::from_resources_mut(resources),
            removed: Vec::new(),
        }
    }
}

impl<S: Storage> Storage for Tracked<S> {
    const SPARSE: bool = S::SPARSE;
    type Component = S::Component;

    #[inline]
    fn contains(&self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool {
        self.base.contains(entity, archetype, index)
    }

    #[inline]
    fn swap_remove(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        index: usize,
    ) -> Option<Self::Component> {
        let old = self.base.swap_remove(entity, archetype, index)?;
        insert_sorted(&mut self.removed, entity);
        Some(old)
    }

    #[inline]
    fn insert(&mut self, entity: Entity, value: Self::Component) {
        self.base.insert(entity, value)
    }

    #[inline]
    fn flush_replace(&mut self, archetype: ArchetypeId, index: usize) -> bool {
        self.base.flush_replace(archetype, index)
    }

    #[inline]
    fn flush_push(&mut self, archetype: ArchetypeId) -> Option<usize> {
        self.base.flush_push(archetype)
    }

    #[inline]
    fn swap_remove_and_push(
        &mut self,
        remove_from_archetype: ArchetypeId,
        remove_from_index: usize,
        push_to_archetype: ArchetypeId,
    ) -> Option<usize> {
        self.base
            .swap_remove_and_push(remove_from_archetype, remove_from_index, push_to_archetype)
    }

    #[inline]
    fn get(
        &self,
        entity: Entity,
        archetype: ArchetypeId,
        index: usize,
    ) -> Option<&Self::Component> {
        self.base.get(entity, archetype, index)
    }

    #[inline]
    fn get_mut(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        index: usize,
    ) -> Option<&mut Self::Component> {
        self.base.get_mut(entity, archetype, index)
    }
}

fn insert_sorted<T: Ord>(vec: &mut Vec<T>, value: T) {
    if let Err(pos) = vec.binary_search(&value) {
        vec.insert(pos, value);
    }
}
