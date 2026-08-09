use pulz_schedule::prelude::Resources;

use crate::{
    WorldInner,
    component::{ComponentData, ComponentId},
    entity::Entity,
    prelude::Component,
    storage::AnyStorage,
};

impl WorldInner {
    pub fn get_components_fast(
        &self,
        entity: Entity,
    ) -> Option<impl Iterator<Item = &ComponentData>> {
        let location = self.entities.get(entity)?;
        let archetype = self.archetypes.get(location.archetype_id)?;
        Some(
            self.components
                .components
                .iter()
                .filter(move |component| component.check_archetype(archetype)),
        )
    }

    pub fn get_components(
        &self,
        res: &Resources,
        entity: Entity,
    ) -> Option<impl Iterator<Item = &ComponentData>> {
        let location = self.entities.get(entity)?;
        Some(self.get_components_fast(entity)?.filter(move |component| {
            !component.requires_detailed_storage_check
                || component.borrow_mut_any_storage(res).contains(
                    entity,
                    location.archetype_id,
                    location.index(),
                )
        }))
    }

    pub fn contains_component(
        &self,
        res: &Resources,
        entity: Entity,
        component_id: ComponentId,
    ) -> bool {
        let Some(component) = self.components.get(component_id) else {
            return false;
        };
        let Some(location) = self.entities.get(entity) else {
            return false;
        };
        let Some(archetype) = self.archetypes.get(location.archetype_id) else {
            return false;
        };
        component.check_archetype(archetype)
            && (!component.requires_detailed_storage_check
                || component.borrow_mut_any_storage(res).contains(
                    entity,
                    location.archetype_id,
                    location.index(),
                ))
    }

    pub fn try_init_component<T>(
        &mut self,
        res: &mut Resources,
    ) -> Result<ComponentId<T>, ComponentId<T>>
    where
        T: Component,
    {
        if let Some(id) = self.components.id::<T>() {
            Err(id)
        } else {
            let storage_id = res.init::<T::Storage>();
            let id = self.components.try_init::<T>(storage_id)?;
            res.init_meta_id::<dyn AnyStorage, T::Storage>(storage_id);
            Ok(id)
        }
    }

    #[inline]
    pub fn init_component<T>(&mut self, res: &mut Resources) -> ComponentId<T>
    where
        T: Component,
    {
        match self.try_init_component::<T>(res) {
            Ok(id) | Err(id) => id,
        }
    }
}
