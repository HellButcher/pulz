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

use std::{future::Future, pin::Pin};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type LocalBoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

pub trait JoinHandle: Future + Unpin {
    fn cancel_and_block(self);
}

pub trait Executor {
    type JoinHandle: JoinHandle;
    fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static) -> Self::JoinHandle;
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
