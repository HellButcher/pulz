use crossbeam_utils::sync::WaitGroup;

use crate::{
    resource::Resources,
    system::{ExclusiveSystem, IntoSystemDescriptor, SystemDescriptor, SystemVariant},
};

enum TaskGroup {
    // topoligical order of the systems, and the offset (index into `order`) where the system is
    // required first.
    // For example, the array `[(12,2), (13,2), (10,3)]` means, that the system at index `12` and
    // the system at index `13` are a dependency of the system at index `10`. So system `12` and `13`
    // need to be completed before system `10` can start.
    // The `2` refers to the third entry of this array `(10,3)`, so this means system `10`.
    // The `3` refers to the end of this array, so it is the last entry, and is not a dependency
    // of any entry in this group.
    Concurrent(Vec<(usize, usize)>),
    Exclusive(usize),
}

pub struct Schedule {
    systems: Vec<SystemDescriptor>,
    ordered_task_groups: Vec<TaskGroup>,
    dirty: bool,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            ordered_task_groups: Vec::new(),
            dirty: true,
        }
    }

    #[inline]
    pub fn with<Marker>(mut self, system: impl IntoSystemDescriptor<Marker>) -> Self {
        self.add_system(system);
        self
    }

    #[inline]
    pub fn add_system<Marker>(&mut self, system: impl IntoSystemDescriptor<Marker>) -> &mut Self {
        self.add_system_inner(system.into_system_descriptor());
        self
    }

    fn add_system_inner(&mut self, system: SystemDescriptor) {
        self.dirty = true;
        self.systems.push(system)
    }

    fn rebuild(&mut self) {
        // TODO: simple order
        self.ordered_task_groups = (0..self.systems.len()).map(TaskGroup::Exclusive).collect();
    }

    pub fn init(&mut self, resources: &mut Resources) {
        // TODO: track identity of resource-set
        if self.dirty {
            self.rebuild();
            self.dirty = false;
            for sys in &mut self.systems {
                sys.init(resources)
            }
        }
    }

    #[inline]
    pub fn run(&mut self, resources: &mut Resources) {
        self.executor(resources).run()
    }

    pub fn executor<'s>(&'s mut self, resources: &'s mut Resources) -> ScheduleExecution<'_> {
        self.init(resources);
        ScheduleExecution {
            systems: &mut self.systems,
            ordered_task_groups: &self.ordered_task_groups,
            resources,
            current_task_group: 0,
            current_sub_entry: 0,

            #[cfg(not(target_os = "unknown"))]
            tasks_rev: Vec::new(),
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
    fn init(&mut self, resources: &mut Resources) {
        self.init(resources)
    }
    #[inline]
    fn run(&mut self, resources: &mut Resources) {
        self.run(resources)
    }
}

impl Resources {
    #[inline]
    pub fn run<Marker>(&mut self, sys: impl IntoSystemDescriptor<Marker>) {
        sys.into_system_descriptor().run(self)
    }
}

#[must_use]
pub struct ScheduleExecution<'s> {
    systems: &'s mut [SystemDescriptor],
    ordered_task_groups: &'s [TaskGroup],
    resources: &'s mut Resources,
    current_task_group: usize,
    current_sub_entry: usize,
    #[cfg(not(target_os = "unknown"))]
    // Is one item longer than task_group.len().
    // The task `i` of a task_group will wait on WaitGroup [task_group.len() - current_sub_entry]!
    tasks_rev: Vec<WaitGroup>,
}

#[cfg(not(target_os = "unknown"))]
pub mod threadpool {
    use std::{cell::RefCell, ops::DerefMut, panic::AssertUnwindSafe, str::FromStr, sync::Mutex};

    pub use ::threadpool::{Builder, ThreadPool};
    use once_cell::sync::OnceCell;
    static GLOBAL: OnceCell<Mutex<ThreadPool>> = OnceCell::new();

    thread_local!(static CURRENT: RefCell<Option<ThreadPool>> = RefCell::new(None));

