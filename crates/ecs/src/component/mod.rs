//! Component trait and related types for the ECS.
//!
//! A [`Component`] is a plain data type that can be attached to entities.
//! Components are stored in typed [`Storage`] resources and looked up by [`ComponentId`].

use pulz_schedule::resource::{Res, ResMut};

mod components;
mod id;
/// A sorted-array map keyed by component id.
pub mod map;
/// A compact bitset of component ids describing archetype membership.
pub mod set;

pub use components::{ComponentData, Components};
pub use id::ComponentId;

use crate::storage::Storage;

/// Shared borrow of a component value, equivalent to [`Res`].
pub type Ref<'w, T> = Res<'w, T>;
/// Exclusive borrow of a component value, equivalent to [`ResMut`].
pub type RefMut<'w, T> = ResMut<'w, T>;

pub use pulz_ecs_macros::Component;

/// Marker trait for types that can be attached to entities as components.
///
/// Implement this trait (or derive it with `#[derive(Component)]`) to make a type usable
/// as a component. The associated `Storage` type controls how instances are stored.
pub trait Component: Send + Sync + 'static {
    /// The storage backend used to hold values of this component type.
    type Storage: Storage<Component = Self>;
}
