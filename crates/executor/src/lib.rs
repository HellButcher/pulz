#![warn(
    // missing_docs,
    // rustdoc::missing_doc_code_examples,
    future_incompatible,
    rust_2018_idioms,
    unused,
    trivial_casts,
    trivial_numeric_casts,
    unused_lifetimes,
    unused_qualifications,
    unused_crate_dependencies,
    clippy::cargo,
    clippy::multiple_crate_versions,
    clippy::empty_line_after_outer_attr,
    clippy::fallible_impl_from,
    clippy::redundant_pub_crate,
    clippy::use_self,
    clippy::suspicious_operation_groupings,
    clippy::useless_let_if_seq,
    // clippy::missing_errors_doc,
    // clippy::missing_panics_doc,
    clippy::wildcard_imports
)]
#![doc(html_logo_url = "https://raw.githubusercontent.com/HellButcher/pulz/master/docs/logo.png")]
#![doc(html_no_source)]
#![doc = include_str!("../README.md")]

/// A handle that awaits the result of a spawned task.
///
/// Dropping a [`JoinHandle`] will detach the task, meaning that there is no longer
/// a handle to the task and no way to `join` on it.
pub trait JoinHandle {
    /// Cancels the tasks and blocks until it is cancelled.
    fn cancel_and_block(self);
}

/// An Abstraction over common functionalities of async runtimes.
pub trait Executor: 'static {
    /// The return-type used by `spawn(..)`
    type JoinHandle: JoinHandle;

    /// Spawns an async task.
    fn spawn(&self, task: impl FnOnce() + Send + 'static) -> Self::JoinHandle;

    /// Waits for the `JoinHandle` that was issued by `spawn`
    fn wait_for(&self, handles: impl Iterator<Item = Self::JoinHandle>);
}

pub struct ExecutorScope<'a, E: Executor> {
    executor: &'a E,
    tasks: Vec<Vec<E::JoinHandle>>,
    local_tasks: Vec<Box<dyn FnOnce()>>,
}

impl<'a, E: Executor> ExecutorScope<'a, E> {
    pub fn with_capacity(executor: &'a E, num_tasks: usize) -> Self {
        let mut tasks = Vec::new();
        tasks.resize_with(num_tasks + 1, Default::default);
        Self {
            executor,
            tasks,
            local_tasks: Vec::new(),
        }
    }

    #[inline]
    pub fn new(executor: &'a E) -> Self {
        Self::with_capacity(executor, 1)
    }

    pub fn spawn<'b>(&'b mut self, i: usize, task: impl FnOnce() + Send + 'b) {
        let task: Box<dyn FnOnce() + Send + 'b> = Box::new(task);
        //SAFETY: we wait/block for task. to be completed when dropped
        let task: Box<dyn FnOnce() + Send + 'static> = unsafe { std::mem::transmute(task) };
        if self.tasks.len() <= i {
            self.tasks.resize_with(i + 1, Default::default);
        }
        self.tasks[i].push(self.executor.spawn(task));
    }

    pub fn spawn_local<'b>(&'b mut self, _i: usize, task: impl FnOnce() + 'b) {
        let task: Box<dyn FnOnce() + 'b> = Box::new(task);
        //SAFETY: we wait/block for task. to be completed when dropped
        let task: Box<dyn FnOnce() + 'static> = unsafe { std::mem::transmute(task) };
        self.local_tasks.push(task);
    }

    pub fn wait_for(&mut self, i: usize) {
        // first execute all local tasks
        for task in self.local_tasks.drain(..) {
            task()
        }

        // then wait for async tasks
        if let Some(handles) = self.tasks.get_mut(i) {
            if !handles.is_empty() {
                self.executor.wait_for(handles.drain(..));
            }
        }
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

impl<'a, E: Executor> Drop for ExecutorScope<'a, E> {
    fn drop(&mut self) {
        if self.has_open_tasks() {
            self.abort_all();
        }
    }
}

mod single_threaded {

    /// pseudo Join-Handle for ImmediateExecutor. Does nothing because the
    /// task will already be completed, when `spawn` returns.
    pub struct JoinHandle;

    /// simple executor that executes tasks immediately within the same thread as the caller.
    #[derive(Copy, Clone, Debug)]
    pub struct ImmediateExecutor;

    impl super::JoinHandle for JoinHandle {
        #[inline]
        fn cancel_and_block(self) {}
    }

    impl super::Executor for ImmediateExecutor {
        type JoinHandle = JoinHandle;
        #[inline]
        fn spawn(&self, task: impl FnOnce() + Send + 'static) -> Self::JoinHandle {
            task();
            JoinHandle
        }
        fn wait_for(&self, _handles: impl Iterator<Item = Self::JoinHandle>) {}
    }
}

pub use self::single_threaded::ImmediateExecutor;

#[cfg(feature = "tokio")]
mod tokio {

    use super::{Executor, JoinHandle};

    impl JoinHandle for tokio::task::JoinHandle<()> {
        #[inline]
        fn cancel_and_block(self) {
            Self::abort(&self)
        }
    }
    impl Executor for tokio::runtime::Handle {
        type JoinHandle = tokio::task::JoinHandle<()>;
        #[inline]
        fn spawn(&self, task: impl FnOnce() + Send + 'static) -> Self::JoinHandle {
            Self::spawn(self, async move {
                task();
            })
        }
        #[inline]
        fn wait_for(&self, handles: impl Iterator<Item = Self::JoinHandle>) {
            Self::block_on(self, async move {
                for handle in handles {
                    handle.await;
                }
            })
        }
    }
    impl Executor for tokio::runtime::Runtime {
        type JoinHandle = tokio::task::JoinHandle<()>;
        #[inline]
        fn spawn(&self, task: impl FnOnce() + Send + 'static) -> Self::JoinHandle {
            Self::spawn(self, async move {
                task();
            })
        }
        #[inline]
        fn wait_for(&self, handles: impl Iterator<Item = Self::JoinHandle>) {
            Self::block_on(self, async move {
                for handle in handles {
                    handle.await;
                }
            })
        }
    }
}

#[cfg(feature = "async-std")]
mod async_std {
    use super::{Executor, JoinHandle};

    /// An `Executor` implementation for `async-std`
    pub struct AsyncStdExecutor;

    impl JoinHandle for async_std::task::JoinHandle<()> {
        #[inline]
        fn cancel_and_block(self) {
            async_std::task::block_on(self.cancel());
        }
    }
    impl Executor for AsyncStdExecutor {
        type JoinHandle = async_std::task::JoinHandle<()>;

        #[cfg(target_os = "unknown")]
        #[inline]
        fn spawn(&self, task: impl FnOnce() + Send + 'static) -> Self::JoinHandle {
            async_std::task::spawn_local(async move { task() })
        }

        #[cfg(not(target_os = "unknown"))]
        #[inline]
        fn spawn(&self, task: impl FnOnce() + Send + 'static) -> Self::JoinHandle {
            async_std::task::spawn(async move { task() })
        }

        #[inline]
        fn wait_for(&self, handles: impl Iterator<Item = Self::JoinHandle>) {
            async_std::task::block_on(async move {
                for handle in handles {
                    handle.await;
                }
            })
        }
    }
}

#[cfg(feature = "async-std")]
pub use self::async_std::AsyncStdExecutor;
