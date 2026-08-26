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
                let dep_layer = &mut system_dependent_layers[s.0];
                if *dep_layer == Layer::UNDEFINED || current_layer.0 < dep_layer.0 {
                    *dep_layer = current_layer;
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
        } else if entry.last_exclusive != Layer::UNDEFINED
            && entry.last_exclusive > entry.last_shared
        {
            // An exclusive access happened after the last shared access — invalidate previous shared systems
            entry.last_shared = current_layer;
            entry.systems.clear();
            entry.systems.push(current_system);
            Ok(())
        } else {
            for s in entry.systems.iter().copied() {
                // Update dependent layer if it's UNDEFINED or current_layer is less
                let dep_layer = &mut system_dependent_layers[s.0];
                if *dep_layer == Layer::UNDEFINED || current_layer.0 < dep_layer.0 {
                    *dep_layer = current_layer;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_shared_access(resource_idx: usize) -> ResourceAccess {
        let mut access = ResourceAccess::new();
        // We need to manually insert into the shared BitSet
        // Since ResourceAccess::shared is pub(crate), we can access it
        access.shared.insert(resource_idx);
        access
    }

    fn make_exclusive_access(resource_idx: usize) -> ResourceAccess {
        let mut access = ResourceAccess::new();
        access.exclusive.insert(resource_idx);
        access
    }

    // --- Exclusive-exclusive conflict ---

    #[test]
    fn tracker_no_conflict_sequential_layers() {
        let mut tracker = ResourceMutTracker::new();
        let mut dependent = vec![Layer::UNDEFINED; 2];
        // Layer 0: exclusive on resource 0
        tracker
            .mark_access(
                &make_exclusive_access(0),
                Layer(0),
                SystemId(0),
                &mut dependent,
            )
            .unwrap();
        // Layer 1: exclusive on resource 0 — OK, different layer
        tracker
            .mark_access(
                &make_exclusive_access(0),
                Layer(1),
                SystemId(1),
                &mut dependent,
            )
            .unwrap();
    }

    #[test]
    fn tracker_exclusive_exclusive_same_layer_conflict() {
        let mut tracker = ResourceMutTracker::new();
        let mut dependent = vec![Layer::UNDEFINED; 2];
        // Layer 0: exclusive on resource 0 by system 0
        tracker
            .mark_access(
                &make_exclusive_access(0),
                Layer(0),
                SystemId(0),
                &mut dependent,
            )
            .unwrap();
        // Layer 0: exclusive on resource 0 by system 1 — CONFLICT
        let result = tracker.mark_access(
            &make_exclusive_access(0),
            Layer(0),
            SystemId(1),
            &mut dependent,
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            ResourceConflict::ExclusiveExclusive {
                resource: 0,
                system_a: SystemId(0),
                system_b: SystemId(1),
                layer: 0,
            } => {}
            other => panic!("unexpected conflict: {other:?}"),
        }
    }

    // --- Shared-exclusive conflict ---

    #[test]
    fn tracker_shared_exclusive_same_layer_conflict() {
        let mut tracker = ResourceMutTracker::new();
        let mut dependent = vec![Layer::UNDEFINED; 2];
        // Layer 0: shared on resource 0 by system 0
        tracker
            .mark_access(
                &make_shared_access(0),
                Layer(0),
                SystemId(0),
                &mut dependent,
            )
            .unwrap();
        // Layer 0: exclusive on resource 0 by system 1 — CONFLICT
        let result = tracker.mark_access(
            &make_exclusive_access(0),
            Layer(0),
            SystemId(1),
            &mut dependent,
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            ResourceConflict::SharedExclusive {
                resource: 0,
                system_shared,
                system_exclusive: SystemId(1),
                layer: 0,
            } => {
                assert_eq!(system_shared, vec![SystemId(0)]);
            }
            other => panic!("unexpected conflict: {other:?}"),
        }
    }

    #[test]
    fn tracker_exclusive_shared_same_layer_conflict() {
        let mut tracker = ResourceMutTracker::new();
        let mut dependent = vec![Layer::UNDEFINED; 2];
        // Layer 0: exclusive on resource 0 by system 0
        tracker
            .mark_access(
                &make_exclusive_access(0),
                Layer(0),
                SystemId(0),
                &mut dependent,
            )
            .unwrap();
        // Layer 0: shared on resource 0 by system 1 — CONFLICT
        let result = tracker.mark_access(
            &make_shared_access(0),
            Layer(0),
            SystemId(1),
            &mut dependent,
        );
        assert!(result.is_err());
    }

    // --- Safe patterns ---

    #[test]
    fn tracker_exclusive_then_shared_next_layer_ok() {
        let mut tracker = ResourceMutTracker::new();
        let mut dependent = vec![Layer::UNDEFINED; 2];
        // Layer 0: exclusive on resource 0
        tracker
            .mark_access(
                &make_exclusive_access(0),
                Layer(0),
                SystemId(0),
                &mut dependent,
            )
            .unwrap();
        // Layer 1: shared on resource 0 — OK
        tracker
            .mark_access(
                &make_shared_access(0),
                Layer(1),
                SystemId(1),
                &mut dependent,
            )
            .unwrap();
    }

    #[test]
    fn tracker_shared_then_exclusive_next_layer_ok() {
        let mut tracker = ResourceMutTracker::new();
        let mut dependent = vec![Layer::UNDEFINED; 2];
        // Layer 0: shared on resource 0
        tracker
            .mark_access(
                &make_shared_access(0),
                Layer(0),
                SystemId(0),
                &mut dependent,
            )
            .unwrap();
        // Layer 1: exclusive on resource 0 — OK
        tracker
            .mark_access(
                &make_exclusive_access(0),
                Layer(1),
                SystemId(1),
                &mut dependent,
            )
            .unwrap();
    }

    #[test]
    fn tracker_multiple_shared_same_layer_ok() {
        let mut tracker = ResourceMutTracker::new();
        let mut dependent = vec![Layer::UNDEFINED; 3];
        // Layer 0: shared on resource 0 by systems 0 and 1
        tracker
            .mark_access(
                &make_shared_access(0),
                Layer(0),
                SystemId(0),
                &mut dependent,
            )
            .unwrap();
        tracker
            .mark_access(
                &make_shared_access(0),
                Layer(0),
                SystemId(1),
                &mut dependent,
            )
            .unwrap();
    }

    #[test]
    fn tracker_different_resources_no_conflict() {
        let mut tracker = ResourceMutTracker::new();
        let mut dependent = vec![Layer::UNDEFINED; 2];
        // Layer 0: exclusive on resource 0 by system 0
        tracker
            .mark_access(
                &make_exclusive_access(0),
                Layer(0),
                SystemId(0),
                &mut dependent,
            )
            .unwrap();
        // Layer 0: exclusive on resource 1 by system 1 — OK, different resource
        tracker
            .mark_access(
                &make_exclusive_access(1),
                Layer(0),
                SystemId(1),
                &mut dependent,
            )
            .unwrap();
    }

    // --- Dependent layer tracking ---

    #[test]
    fn tracker_shared_dependent_layer_updated() {
        let mut tracker = ResourceMutTracker::new();
        let mut dependent = vec![Layer::UNDEFINED; 2];
        // Layer 0: shared on resource 0 by system 0
        tracker
            .mark_access(
                &make_shared_access(0),
                Layer(0),
                SystemId(0),
                &mut dependent,
            )
            .unwrap();
        // Layer 1: shared on resource 0 by system 1
        tracker
            .mark_access(
                &make_shared_access(0),
                Layer(1),
                SystemId(1),
                &mut dependent,
            )
            .unwrap();
        // System 0's dependent layer should be updated to 1
        assert_eq!(dependent[0], Layer(1));
    }

    // --- Resource tracking entries ---

    #[test]
    fn tracker_exclusive_cleans_shared_systems() {
        let mut tracker = ResourceMutTracker::new();
        let mut dependent = vec![Layer::UNDEFINED; 2];
        // Layer 0: shared on resource 0 by system 0
        tracker
            .mark_access(
                &make_shared_access(0),
                Layer(0),
                SystemId(0),
                &mut dependent,
            )
            .unwrap();
        // Layer 1: exclusive on resource 0 by system 1 — should replace shared tracking
        tracker
            .mark_access(
                &make_exclusive_access(0),
                Layer(1),
                SystemId(1),
                &mut dependent,
            )
            .unwrap();
        // Layer 2: shared on resource 0 by system 2 — should conflict with exclusive
        let result = tracker.mark_access(
            &make_shared_access(0),
            Layer(1),
            SystemId(2),
            &mut dependent,
        );
        assert!(result.is_err());
    }
}
