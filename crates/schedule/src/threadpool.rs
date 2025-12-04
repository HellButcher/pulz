use std::{
    any::Any,
    cell::UnsafeCell,
    marker::PhantomData,
    panic,
    sync::{Arc, atomic},
    thread,
};

struct ScopeData {
    a_thread_panicked: atomic::AtomicBool,
    num_running_tasks: atomic::AtomicUsize,
    main_thread: thread::Thread,
}

struct Packet<'scope, T: Send> {
    scope_data: Arc<ScopeData>,
    result: UnsafeCell<Option<Result<T, Box<dyn Any + Send + 'static>>>>,
    _marker: PhantomData<Option<&'scope ScopeData>>,
}
pub struct Scope<'scope, 'env: 'scope> {
    data: Arc<ScopeData>,
    threadpool: &'scope ThreadPool,
    scope: PhantomData<&'scope mut &'scope ()>,
    env: PhantomData<&'env mut &'env ()>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ThreadPool(::threadpool::ThreadPool);

impl Default for ThreadPool {
    #[inline]
    fn default() -> Self {
        Self::from_env()
    }
}

impl ThreadPool {
    const DEFAULT_NAME: &'static str = module_path!();

    pub fn from_env() -> Self {
        let mut builder = ::threadpool::Builder::new().thread_name(Self::DEFAULT_NAME.to_string());
        if let Some(num_threads) = std::env::var("PULZ_NUM_THREADS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
        {
            builder = builder.num_threads(num_threads);
        }
        Self(builder.build())
    }

    #[inline]
    pub fn new(num_threads: usize) -> Self {
        Self::with_name(Self::DEFAULT_NAME.to_string(), num_threads)
    }

    #[inline]
    pub fn with_name(name: String, num_threads: usize) -> Self {
        Self(::threadpool::ThreadPool::with_name(name, num_threads))
    }

    /// Executes the function job on a thread in the pool.
    #[inline]
    pub fn execute<F>(&self, job: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.0.execute(job);
    }

    /// Block the current thread until all jobs in the pool have been executed.
    #[inline]
    pub fn join(&self) {
        self.0.join()
    }

    pub fn scope<'env, F, T>(&self, f: F) -> T
    where
        F: for<'scope> FnOnce(&'scope Scope<'scope, 'env>) -> T,
    {
        struct AbortOnPanic;
        impl Drop for AbortOnPanic {
            fn drop(&mut self) {
                if thread::panicking() {
                    std::process::abort();
                }
            }
        }

        let scope = Scope {
            data: Arc::new(ScopeData {
                num_running_tasks: atomic::AtomicUsize::new(0),
                main_thread: thread::current(),
                a_thread_panicked: atomic::AtomicBool::new(false),
            }),
            threadpool: self,
            env: PhantomData,
            scope: PhantomData,
        };

        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| f(&scope)));

        let guard = AbortOnPanic;

        // Wait until all the threads are finished.
        while scope.data.num_running_tasks.load(atomic::Ordering::Acquire) != 0 {
            thread::park();
        }

        std::mem::forget(guard);

        // Throw any panic from `f`, or the return value of `f` if no thread panicked.
        match result {
            Err(e) => panic::resume_unwind(e),
            Ok(_) if scope.data.a_thread_panicked.load(atomic::Ordering::Relaxed) => {
                panic!("a scoped task panicked")
            }
            Ok(result) => result,
        }
    }
}

impl From<::threadpool::ThreadPool> for ThreadPool {
    #[inline]
    fn from(pool: ::threadpool::ThreadPool) -> Self {
        Self(pool)
    }
}

impl<'scope, 'env> Scope<'scope, 'env> {
    pub fn execute<F, T>(&'scope self, f: F)
    where
        F: FnOnce() -> T + Send + 'scope,
        T: Send + 'scope,
    {
        let packet = Packet {
            scope_data: self.data.clone(),
            result: UnsafeCell::new(None),
            _marker: PhantomData,
        };
        let closure = move || {
            let mut packet = packet;
            let result = panic::catch_unwind(panic::AssertUnwindSafe(f));
            *packet.result.get_mut() = Some(result);
            // Here `task_packet` gets dropped, and if this is the last `Arc` for that packet that
            // will call `decrement_num_running_threads` and therefore signal that this thread is
            // done.
            drop(packet);
        };

        self.data.increment_num_running_tasks();

        let closure = Box::new(closure);
        // lifetime change to ensure that the closure is `'scope` and `'env` compatible
        let closure = unsafe {
            Box::from_raw(std::mem::transmute::<
                *mut (dyn FnOnce() + Send + '_),
                *mut (dyn FnOnce() + Send + 'static),
            >(Box::into_raw(closure)))
        };

        self.threadpool.execute(closure);
    }
}

impl ScopeData {
    #[inline]
    fn increment_num_running_tasks(&self) {
        if self
            .num_running_tasks
            .fetch_add(1, atomic::Ordering::Relaxed)
            > usize::MAX / 2
        {
            self.overflow();
        }
    }

    #[cold]
    fn overflow(&self) {
        self.decrement_num_running_tasks(false);
        panic!("too many running threads in thread scope");
    }

    fn decrement_num_running_tasks(&self, panic: bool) {
        if panic {
            self.a_thread_panicked
                .store(true, atomic::Ordering::Relaxed);
        }

        if self
            .num_running_tasks
            .fetch_sub(1, atomic::Ordering::Release)
            == 1
        {
            self.main_thread.unpark();
        }
    }
}

// Due to the usage of `UnsafeCell` we need to manually implement Sync.
// The type `T` should already always be Send (otherwise the thread could not
// have been created) and the Packet is Sync because all access to the
// `UnsafeCell` synchronized (by the `join()` boundary), and `ScopeData` is Sync.
unsafe impl<'scope, T: Send> Sync for Packet<'scope, T> {}

impl<'scope, T: Send> Drop for Packet<'scope, T> {
    fn drop(&mut self) {
        let unhandled_panic = match self.result.get_mut().take() {
            None => None,
            Some(Err(e)) => Some(e),
            Some(Ok(r)) => panic::catch_unwind(panic::AssertUnwindSafe(|| {
                drop(r);
            }))
            .err(),
        };

        self.scope_data
            .decrement_num_running_tasks(unhandled_panic.is_some());
        if let Some(e) = unhandled_panic {
            panic::resume_unwind(e);
        }
    }
}
