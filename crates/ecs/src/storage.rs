use std::any::{Any, TypeId};

use pulz_schedule::{
    impl_any_cast, label::CoreSystemPhase, resource::Resources, schedule::Schedule,
};
use slotmap::{SecondaryMap, SparseSecondaryMap};

use crate::{
    Entity,
    archetype::{Archetype, ArchetypeId},
    component::ComponentDetails,
    insert_sorted,
    resource::FromResourcesMut,
};

pub trait Storage: Send + Sync + Any + FromResourcesMut {
    const SPARSE: bool;

    type Component;

    #[inline]
    fn install_systems(_schedule: &mut Schedule) {}

    #[inline]
    fn component_type_id() -> TypeId {
        TypeId::of::<Self::Component>()
    }

    fn fast_contains(
        res: &Resources,
        entity: Entity,
        component: &ComponentDetails,
        archetype: &Archetype,
    ) -> bool;

    fn contains(&self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool;
    fn swap_remove(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        index: usize,
    ) -> Option<Self::Component>;

    fn insert(&mut self, entity: Entity, value: Self::Component);

    fn flush_replace(&mut self, archetype: ArchetypeId, index: usize) -> bool;
    fn flush_push(&mut self, archetype: ArchetypeId) -> Option<usize>;

    fn swap_remove_and_insert(
        &mut self,
        remove_from_archetype: ArchetypeId,
        remove_from_index: usize,
        insert_to_archetype: ArchetypeId,
    ) -> Option<usize>;

    fn get(&self, entity: Entity, archetype: ArchetypeId, index: usize)
    -> Option<&Self::Component>;

    fn get_mut(
        &mut self,
        entity: Entity,
        archetype: ArchetypeId,
        index: usize,
    ) -> Option<&mut Self::Component>;
}

pub trait AnyStorage: Send + Sync + Any {
    fn component_type_id(&self) -> TypeId;
    fn contains(&self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool;
    fn swap_remove(&mut self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool;

    fn flush_replace(&mut self, archetype: ArchetypeId, index: usize) -> bool;
    fn flush_push(&mut self, archetype: ArchetypeId) -> Option<usize>;

    fn swap_remove_and_insert(
        &mut self,
        remove_from_archetype: ArchetypeId,
        remove_from_index: usize,
        insert_to_archetype: ArchetypeId,
    ) -> Option<usize>;
}

impl_any_cast!(dyn AnyStorage);

pub struct ArchetypeStorage<T> {
    data: Vec<Vec<T>>,
    tmp: Option<T>,
}

pub type SlotStorage<T> = SecondaryMap<Entity, T>;
pub type SparseStorage<T> = SparseSecondaryMap<Entity, T>;

#[deprecated]
pub type DenseStorage<T> = ArchetypeStorage<T>;
#[deprecated]
pub type HashMapStorage<T> = SparseStorage<T>;

impl<T> Default for ArchetypeStorage<T> {
    #[inline]
    fn default() -> Self {
        Self {
            data: Vec::new(),
            tmp: None,
        }
    }
}

fn vec_make_available<T: Default>(vec: &mut Vec<T>, index: usize) -> &mut T {
    if vec.len() <= index {
        vec.resize_with(index + 1, Default::default);
    }
    // SAFETY: was resized if length was to short
    unsafe { vec.get_unchecked_mut(index) }
}

impl<T> Storage for ArchetypeStorage<T>
where
    T: Send + Sync + 'static,
{
    const SPARSE: bool = false;
    type Component = T;

    #[inline]
    fn fast_contains(
        _res: &Resources,
        _entity: Entity,
        component: &ComponentDetails,
        archetype: &Archetype,
    ) -> bool {
        archetype.components.contains(component.id())
    }

    #[inline]
    fn contains(&self, _entity: Entity, archetype: ArchetypeId, index: usize) -> bool {
        self.data
            .get(archetype.index())
            .is_some_and(|col| index < col.len())
    }

    #[inline]
    fn swap_remove(&mut self, _entity: Entity, archetype: ArchetypeId, index: usize) -> Option<T> {
        self.tmp = None;
        if let Some(col) = self.data.get_mut(archetype.index()) {
            if index < col.len() {
                return Some(col.swap_remove(index));
            }
        }
        None
    }

    #[inline]
    fn insert(&mut self, _entity: Entity, value: T) {
        self.tmp.replace(value);
    }

    fn flush_replace(&mut self, archetype: ArchetypeId, index: usize) -> bool {
        let Some(cell) = self
            .data
            .get_mut(archetype.index())
            .and_then(|c| c.get_mut(index))
        else {
            return false;
        };
        if let Some(value) = self.tmp.take() {
            *cell = value;
            true
        } else {
            false
        }
    }

    fn flush_push(&mut self, archetype: ArchetypeId) -> Option<usize> {
        let Some(value) = self.tmp.take() else {
            return None;
        };
        let col = vec_make_available(&mut self.data, archetype.index());
        let index = col.len();
        col.push(value);
        Some(index)
    }

    fn swap_remove_and_insert(
        &mut self,
        remove_from_archetype: ArchetypeId,
        remove_from_index: usize,
        insert_to_archetype: ArchetypeId,
    ) -> Option<usize> {
        if remove_from_archetype == insert_to_archetype {
            return None;
        }
        let Some(col) = self.data.get_mut(remove_from_archetype.index()) else {
            return None;
        };
        if remove_from_index >= col.len() {
            return None;
        }
        let removed_value = col.swap_remove(remove_from_index);
        let col = vec_make_available(&mut self.data, insert_to_archetype.index());
        let index = col.len();
        col.push(removed_value);
        Some(index)
    }

    #[inline]
    fn get(
        &self,
        _entity: Entity,
        archetype: ArchetypeId,
        index: usize,
    ) -> Option<&Self::Component> {
        self.data.get(archetype.index())?.get(index)
    }

    #[inline]
    fn get_mut(
        &mut self,
        _entity: Entity,
        archetype: ArchetypeId,
        index: usize,
    ) -> Option<&mut Self::Component> {
        self.data.get_mut(archetype.index())?.get_mut(index)
    }
}

impl<T> Storage for SparseStorage<T>
where
    T: Send + Sync + 'static,
{
    const SPARSE: bool = true;
    type Component = T;

    #[inline]
    fn fast_contains(
        res: &Resources,
        entity: Entity,
        component: &ComponentDetails,
        _archetype: &Archetype,
    ) -> bool {
        res.borrow_res_id(component.storage_id.typed::<Self>())
            .is_some_and(|s| s.contains_key(entity))
    }

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
    fn swap_remove_and_insert(
        &mut self,
        _remove_from_archetype: ArchetypeId,
        _remove_from_index: usize,
        _insert_to_archetype: ArchetypeId,
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

pub struct Tracked<S> {
    base: S,
    pub(crate) removed: Vec<Entity>,
}

impl<S> Tracked<S> {
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

    fn install_systems(schedule: &mut Schedule) {
        schedule
            .add_system(Self::reset)
            .into_phase(CoreSystemPhase::First);
    }

    #[inline]
    fn fast_contains(
        res: &Resources,
        entity: Entity,
        component: &ComponentDetails,
        archetype: &Archetype,
    ) -> bool {
        S::fast_contains(res, entity, component, archetype)
    }

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
    fn swap_remove_and_insert(
        &mut self,
        remove_from_archetype: ArchetypeId,
        remove_from_index: usize,
        insert_to_archetype: ArchetypeId,
    ) -> Option<usize> {
        self.base.swap_remove_and_insert(
            remove_from_archetype,
            remove_from_index,
            insert_to_archetype,
        )
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

impl<S> AnyStorage for S
where
    S: Storage,
{
    fn component_type_id(&self) -> TypeId {
        S::component_type_id()
    }

    fn contains(&self, entity: Entity, archetype: ArchetypeId, index: usize) -> bool {
        S::contains(self, entity, archetype, index)
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

    fn swap_remove_and_insert(
        &mut self,
        remove_from_archetype: ArchetypeId,
        remove_from_index: usize,
        insert_to_archetype: ArchetypeId,
    ) -> Option<usize> {
        S::swap_remove_and_insert(
            self,
            remove_from_archetype,
            remove_from_index,
            insert_to_archetype,
        )
    }
}