    pub fn replace_global_pool<F, R>(pool: ThreadPool) -> Option<ThreadPool> {
        let mut tmp = Some(pool);
        let mutex = GLOBAL.get_or_init(|| Mutex::new(tmp.take().unwrap()));
        tmp.map(|tmp| std::mem::replace(mutex.lock().unwrap().deref_mut(), tmp))
    }

    pub fn with_pool<F, R>(pool: ThreadPool, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        CURRENT.with(|current| {
            let old = current.replace(Some(pool));
            let result = std::panic::catch_unwind(AssertUnwindSafe(f));
            current.replace(old);
            match result {
                Err(panic) => std::panic::resume_unwind(panic),
                Ok(value) => value,
            }
        })
    }

    fn get_or_init_global() -> &'static Mutex<ThreadPool> {
        GLOBAL.get_or_init(|| {
            let pool = if let Some(num_threads) = std::env::var("PULZ_SCHEDULER_NUM_THREADS")
                .ok()
                .as_deref()
                .and_then(|s| usize::from_str(s).ok())
            {
                ThreadPool::new(num_threads)
            } else {
                ThreadPool::default()
            };
            Mutex::new(pool)
        })
    }

    pub(crate) fn spawn<F>(task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        CURRENT.with(|current| {
            if let Some(current) = current.borrow().as_ref() {
                current.execute(task);
            } else {
                let global = get_or_init_global().lock().unwrap();
                current.replace(Some(global.clone()));
                global.execute(task);
            }
        });
        todo!()
    }
}

impl<'s> ScheduleExecution<'s> {
    fn check_end(&mut self) -> bool {
        if self.current_task_group < self.ordered_task_groups.len() {
            true
        } else {
            // reset position, so a new iteration can begin
            self.current_task_group = 0;
            self.current_sub_entry = 0;
            false
        }
    }

    /// Runs a single iteration of all active systems on the *current thread*.
    pub fn run_local(&mut self) {
        while self.run_next_system_local() {}
    }

    /// Runs the next active system on the *current thread*.
    ///
    /// This method will return `true` as long there are more active systems to run in this iteration.
    /// When the iteration is completed, `false` is returned, and the execution is reset.
    /// When called after it had returned `false`, a new iteration will start and it will
    /// return `true` again, until this iteration is completed (and so on).
    pub fn run_next_system_local(&mut self) -> bool {
        match self.ordered_task_groups.get(self.current_task_group) {
            Some(&TaskGroup::Exclusive(system_index)) => {
                self.systems[system_index].run(self.resources);
                self.current_task_group += 1;
            }
            Some(TaskGroup::Concurrent(entries)) => {
                if let Some(&(system_index, _signal_task)) = entries.get(self.current_sub_entry) {
                    self.systems[system_index].run(self.resources);
                    self.current_sub_entry += 1;
                } else {
                    self.current_task_group += 1;
                    self.current_sub_entry = 0;
                }
            }
            None => (),
        }
        self.check_end()
    }

    /// The current target does not support spawning threads.
    /// Therefore this is an alias to `run_local`
    #[cfg(target_os = "unknown")]
    #[inline]
    pub fn run(&mut self) {
        self.run_local()
    }

    /// The current target does not support spawning threads.
    /// Therefore this is an alias to `run_next_system_local`
    ///
    /// SAFETY: because this is just an alias to `run_next_system_local`,
    /// this is actually safe!
    #[cfg(target_os = "unknown")]
    #[inline]
    pub unsafe fn run_next_system_unchecked(&mut self) -> bool {
        self.run_next_system_local()
    }

    /// Runs a single iteration of all active systems on the *current thread*.
    #[cfg(not(target_os = "unknown"))]
    #[inline]
    pub fn run(&mut self) {
        use std::panic::AssertUnwindSafe;

        let result = std::panic::catch_unwind(AssertUnwindSafe(|| unsafe {
            while self.run_next_system_unchecked() {}
        }));
        self.join();
        if let Err(err) = result {
            std::panic::resume_unwind(err);
        }
    }

