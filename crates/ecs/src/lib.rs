//! ECS (Entity Component System) implementation built on top of [`pulz_schedule`].
//!
//! Provides entities, components stored in archetype-based column storage,
//! and queries for efficiently iterating matching entities.
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

// Allows macro-generated code to resolve to this crate when used internally (tests, doctests).

pub mod archetype;
pub mod component;
pub mod entity;
pub mod query;
pub mod storage;
mod world;

#[doc(hidden)]
use pulz_schedule::Void;

pub use crate::world::{World, WorldMut};

#[derive(Default)]
struct WorldInner {
    entities: entity::Entities,
    components: component::Components,
    archetypes: archetype::Archetypes,
}
struct WorldMutInnerTemp {
    tmp_removed: component::set::ComponentSet,
    tmp_inserted: component::set::ComponentSet,
    // tracks removed components
    //removed: component::ComponentMap<Vec<Entity>>,
}

/// Extension trait that adds [`World`] and [`WorldMut`] accessors to [`pulz_schedule::resource::Resources`].
pub trait ResourcesExt {
    /// Returns a shared view of the ECS world stored in these resources.
    fn world(&self) -> World<'_>;
    /// Returns an exclusive view of the ECS world stored in these resources.
    fn world_mut(&mut self) -> WorldMut<'_>;
}

/// Convenience re-exports for common ECS types.
pub mod prelude {
    pub use pulz_schedule::prelude::*;

    pub use crate::{
        ResourcesExt, World, WorldMut,
        component::Component,
        entity::{Entity, EntityMut, EntityRef},
        query::{Query, QueryData, QueryFilter, With, Without},
    };
}
