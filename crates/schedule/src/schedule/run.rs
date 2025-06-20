#[cfg(not(target_os = "unknown"))]
use std::ops::Range;

use crate::{
    prelude::{Resources, Schedule},
    resource::ResourceAccess,
    schedule::{RunSharedSheduleSystem, SharedSchedule, SystemId},
    system::{BoxedSystem, ExclusiveSystem, System, SystemInit},
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
                    Self::run_layers_shared(
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
            Self::run_layers_shared(
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
    fn run_layers_shared(
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

impl SharedSchedule {
    fn run_shared(&mut self, resources: &Resources) {
        for layer in &self.0.ordered_layers {
            if self.is_exclusive_layer(layer) {
                panic!("SharedSchedule cannot run exclusive layers");
            }
        }
        #[cfg(not(target_os = "unknown"))]
        {
            let threadpool = resources
                .borrow_res_id(self.threadpool_id.unwrap())
                .expect("ThreadPool resource not found");
            Schedule::run_layers_shared(
                &threadpool,
                resources,
                &self.0.ordered_layers,
                &self.0.system_dependent_layers,
                0..self.0.ordered_layers.len(),
                &mut self.0.systems,
            );
        }
        #[cfg(target_os = "unknown")]
        {
            for layer in &self.0.ordered_layers {
                for &system_id in layer {
                    let system = &mut self.0.systems[system_id.0];
                    system.run(resources);
                }
            }
        }
    }
}

impl SystemInit for SharedSchedule {
    #[inline]
    fn init(&mut self, resources: &mut Resources) {
        self.init(resources);
    }
}

impl System for SharedSchedule {
    #[inline]
    fn run(&mut self, resources: &Resources) {
        self.run_shared(resources);
    }

    fn update_access(&self, res: &Resources, access: &mut ResourceAccess) {
        let mut access_result = ResourceAccess::new();
        let mut sub_access = ResourceAccess::new();
        for layer in &self.0.ordered_layers {
            sub_access.clear();
            for &system_id in layer {
                let system = &self.0.systems[system_id.0];
                system.update_access(res, &mut sub_access);
            }
            access_result.union_with(&sub_access);
        }
        access.union_with_checked(&sub_access);
    }
}

impl<S> SystemInit for RunSharedSheduleSystem<S>
where
    S: AsMut<SharedSchedule>,
{
    #[inline]
    fn init(&mut self, res: &mut Resources) {
        let schedule_id = *self.0.get_or_insert_with(|| res.expect_id());
        res.take_id_and(schedule_id, |schedule, res| {
            schedule.as_mut().init(res);
        });
    }
}

impl<S> System for RunSharedSheduleSystem<S>
where
    S: AsMut<SharedSchedule>,
{
    fn run(&mut self, res: &Resources) {
        let schedule_id = self.0.expect("not initialized");
        res.borrow_res_mut_id(schedule_id)
            .unwrap()
            .as_mut()
            .run_shared(res);
    }

    fn update_access(&self, res: &Resources, access: &mut ResourceAccess) {
        let schedule_id = self.0.expect("not initialized");
        access.add_shared_checked(schedule_id);
        res.borrow_res_mut_id(schedule_id)
            .unwrap()
            .as_mut()
            .update_access(res, access);
    }
}
