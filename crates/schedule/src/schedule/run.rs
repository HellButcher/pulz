#[cfg(not(target_os = "unknown"))]
use std::ops::Range;

use crate::{
    prelude::{Resources, Schedule},
    schedule::SystemId,
    system::{BoxedSystem, ExclusiveSystem},
};
#[cfg(not(target_os = "unknown"))]
use crate::{schedule::Layer, threadpool::ThreadPool};

impl Schedule {
    /// The current target does not support spawning threads.
    /// Therefore this is an alias to `run_local`
    #[cfg(target_os = "unknown")]
    #[inline]
    pub fn run(&mut self, resources: &mut Resources) {
        self.run_local(resources);
    }

    /// Runs a single iteration of all active systems.
    ///
    /// Exclusive-Systems and Non-Send Systems are always run on the current thread.
    /// Send-Systems are send on a thread-pool.
    #[cfg(not(target_os = "unknown"))]
    pub fn run(&mut self, resources: &mut Resources) {
        self.init(resources);
        let mut from = 0;
        for layer_index in 0..self.ordered_layers.len() {
            let layer = &self.ordered_layers[layer_index];
            if self.is_exclusive_layer(layer) {
                if from < layer_index {
                    // run shared layers before exclusive layer
                    let threadpool = resources
                        .borrow_res_id(self.threadpool_id.unwrap())
                        .expect("ThreadPool resource not found");
                    Self::run_shared(
                        &threadpool,
                        resources,
                        &self.ordered_layers,
                        &self.system_dependent_layers,
                        from..layer_index,
                        &mut self.systems,
                    );
                }
                // run exclusive layer
                Self::run_layer_exclusive(resources, layer, &mut self.systems);
                from = layer_index + 1; // next layer is shared
            }
        }
        if from < self.ordered_layers.len() {
            // run remaining shared layers
            let threadpool = resources
                .borrow_res_id(self.threadpool_id.unwrap())
                .expect("ThreadPool resource not found");
            Self::run_shared(
                &threadpool,
                resources,
                &self.ordered_layers,
                &self.system_dependent_layers,
                from..self.ordered_layers.len(),
                &mut self.systems,
            );
        }
    }

    /// Runs a single iteration of all active systems.
    ///
    /// `run_local` runs all systems on the current thread.
    pub fn run_local(&mut self, resources: &mut Resources) {
        self.init(resources);
        for layer in &self.ordered_layers {
            Self::run_layer_exclusive(resources, layer, &mut self.systems);
        }
    }

    fn run_layer_exclusive(
        resources: &mut Resources,
        layer: &[SystemId],
        systems: &mut [BoxedSystem],
    ) {
        for &system_id in layer {
            let system = &mut systems[system_id.0];
            system.run_exclusive(resources);
        }
    }

    fn is_exclusive_layer(&self, layer: &[SystemId]) -> bool {
        let Some(&first) = layer.first() else {
            return false; // nothing to run
        };
        matches!(self.systems[first.0], BoxedSystem::Exclusive(_))
    }

    #[cfg(not(target_os = "unknown"))]
    fn run_shared(
        threadpool: &ThreadPool,
        resources: &Resources,
        layers: &[Vec<SystemId>],
        system_dependent_layers: &[Layer],
        range: Range<usize>,
        systems: &mut [BoxedSystem],
    ) {
        use crossbeam_utils::sync::WaitGroup;

        use crate::util::DisjointSliceHelper;

        let systems_disjoint_mut = DisjointSliceHelper::new(systems);
        threadpool.scope(|scope| {
            let mut wait_groups = Vec::new();
            wait_groups.resize_with(range.len(), || Some(WaitGroup::new()));

            for (current_layer_index, current_layer) in range.enumerate() {
                // wait for previous systems to finish
                wait_groups[current_layer_index].take().unwrap().wait();

                for &system_id in &layers[current_layer] {
                    let system = systems_disjoint_mut
                        .get_mut(system_id.0)
                        .expect("System already mutably borrowed");
                    match system {
                        BoxedSystem::Exclusive(_) => panic!("must be non-exclusive system"),
                        BoxedSystem::Unsend(system) => {
                            system.run(resources);
                        }
                        BoxedSystem::Send(system) => {
                            let next_layer = system_dependent_layers[system_id.0];
                            let resources = resources.as_send();
                            let wg = if let Some(wg) = wait_groups.get_mut(next_layer.0) {
                                wg.clone()
                            } else {
                                None
                            };
                            scope.execute(|| {
                                let wg = wg;
                                system.run_send(resources);
                                drop(wg);
                            });
                        }
                    }
                }
            }
        });
    }
}
