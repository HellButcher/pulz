use std::rc::Rc;

use pulz_executor::{Executor, ExecutorScope};

use crate::{
    system::{ExclusiveSystem, IntoSystem, System, SystemDescriptor, SystemVariant},
    world::World,
};

pub struct Schedule {
    systems: Vec<SystemDescriptor>,
    // topoligical order of the systems, and the offset (index into `order`) where the system is required first
    order: Vec<(usize, usize)>,
    dirty: bool,
    executor: Option<Rc<dyn ScheduleExecutor>>,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            order: Vec::new(),
            dirty: true,
            executor: None,
        }
    }

    #[inline]
    pub fn with<Marker>(mut self, system: impl IntoSystem<Marker>) -> Self {
        self.add_system(system);
        self
    }

    #[inline]
    pub fn add_system<Marker>(&mut self, system: impl IntoSystem<Marker>) -> &mut Self {
        self.add_system_inner(system.into_system());
        self
    }

    fn add_system_inner(&mut self, system: SystemDescriptor) {
        self.dirty = true;
        self.systems.push(system)
    }

    fn rebuild(&mut self) {
        // TODO: simple order
        self.order = (0..self.systems.len()).map(|i| (i, i + 1)).collect();
    }

    #[inline]
    pub fn with_executor<E: Executor>(mut self, executor: E) -> Self {
        self.set_executor(executor);
        self
    }

    #[inline]
    pub fn set_executor<E: Executor>(&mut self, executor: E) -> &mut Self {
        self.executor = Some(Rc::new(executor));
        self
    }

    #[inline]
    pub fn run(&mut self, world: &mut World) {
        let old_active_exec = world
            .resources_mut()
            .get_mut::<Rc<dyn ScheduleExecutor>>()
            .cloned();
        if let Some(exec) = self.executor.clone() {
            let active_exec_id = world.resources_mut().insert_unsend(exec.clone());

            exec.execute_schedule(world, self);

            if let Some(old) = old_active_exec {
                world.resources_mut().insert_unsend(old);
            } else {
                world.resources_mut().remove_id(active_exec_id);
            }
        } else if let Some(exec) = old_active_exec {
            exec.execute_schedule(world, self);
        } else {
            panic!("no executor active");
        }
    }
}

trait ScheduleExecutor: 'static {
    fn execute_schedule(&self, world: &mut World, schedule: &mut Schedule);
}

impl<E: Executor> ScheduleExecutor for E {
    fn execute_schedule(&self, world: &mut World, schedule: &mut Schedule) {
        if schedule.dirty {
            schedule.rebuild();
            schedule.dirty = false;
            for sys in &mut schedule.systems {
                sys.initialize(world)
            }
        }

        let mut tasks = ExecutorScope::with_capacity(self, schedule.order.len());

        let mut i = 0;
        while let Some(&(system_index, next_order_index)) = schedule.order.get(i) {
            // wait for dependencies
            tasks.wait_for(i);

            let system = &mut schedule.systems[system_index];
            match system.system_variant {
                SystemVariant::Concurrent(ref mut system) => {
                    assert!(i < next_order_index && next_order_index <= schedule.order.len());
                    let system: &mut dyn System = system.as_mut();
                    if system.is_send() {
                        let world = world.as_send(); // shared borrow
                        tasks.spawn(next_order_index, move || system.run_send(world));
                    } else {
                        let world = &*world;
                        tasks.spawn_local(next_order_index, move || system.run(world));
                    }
                }
                SystemVariant::Exclusive(ref mut system) => {
                    system.run(world);
                }
            }
            i += 1;
        }
    }
}

impl Default for Schedule {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl ExclusiveSystem for Schedule {
    #[inline]
    fn run(&mut self, world: &mut World) {
        self.run(world)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        executor::AsyncStdExecutor,
        system::{ExclusiveSystem, System},
    };

    use super::*;

    #[async_std::test]
    async fn test_schedule() {
        struct A;
        struct Sys(Arc<std::sync::atomic::AtomicUsize>);
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        unsafe impl System for Sys {
            fn run(&mut self, _arg: &World) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            }
            fn is_send(&self) -> bool {
                true
            }
        }
        struct ExSys;
        impl ExclusiveSystem for ExSys {
            fn run(&mut self, arg: &mut World) {
                arg.spawn().insert(A);
            }
        }

        let mut world = World::new();
        let mut schedule = Schedule::new().with(Sys(counter.clone())).with(ExSys);

        assert_eq!(0, counter.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(0, world.entities().len());

        AsyncStdExecutor.execute_schedule(&mut world, &mut schedule);

        assert_eq!(1, counter.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(1, world.entities().len());
    }
}
