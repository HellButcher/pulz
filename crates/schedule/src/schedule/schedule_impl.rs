use std::{
    collections::{BTreeMap, btree_map::Entry},
    ops::RangeBounds,
};

use crate::{
    atom::Atom,
    label::{SystemSet, SystemSetId},
    prelude::{Resources, Schedule},
    resource::ResourceAccess,
    schedule::{
        Layer, ScheduleError, SystemId,
        graph::{Graph, NodeId},
        resource_tracker::{ResourceConflict, ResourceMutTracker},
    },
    system::{BoxedSystem, ExclusiveSystem, IntoSystem, SendSystem, System, SystemInit},
    threadpool::ThreadPool,
    util::DirtyVersion,
};

impl Schedule {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            phase_labels: BTreeMap::new(),
            access: Vec::new(),
            graph: Graph::new(),
            ordered_layers: Vec::new(),
            system_dependent_layers: Vec::new(),
            #[cfg(not(target_os = "unknown"))]
            threadpool_id: None,
            atom: Atom::ZERO,
            version: DirtyVersion::new(),
        }
    }

    #[inline]
    pub fn add_system<S, Marker>(&mut self, system: S) -> ScheduleNodeBuilder<'_>
    where
        S: IntoSystem<Marker>,
        S::System: SendSystem,
    {
        self._add_system(BoxedSystem::Send(Box::new(system.into_system())))
    }

    #[inline]
    pub fn add_system_unsend<S, Marker>(&mut self, system: S) -> ScheduleNodeBuilder<'_>
    where
        S: IntoSystem<Marker>,
        S::System: System,
    {
        self._add_system(BoxedSystem::Unsend(Box::new(system.into_system())))
    }

    #[inline]
    pub fn add_system_exclusive<S, Marker>(&mut self, system: S) -> ScheduleNodeBuilder<'_>
    where
        S: IntoSystem<Marker>,
    {
        self._add_system(BoxedSystem::Exclusive(Box::new(system.into_system())))
    }

    fn _add_system(&mut self, system: BoxedSystem) -> ScheduleNodeBuilder<'_> {
        self.version.dirty();
        let index = self.systems.len();
        let label = system.system_label().as_label();
        self.systems.push(system);
        self.access.push(ResourceAccess::new());
        let id = SystemId(index);
        let node_id = self._get_or_set_label(label, id);
        ScheduleNodeBuilder {
            schedule: self,
            label,
            node_id,
        }
    }

    fn _get_or_set_label(&mut self, label: SystemSetId, system_id: SystemId) -> NodeId {
        match self.phase_labels.entry(label) {
            // if label already exists, use existing node
            Entry::Occupied(entry) => {
                let node_id = *entry.get();
                let node = &mut self.graph[node_id];
                if system_id.is_defined() {
                    assert_eq!(
                        node.system,
                        SystemId::UNDEFINED,
                        "system label already used for another system"
                    );
                    node.system = system_id;
                }
                node_id
            }
            // otherwise create a new phase node
            Entry::Vacant(entry) => {
                let (node_id, _) = self.graph.insert(system_id);
                entry.insert(node_id);
                node_id
            }
        }
    }

    #[inline]
    pub fn chain(&mut self, phases: impl IntoIterator<Item = impl SystemSet>) {
        self._chain(phases.into_iter().map(|l| l.as_label()));
    }
    fn _chain(&mut self, mut phases: impl Iterator<Item = SystemSetId>) {
        // get index of first entry of the sequence
        let Some(phase_label) = phases.next() else {
            return;
        };
        self.version.dirty();
        let mut prev_node_id = self._get_or_set_label(phase_label, SystemId::UNDEFINED);
        // handle rest of chain
        for phase_label in phases {
            let current_node_id = self._get_or_set_label(phase_label, SystemId::UNDEFINED);
            self.graph.add_dependency(prev_node_id, current_node_id);
            prev_node_id = current_node_id;
        }
    }

    #[inline]
    pub fn add_dependency(&mut self, dependency: impl SystemSet, dependent: impl SystemSet) {
        self._add_dependency(dependency.as_label(), dependent.as_label());
    }

    fn _add_dependency(&mut self, dependency: SystemSetId, dependent: SystemSetId) {
        self.version.dirty();
        let dependency_node_id = self._get_or_set_label(dependency, SystemId::UNDEFINED);
        let dependent_node_id = self._get_or_set_label(dependent, SystemId::UNDEFINED);
        self.graph
            .add_dependency(dependency_node_id, dependent_node_id);
    }

    fn split_exclusive(&self, layers: &mut Vec<Vec<SystemId>>, dependent_layers: &mut [Layer]) {
        let mut shared_waiting_until_layer = 0; // latest layer until when shared systems are schedules
        let mut exclusive_dependent_layer_min = !0; // earliest layer, until an exclusive system from tmp_exclusive MUST be scheduled
        let mut tmp_exclusive = Vec::new();
        let mut layer_index = 0;
        while layer_index < layers.len() {
            let layer_systems = &mut layers[layer_index];
            if layer_systems.len() > 1 {
                let mut shared_dependent_layer_max = shared_waiting_until_layer;
                // collect and remove exclusive systems
                let mut i = 0;
                while i < layer_systems.len() {
                    let system_id = layer_systems[i];
                    let system = &self.systems[system_id.0];
                    let dependent_layer = dependent_layers[system_id.0];
                    if system.is_exclusive() {
                        if exclusive_dependent_layer_min > dependent_layer.0 {
                            exclusive_dependent_layer_min = dependent_layer.0;
                        }
                        // exclusive systems are moved to the end of the layer
                        tmp_exclusive.push(system_id);
                        layer_systems.swap_remove(i);
                        continue;
                    }
                    if shared_dependent_layer_max < dependent_layer.0 {
                        shared_dependent_layer_max = dependent_layer.0;
                    }
                    i += 1;
                }

                // place exclusive systems in a seperate layer at a reasonable position
                if !tmp_exclusive.is_empty() {
                    if layer_systems.is_empty() {
                        // layer consisted only of exclusive systems, so restore the layer
                        layer_systems.append(&mut tmp_exclusive);
                        exclusive_dependent_layer_min = !0;
                    } else if shared_waiting_until_layer <= layer_index {
                        // currently not waiting for shared systems, so place it before the this layer
                        layers.insert(layer_index, std::mem::take(&mut tmp_exclusive));
                        exclusive_dependent_layer_min = !0;
                        shift_values(dependent_layers, layer_index.., 1);
                    }
                }
                shared_waiting_until_layer = shared_dependent_layer_max;
                if exclusive_dependent_layer_min < shared_waiting_until_layer {
                    exclusive_dependent_layer_min = shared_waiting_until_layer;
                }
            }
            layer_index += 1;
        }
    }

    fn resolve_resource_conflicts(
        &self,
        layers: &mut Vec<Vec<SystemId>>,
        dependent_layers: &mut [Layer],
    ) -> Result<(), ResourceConflict> {
        let mut resource_tracker = ResourceMutTracker::new();
        let mut layer_index = 0;
        while layer_index < layers.len() {
            let layer_systems = &mut layers[layer_index];
            let mut tmp_exclusive = Vec::new();
            let mut tmp_shared = Vec::new();
            for &system_id in layer_systems.iter() {
                let access = &self.access[system_id.0];
                match resource_tracker.mark_access(
                    access,
                    Layer(layer_index),
                    system_id,
                    dependent_layers,
                ) {
                    Ok(()) => {} // no conflict, continue
                    Err(ResourceConflict::SharedExclusive {
                        mut system_shared,
                        system_exclusive,
                        ..
                    }) => {
                        tmp_exclusive.push(system_exclusive);
                        tmp_shared.append(&mut system_shared);
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
            if !tmp_shared.is_empty() {
                for e in tmp_exclusive.iter() {
                    if tmp_shared.contains(e) {
                        return Err(ResourceConflict::SharedExclusive {
                            resource: 0, // dummy value, not used
                            system_shared: tmp_shared,
                            system_exclusive: *e,
                            layer: layer_index,
                        });
                    }
                }
                layer_systems.retain(|s| !tmp_shared.contains(s));
                layers.insert(layer_index, tmp_shared);
                shift_values(dependent_layers, layer_index + 1.., 1);
                for e in tmp_exclusive {
                    if dependent_layers[e.0].0 > layer_index + 1 {
                        dependent_layers[e.0].0 = layer_index + 1;
                    }
                }
            }
            layer_index += 1;
        }
        Ok(())
    }

    fn move_nonsent_to_end_of_layers(&self, layers: &mut [Vec<SystemId>]) {
        for layer in layers.iter_mut() {
            let mut e = layer.len();
            if e == 0 {
                continue;
            }
            e -= 1;
            let mut i = 0;
            while i < e {
                let system_id = layer[i];
                if matches!(self.systems[system_id.0], BoxedSystem::Unsend(_)) {
                    layer.swap(i, e);
                    e -= 1;
                } else {
                    i += 1;
                }
            }
        }
    }

    fn rebuild(&mut self) -> Result<(), ScheduleError> {
        // group systems based on their dependency graph
        let (mut layers, mut dependent_layer) = self
            .graph
            .work_graph()
            .systems_topological_order_with_dependent_layers()?;

        // split exclusive systems into dedicated layers
        self.split_exclusive(&mut layers, &mut dependent_layer);

        // resolve resource-conflicts by ordering EXCLUSIVE-access before SHARED access
        self.resolve_resource_conflicts(&mut layers, &mut dependent_layer)?;

        // move non-send systems to the end of the layers
        self.move_nonsent_to_end_of_layers(&mut layers);

        self.ordered_layers = layers;
        self.system_dependent_layers = dependent_layer;

        Ok(())
    }

    pub fn init(&mut self, resources: &mut Resources) -> bool {
        let r_atom = resources.atom();
        if self.atom != r_atom {
            if self.atom == Atom::ZERO {
                self.atom = resources.atom();
            } else {
                panic!("Schedule was already initialized with a different resource object");
            }
        }
        if self.version.check_and_reset(resources.version_mut()) {
            for (sys, access) in self.systems.iter_mut().zip(self.access.iter_mut()) {
                sys.init(resources);
                access.clear();
                sys.update_access(resources, access);
            }

            #[cfg(not(target_os = "unknown"))]
            if self.threadpool_id.is_none() {
                // create default-threatpool if not already created
                self.threadpool_id = Some(resources.init::<ThreadPool>());
            }

            if let Err(e) = self.rebuild() {
                let _ = self.dump_if_env();
                panic!(
                    "Failed to rebuild schedule: {e}\nuse PULZ_DUMP_SCHEDULE=[path] to dump the schedule to a file for debugging."
                );
            }
            true
        } else {
            false
        }
    }
}

impl Default for Schedule {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl SystemInit for Schedule {
    #[inline]
    fn init(&mut self, resources: &mut Resources) {
        self.init(resources);
    }
}

impl ExclusiveSystem for Schedule {
    #[inline]
    fn run_exclusive(&mut self, resources: &mut Resources) {
        self.run(resources);
    }
}

impl AsRef<Self> for Schedule {
    #[inline]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl AsMut<Self> for Schedule {
    #[inline]
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

pub struct ScheduleNodeBuilder<'l> {
    schedule: &'l mut Schedule,
    label: SystemSetId,
    node_id: NodeId,
}

impl ScheduleNodeBuilder<'_> {
    #[inline]
    pub fn parent(&mut self, label: impl SystemSet) -> &mut Self {
        let label_node = self
            .schedule
            ._get_or_set_label(label.as_label(), SystemId::UNDEFINED);
        self.schedule.graph.set_parent(label_node, self.node_id);
        self
    }

    #[inline]
    pub fn before(&mut self, label: impl SystemSet) -> &mut Self {
        let label_node = self
            .schedule
            ._get_or_set_label(label.as_label(), SystemId::UNDEFINED);
        self.schedule.graph.add_dependency(self.node_id, label_node);
        self
    }

    #[inline]
    pub fn after(&mut self, label: impl SystemSet) -> &mut Self {
        let label_node = self
            .schedule
            ._get_or_set_label(label.as_label(), SystemId::UNDEFINED);
        self.schedule.graph.add_dependency(label_node, self.node_id);
        self
    }

    #[inline]
    pub fn as_label(&self) -> SystemSetId {
        self.label
    }
}

fn shift_values(entries: &mut [Layer], in_range: impl RangeBounds<usize>, shift: isize) {
    if shift == 0 {
        return;
    }
    for entry in entries {
        if in_range.contains(&entry.0) {
            entry.0 = (entry.0 as isize + shift) as usize;
        }
    }
}
