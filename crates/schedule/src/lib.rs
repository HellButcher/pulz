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

macro_rules! peel {
    ($macro:tt [$($args:tt)*] $name:ident.$index:tt, ) => ($macro! { $($args)* });
    ($macro:tt [$($args:tt)*] $name:ident.$index:tt, $($other:tt)+) => (peel!{ $macro [$($args)* $name.$index, ] $($other)+ } );
}

#[doc(hidden)]
pub enum Void {}

pub mod event;
pub mod label;
pub mod module;
pub mod resource;
pub mod schedule;
pub mod system;

pub mod prelude {
    pub use crate::{
        module::Module,
        resource::{FromResources, Res, ResMut, ResourceId, Resources},
        schedule::Schedule,
        system::{IntoExclusiveSystem, IntoSystem},
    };
}
