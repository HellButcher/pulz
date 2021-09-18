use std::{future::Future, pin::Pin, task::Poll};

use crate::{
    executor::{BoxFuture, Executor, JoinHandle},
    system::{IntoSystem, System, SystemDescriptor, SystemVariant},
    World,
};

#[derive(Default)]
pub struct Schedule<'l> {
    systems: Vec<SystemDescriptor<'l>>,
    // topoligical order of the systems, and the offset (index into `order`) where the system is required first
    order: Vec<(usize, usize)>,
    dirty: bool,
}

impl<'l> Schedule<'l> {
    pub const fn new() -> Self {
        Self {
            systems: Vec::new(),
            order: Vec::new(),
            dirty: true,
        }
    }

    pub fn with<Marker: 'l>(mut self, system: impl IntoSystem<'l, Marker>) -> Self {
        self.add_system(system);
        self
    }

    pub fn add_system<Marker: 'l>(&mut self, system: impl IntoSystem<'l, Marker>) -> &mut Self {
        self.add_system_inner(system.into_system());
        self
    }

    fn add_system_inner(&mut self, system: SystemDescriptor<'l>) {
        self.dirty = true;
        self.systems.push(system)
    }

    fn rebuild(&mut self) {
        // TODO: simple order
        self.order = (0..self.systems.len()).map(|i| (i, i + 1)).collect();
    }
}

struct ScheduleFuture<'a, 'l, E: Executor> {
    executor: &'a E,
    world: &'a mut World,
    schedule: &'a mut Schedule<'l>,
    tasks: Vec<Vec<E::JoinHandle>>,
    i: usize,
}

impl<'a, 'l, E: Executor> ScheduleFuture<'a, 'l, E> {
    fn poll_tasks(&mut self, cx: &mut std::task::Context<'_>) -> Poll<()> {
        if let Some(wait_for) = self.tasks.get_mut(self.i) {
            while let Some(item) = wait_for.last_mut() {
                let item = Pin::new(item);
                match item.poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(_) => {
                        wait_for.pop();
                    }
                }
            }
        }
        Poll::Ready(())
    }

    fn has_open_tasks(&self) -> bool {
        self.tasks.iter().any(|e| !e.is_empty())
    }
    fn abort_all(&mut self) {
        for wait_for in self.tasks.iter_mut() {
            while let Some(item) = wait_for.pop() {
                item.cancel_and_block();
            }
        }
    }
}

impl<'a, 'l, E: Executor> Future for ScheduleFuture<'a, 'l, E> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        // SAFETY: we dont move anything
        let this = unsafe { self.get_unchecked_mut() };
        while let Some(&(system_index, next_order_index)) = this.schedule.order.get(this.i) {
            match this.poll_tasks(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(()) => (),
            };
            let system = &mut this.schedule.systems[system_index];
            match system.system_variant {
                SystemVariant::Concurrent(ref mut system) => {
                    assert!(this.i < next_order_index && next_order_index < this.tasks.len());
                    let system: &mut dyn System = system.as_mut();
                    let world: &mut World = this.world;
                    let fut: BoxFuture<'_, ()> = Box::pin(async move { system.run(world) });
                    //SAFETY: we wait/block for fut. to be completed when dropped
                    let fut: BoxFuture<'static, ()> = unsafe { std::mem::transmute(fut) };
                    this.tasks[next_order_index].push(this.executor.spawn(fut));
                }
                SystemVariant::Exclusive(ref mut system) => {
                    system.run(this.world);
                }
            }
            this.i += 1;
        }
        this.poll_tasks(cx)
    }
}

impl<'a, 'l, E: Executor> Drop for ScheduleFuture<'a, 'l, E> {
    fn drop(&mut self) {
        if self.has_open_tasks() {
            self.abort_all()
        }
    }
}

impl<'l> Schedule<'l> {
    pub async fn run<E>(&mut self, executor: &E, world: &mut World)
    where
        E: Executor,
    {
        if self.dirty {
            self.rebuild();
            self.dirty = false;
        }

        let mut tasks = Vec::new();
        tasks.resize_with(self.order.len() + 1, Default::default);
        ScheduleFuture {
            executor,
            world,
            schedule: self,
            tasks,
            i: 0,
        }
        .await
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        executor::AsyncStd,
        system::{ExclusiveSystem, System},
    };

    use super::*;

    #[async_std::test]
    async fn test_schedule() {
        let executor = AsyncStd;
        struct A;
        struct Sys<'l>(&'l std::sync::atomic::AtomicUsize);
        let counter = std::sync::atomic::AtomicUsize::new(0);
        impl<'l> System for Sys<'l> {
            fn run(&mut self, _arg: &World) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            }
        }
        struct ExSys;
        impl ExclusiveSystem for ExSys {
            fn run(&mut self, arg: &mut World) {
                arg.spawn().insert(A);
            }
        }

        let mut world = World::new();
        let mut schedule = Schedule::new().with(Sys(&counter)).with(ExSys);

        assert_eq!(0, counter.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(0, world.entities().len());

        schedule.run(&executor, &mut world).await;

        assert_eq!(1, counter.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(1, world.entities().len());
    }
}
