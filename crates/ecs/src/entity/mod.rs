//! Entity identifier and management types.
//!
//! An [`Entity`] is a lightweight, versioned handle. [`Entities`] tracks
//! all live entities and their archetype locations. [`EntityRef`] and
//! [`EntityMut`] provide read and read-write access to a single entity's components.

mod entities;
mod entity_ref;
mod location;

pub use entities::{Entities, Entity};
pub use entity_ref::{EntityMut, EntityRef};
pub use location::EntityLocation;
