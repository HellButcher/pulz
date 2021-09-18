#![warn(
    missing_docs,
    rustdoc::missing_doc_code_examples,
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
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::wildcard_imports
)]
#![doc(html_logo_url = "https://raw.githubusercontent.com/HellButcher/pulz/master/docs/logo.png")]
#![doc(html_no_source)]
#![doc = include_str!("../README.md")]

use std::{future::Future, pin::Pin};

/// An owned dynamically typed [`Future`]
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// An owned dynamically typed [`Future`] without `Send` requirement
pub type LocalBoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// A handle that awaits the result of a spawned task.
///
/// Dropping a [`JoinHandle`] will detach the task, meaning that there is no longer
/// a handle to the task and no way to `join` on it.
pub trait JoinHandle: Future + Unpin {
    /// Cancels the tasks and blocks until it is cancelled.
    fn cancel_and_block(self);
}

/// An Abstraction over common functionalities of async runtimes.
pub trait Executor {
    /// The return-type used by `spawn(..)`
    type JoinHandle: JoinHandle;

    /// Spawns an async task.
    fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static) -> Self::JoinHandle;

    /// Spawns a task and blocks the current thread on its result.
    fn block_on(&self, fut: impl Future<Output = ()>);
}

#[cfg(feature = "tokio")]
mod tokio {
    use std::future::Future;

    use tokio::task::JoinError;

    use super::{Executor, JoinHandle};

    impl JoinHandle for tokio::task::JoinHandle<()> {
        #[inline]
        fn cancel_and_block(self) {
            Self::abort(self)
        }
    }
    impl Executor for tokio::runtime::Handle {
        type JoinHandle = tokio::task::JoinHandle<()>;
        #[inline]
        fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static) -> Self::JoinHandle {
            Self::spawn(self, fut)
        }
        #[inline]
        fn block_on(&self, fut: impl Future<Output = ()>) {
            Self::block_on(self, fut)
        }
    }
    impl Executor for tokio::runtime::Runtime {
        type JoinHandle = tokio::task::JoinHandle<()>;
        #[inline]
        fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static) -> Self::JoinHandle {
            Self::spawn(self, fut)
        }
        #[inline]
        fn block_on(&self, fut: impl Future<Output = ()>) {
            Self::block_on(self, fut)
        }
    }
}

#[cfg(feature = "async-std")]
mod async_std {
    use super::{Executor, JoinHandle};
    use std::future::Future;

    /// An `Executor` implementation for `async-std`
    pub struct AsyncStd;

    impl JoinHandle for async_std::task::JoinHandle<()> {
        #[inline]
        fn cancel_and_block(self) {
            async_std::task::block_on(Self::cancel(self));
        }
    }
    impl Executor for AsyncStd {
        type JoinHandle = async_std::task::JoinHandle<()>;
        #[inline]
        fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static) -> Self::JoinHandle {
            async_std::task::spawn(fut)
        }
        #[inline]
        fn block_on(&self, fut: impl Future<Output = ()>) {
            async_std::task::block_on(fut)
        }
    }
}

#[cfg(feature = "async-std")]
pub use self::async_std::AsyncStd;
