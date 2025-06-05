use crate::schedule::{ScheduleExecution, SharedScheduleExecution, TaskGroup};

#[cfg(not(target_os = "unknown"))]
pub mod threadpool {
    use std::{
        cell::RefCell,
        ops::DerefMut,
        panic::AssertUnwindSafe,
        str::FromStr,
        sync::{Mutex, OnceLock},
    };

    pub use ::threadpool::ThreadPool;
    static GLOBAL: OnceLock<Mutex<ThreadPool>> = OnceLock::new();

    thread_local!(static CURRENT: RefCell<Option<ThreadPool>> = const { RefCell::new(None) });

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
            {
                if let Some(current) = current.borrow().as_ref() {
                    current.execute(task);
                    return;
                }
            }
            let global = { get_or_init_global().lock().unwrap().clone() };
            current.replace(Some(global.clone()));
            global.execute(task);
        });
    }
}

impl ScheduleExecution<'_> {
    /// Runs a single iteration of all active systems on the *current thread*.
    pub fn run_local(&mut self) {
        for group in self.ordered_task_groups {
            match group {
                TaskGroup::Exclusive(system_index) => {
                    self.systems[*system_index].run_exclusive(self.resources);
                }
                TaskGroup::Concurrent(entries) => {
                    for &(system_index, _signal_task) in entries {
                        self.systems[system_index].run_shared(self.resources);
                    }
                }
            }
        }
    }

    /// The current target does not support spawning threads.
    /// Therefore this is an alias to `run_local`
    #[cfg(target_os = "unknown")]
    #[inline]
    pub fn run(&mut self) {
        self.run_local()
    }

    /// Runs a single iteration of all active systems.
    ///
    /// Exclusive-Systems and Non-Send Systems are always run on the current thread.
    /// Send-Systems are send on a thread-pool.
    #[cfg(not(target_os = "unknown"))]
    #[inline]
    pub fn run(&mut self) {
        for group in self.ordered_task_groups {
            match group {
                TaskGroup::Exclusive(system_index) => {
                    self.systems[*system_index].run_exclusive(self.resources);
                }
                TaskGroup::Concurrent(entries) => {
                    use crate::schedule::SharedScheduleExecution;

                    let mut shared = SharedScheduleExecution {
                        systems: self.systems,
                        concurrent_tasks: entries,
                        resources: self.resources,
                        tasks_rev: std::mem::take(&mut self.tasks_rev),
                    };
                    shared.run();
                    std::mem::swap(&mut self.tasks_rev, &mut shared.tasks_rev);
                }
            }
        }
    }
}

impl SharedScheduleExecution<'_> {
    /// Runs a single iteration of all active systems on the *current thread*.
    pub fn run_local(&mut self) {
        for &(system_index, _signal_task) in self.concurrent_tasks {
            self.systems[system_index].run_shared(self.resources);
        }
    }

    /// The current target does not support spawning threads.
    /// Therefore this is an alias to `run_local`
    #[cfg(target_os = "unknown")]
    #[inline]
    pub fn run(&mut self) {
        self.run_local()
    }

    /// Runs a single iteration of all active systems.
    ///
    /// Exclusive-Systems and Non-Send Systems are always run on the current thread.
    /// Send-Systems are send on a thread-pool.
    #[cfg(not(target_os = "unknown"))]
    #[inline]
    pub fn run(&mut self) {
        self.tasks_rev
            .resize_with(self.concurrent_tasks.len() + 1, Default::default);
        for &(system_index, signal_task) in self.concurrent_tasks {
            use crate::system::SystemVariant;

            let current_wait_group = self.tasks_rev.pop().unwrap();
            let signal_wait_group_index = if signal_task == !0 {
                0
            } else {
                self.concurrent_tasks.len() - signal_task
            };
            let signal_wait_group = self.tasks_rev[signal_wait_group_index].clone();

            let SystemVariant::Concurrent(system, _) =
                &mut self.systems[system_index].system_variant
            else {
                unreachable!("expected a concurrent system!");
            };

            // UNSAFE: cast these lifetimes to a 'static scope for usage in
            // spawned tasks. The requirement is, that these tasks do not
            // outlive lifetime `'s` on `Self`.
            // This is ensured by the Wait-Group and the Drop-impl (in case a panic happens)
            //
            // This also has multiple references into self.systems, but the one entry is
            // accessed by at most one loop-iteration / spawned-thread
            let (resources, system) = unsafe {
                let resources: *const _ = self.resources;
                let system: *mut _ = system;
                (&*resources, &mut *system)
            };

            if system.is_send() {
                let resources = resources.as_send(); // shared borrow
                threadpool::spawn(move || {
                    current_wait_group.wait();
                    system.run_send(resources, ());
                    drop(signal_wait_group);
                });
            } else {
                // execute local
                current_wait_group.wait();
                system.run(self.resources, ());
                drop(signal_wait_group);
            }
        }
        self.join();
    }

    #[cfg(not(target_os = "unknown"))]
    fn join(&mut self) {
        // wait for all outstanding tasks
        while let Some(wait_group) = self.tasks_rev.pop() {
            wait_group.wait();
        }
    }
}

#[cfg(not(target_os = "unknown"))]
impl Drop for SharedScheduleExecution<'_> {
    fn drop(&mut self) {
        // usually only relevant on panic
        self.join();
    }
}
