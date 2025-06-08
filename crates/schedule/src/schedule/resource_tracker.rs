use crate::{
    resource::ResourceAccess,
    schedule::{Layer, SystemId},
};

#[derive(Clone)]
struct ResourceMutTrackerEntry {
    last_exclusive: Layer, // index if the layer, where exclusive access was requested last
    last_shared: Layer,    // index if the layer, where shared access was requested last
    systems: Vec<SystemId>, // index of the system, that had the last access.
}

impl Default for ResourceMutTrackerEntry {
    #[inline]
    fn default() -> Self {
        Self {
            last_exclusive: Layer::UNDEFINED,
            last_shared: Layer::UNDEFINED,
            systems: Vec::new(),
        }
    }
}
pub struct ResourceMutTracker(Vec<ResourceMutTrackerEntry>);

#[derive(thiserror::Error, Clone, Debug)]
pub enum ResourceConflict {
    #[error(
        "Exclusive access to resource {resource} requested by system {system_a:?} and {system_b:?} at the same time in layer {layer:?}"
    )]
    ExclusiveExclusive {
        resource: usize,
        system_a: SystemId,
        system_b: SystemId,
        layer: usize,
    },
    #[error(
        "Shared access to resource {resource} requested by system {system_shared:?} and exclusive access by system {system_exclusive:?} at the same time in layer {layer:?}"
    )]
    SharedExclusive {
        resource: usize,
        system_shared: Vec<SystemId>,
        system_exclusive: SystemId,
        layer: usize,
    },
}

impl ResourceMutTracker {
    #[inline]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    fn get_entry_mut(&mut self, resource: usize) -> &mut ResourceMutTrackerEntry {
        if self.0.len() <= resource {
            self.0
                .resize(resource + 1, ResourceMutTrackerEntry::default());
        }
        &mut self.0[resource]
    }

    fn mark_exclusive(
        &mut self,
        current_resource: usize,
        current_system: SystemId,
        current_layer: Layer,
        system_dependent_layers: &mut [Layer],
    ) -> Result<(), ResourceConflict> {
        let entry = self.get_entry_mut(current_resource);
        if entry.last_exclusive == current_layer {
            Err(ResourceConflict::ExclusiveExclusive {
                resource: current_resource,
                system_a: *entry.systems.first().unwrap(),
                system_b: current_system,
                layer: current_layer.0,
            })
        } else if entry.last_shared == current_layer {
            Err(ResourceConflict::SharedExclusive {
                resource: current_resource,
                system_shared: std::mem::take(&mut entry.systems),
                system_exclusive: current_system,
                layer: current_layer.0,
            })
        } else {
            for s in entry.systems.iter().copied() {
                if system_dependent_layers[s.0] > current_layer {
                    system_dependent_layers[s.0] = current_layer;
                }
            }
            entry.last_exclusive = current_layer;
            entry.systems.clear();
            entry.systems.push(current_system);
            Ok(())
        }
    }

    fn mark_shared(
        &mut self,
        current_resource: usize,
        current_system: SystemId,
        current_layer: Layer,
        system_dependent_layers: &mut [Layer],
    ) -> Result<(), ResourceConflict> {
        let entry = self.get_entry_mut(current_resource);
        if entry.last_exclusive == current_layer {
            Err(ResourceConflict::SharedExclusive {
                resource: current_resource,
                system_exclusive: *entry.systems.first().unwrap(),
                system_shared: vec![current_system],
                layer: current_layer.0,
            })
        } else if entry.last_exclusive > entry.last_shared {
            entry.last_shared = current_layer;
            entry.systems.clear();
            entry.systems.push(current_system);
            Ok(())
        } else {
            for s in entry.systems.iter().copied() {
                if system_dependent_layers[s.0] > current_layer {
                    system_dependent_layers[s.0] = current_layer;
                }
            }
            entry.last_shared = current_layer;
            entry.systems.push(current_system);
            Ok(())
        }
    }
    pub fn mark_access(
        &mut self,
        access: &ResourceAccess,
        current_layer: Layer,
        current_system: SystemId,
        system_dependent_layers: &mut [Layer],
    ) -> Result<(), ResourceConflict> {
        for resource in access.exclusive.iter() {
            self.mark_exclusive(
                resource,
                current_system,
                current_layer,
                system_dependent_layers,
            )?;
        }
        for resource in access.shared.iter() {
            self.mark_shared(
                resource,
                current_system,
                current_layer,
                system_dependent_layers,
            )?;
        }
        Ok(())
    }
}
