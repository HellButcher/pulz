use pulz_executor::{Executor, ExecutorScope, SingleThreadedExecutor};

use crate::{
    system::{ExclusiveSystem, IntoSystem, System, SystemDescriptor, SystemVariant},
    world::World,
};

pub struct Schedule<E> {
    systems: Vec<SystemDescriptor>,
    // topoligical order of the systems, and the offset (index into `order`) where the system is required first
    order: Vec<(usize, usize)>,
    dirty: bool,
    executor: E,
}

impl Schedule<SingleThreadedExecutor> {
    #[inline]
    pub const fn new() -> Self {
        Self::with_executor(SingleThreadedExecutor)
    }
}

impl<E> Schedule<E> {
    pub const fn with_executor(executor: E) -> Self {
        Self {
            systems: Vec::new(),
            order: Vec::new(),
            dirty: true,
            executor,
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
}

impl<E: Executor> Schedule<E> {
    pub fn run(&mut self, world: &mut World) {
        if self.dirty {
            self.rebuild();
            self.dirty = false;
        }

        let mut tasks = ExecutorScope::with_capacity(&self.executor, self.order.len());

        let mut i = 0;
        while let Some(&(system_index, next_order_index)) = self.order.get(i) {
            // wait for dependencies
            tasks.wait_for(i);

            let system = &mut self.systems[system_index];
            match system.system_variant {
                SystemVariant::Concurrent(ref mut system) => {
                    assert!(i < next_order_index && next_order_index <= self.order.len());
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

impl<E: Default> Default for Schedule<E> {
    #[inline]
    fn default() -> Self {
        Self::with_executor(E::default())
    }
}

impl<E: Executor> ExclusiveSystem for Schedule<E> {
    #[inline]
    fn run(&mut self, arg: &mut World) {
        self.run(arg)
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
        let mut schedule = Schedule::with_executor(AsyncStdExecutor)
            .with(Sys(counter.clone()))
            .with(ExSys);

        assert_eq!(0, counter.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(0, world.entities().len());

        schedule.run(&mut world);

        assert_eq!(1, counter.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(1, world.entities().len());
    }
}
