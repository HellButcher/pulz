#![warn(
    //missing_docs,
    //rustdoc::missing_doc_code_examples,
    future_incompatible,
    rust_2018_idioms,
    unused,
    trivial_casts,
    trivial_numeric_casts,
    unused_lifetimes,
    unused_qualifications,
    //unused_crate_dependencies,
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
#![cfg_attr(all(doc, feature = "unstable"), feature(doc_cfg, rustdoc_internals))]
#![cfg_attr(all(doc, feature = "unstable"), allow(internal_features))]

pub use pulz_functional_utils_macros::generate_variadic_array;

#[cfg(feature = "tuple")]
macro_rules! maybe_tuple_doc {
    ($a:ident @ $item:item) => {
        #[cfg_attr(all(doc, feature = "unstable"), doc(fake_variadic))]
        $item
    };
    ($($rest_a:ident)* @ $item:item) => {
        #[doc(hidden)]
        $item
    };
}

#[cfg(feature = "tuple-convert")]
macro_rules! maybe_tuple_doc_alternative {
    ($a:ident @ $item:item) => {
        $item
    };
    ($($rest_a:ident)* @ $item:item) => {
        #[doc(hidden)]
        $item
    };
}

#[cfg(feature = "tuple")]
pub mod tuple;

#[cfg(feature = "func")]
pub mod func;
