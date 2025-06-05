use crate::resource::ResourceAccess;

#[derive(Clone)]
struct ResourceMutTrackerEntry {
    last_exclusive: usize, // index if the group, where exclusive access was requested last
    last_shared: usize,    // index if the group, where shared access was requested last
    systems: Vec<usize>,   // index of the system, that had the last access.
}

impl Default for ResourceMutTrackerEntry {
    #[inline]
    fn default() -> Self {
        Self {
            last_exclusive: !0,
            last_shared: !0,
            systems: Vec::new(),
        }
    }
}
pub struct ResourceMutTracker(Vec<ResourceMutTrackerEntry>);

#[derive(Clone, Debug)]
pub enum ResourceConflict {
    #[allow(unused)] // used for Debug
    ExclusiveExclusive {
        resource: usize,
        system_a: usize,
        system_b: usize,
    },
    #[allow(unused)] // used for Debug
    SharedExclusive {
        resource: usize,
        system_shared: Vec<usize>,
        system_exclusive: usize,
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
        resource: usize,
        current_group: usize,
        system: usize,
        result: &mut [usize],
    ) -> Result<(), ResourceConflict> {
        let entry = self.get_entry_mut(resource);
        if entry.last_exclusive == current_group {
            Err(ResourceConflict::ExclusiveExclusive {
                resource,
                system_a: *entry.systems.first().unwrap(),
                system_b: system,
            })
        } else if entry.last_shared == current_group {
            Err(ResourceConflict::SharedExclusive {
                resource,
                system_shared: std::mem::take(&mut entry.systems),
                system_exclusive: system,
            })
        } else {
            for s in entry.systems.iter().copied() {
                if result[s] > current_group {
                    result[s] = current_group;
                }
            }
            entry.last_exclusive = current_group;
            entry.systems.clear();
            entry.systems.push(system);
            Ok(())
        }
    }

    fn mark_shared(
        &mut self,
        resource: usize,
        current_group: usize,
        system: usize,
        result: &mut [usize],
    ) -> Result<(), ResourceConflict> {
        let entry = self.get_entry_mut(resource);
        if entry.last_exclusive == current_group {
            Err(ResourceConflict::SharedExclusive {
                resource,
                system_exclusive: *entry.systems.first().unwrap(),
                system_shared: vec![system],
            })
        } else if entry.last_exclusive > entry.last_shared {
            entry.last_shared = current_group;
            entry.systems.clear();
            entry.systems.push(system);
            Ok(())
        } else {
            for s in entry.systems.iter().copied() {
                if result[s] > current_group {
                    result[s] = current_group;
                }
            }
            entry.last_shared = current_group;
            entry.systems.push(system);
            Ok(())
        }
    }
    pub fn mark_access(
        &mut self,
        access: &ResourceAccess,
        current_group: usize,
        system: usize,
        result: &mut [usize],
    ) -> Result<(), ResourceConflict> {
        for resource in access.exclusive.iter() {
            self.mark_exclusive(resource, current_group, system, result)?;
        }
        for resource in access.shared.iter() {
            self.mark_shared(resource, current_group, system, result)?;
        }
        Ok(())
    }
}