    /// Runs the next active system. The system will be spawned onto a thread-pool,
    /// when supported by the system.
    ///
    /// This method will return `true` as long there are more active systems to run in this iteration.
    /// When the iteration is completed, `false` is returned, and the execution is reset.
    /// When called after it had returned `false`, a new iteration will start and it will
    /// return `true` again, until this iteration is completed (and so on).
    ///
    /// SAFETY: This function may spawn tasks in a thread-pool. These spawned tasks
    /// MUST NOT outlive lifetime `'s` on Self.
    /// The `Drop`-impl and 'join'-methis ensures, that all spawned tasks are completed before
    /// `self` is dropped by blocked-waiting on these tasks.
    /// But this can be bypassed with `std::mem::forget`. So you MUST NOT call
    /// `forget` or call `join` to enshure, all tasks are completed.
    #[cfg(not(target_os = "unknown"))]
    pub unsafe fn run_next_system_unchecked(&mut self) -> bool {
        match self.ordered_task_groups.get(self.current_task_group) {
            Some(&TaskGroup::Exclusive(system_index)) => {
                self.systems[system_index].run(self.resources);
                self.current_task_group += 1;
            }
            Some(TaskGroup::Concurrent(entries)) => {
                if let Some(&(system_index, _signal_task)) = entries.get(self.current_sub_entry) {
                    self.tasks_rev
                        .resize_with(entries.len() + 1 - self.current_sub_entry, Default::default);
                    let current_wait_group = self.tasks_rev.pop().unwrap();
                    let signal_wait_group =
                        self.tasks_rev[entries.len() - self.current_sub_entry].clone();

                    // UNSAFE: cast these lifetimes to a 'static scope for usage in
                    // spawned tasks. The requirement is, that these tasks do not
                    // outlive lifetime `'s` on `Self`. This is ensured by the `Drop`-impl,
                    // but this can be bypassed with `std::mem::forget`.
                    let system: *mut _ = &mut self.systems[system_index].system_variant;
                    let system = &mut *system;
                    let resources: *const _ = self.resources;
                    let resources = &*resources;

                    if let SystemVariant::Concurrent(system, _) = system {
                        if system.is_send() {
                            let resources = resources.as_send(); // shared borrow
                            self::threadpool::spawn(move || {
                                current_wait_group.wait();
                                system.run_send(resources);
                                drop(signal_wait_group);
                            });
                        } else {
                            // execute local
                            current_wait_group.wait();
                            system.run(resources);
                            drop(signal_wait_group);
                        }
                    } else {
                        panic!("expected a concurrent system!");
                    }
                } else {
                    self.current_task_group += 1;
                    self.current_sub_entry = 0;
                    self.join();
                }
            }
            None => (),
        }
        self.check_end()
    }

    #[cfg(not(target_os = "unknown"))]
    pub fn join(&mut self) {
        // wait for all outstanding tasks
        while let Some(wait_group) = self.tasks_rev.pop() {
            wait_group.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::system::{ExclusiveSystem, System};

    #[test]
    fn test_schedule() {
        struct A;
        struct Sys(Arc<std::sync::atomic::AtomicUsize>);
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        unsafe impl System for Sys {
            fn init(&mut self, _resources: &mut Resources) {}
            fn run(&mut self, _arg: &Resources) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            }
            fn is_send(&self) -> bool {
                true
            }
            fn update_access(
                &self,
                _resources: &Resources,
                _access: &mut crate::resource::ResourceAccess,
            ) {
            }
        }
        struct ExSys;
        impl ExclusiveSystem for ExSys {
            fn init(&mut self, _resources: &mut Resources) {}
            fn run(&mut self, arg: &mut Resources) {
                arg.insert(A);
            }
        }

        let mut resources = Resources::new();
        let mut schedule = Schedule::new().with(Sys(counter.clone())).with(ExSys);
        schedule.init(&mut resources);

        assert_eq!(0, counter.load(std::sync::atomic::Ordering::Acquire));
        assert!(resources.get_mut::<A>().is_none());

        schedule.run(&mut resources);

        assert_eq!(1, counter.load(std::sync::atomic::Ordering::Acquire));
        assert!(resources.get_mut::<A>().is_some());
    }
}
