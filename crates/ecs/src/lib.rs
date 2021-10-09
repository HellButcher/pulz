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
    unused_crate_dependencies,
    clippy::cargo,
    clippy::multiple_crate_versions,
    clippy::empty_line_after_outer_attr,
    clippy::fallible_impl_from,
    clippy::redundant_pub_crate,
    clippy::use_self,
    clippy::suspicious_operation_groupings,
    clippy::useless_let_if_seq,
    //clippy::missing_errors_doc,
    //clippy::missing_panics_doc,
    clippy::wildcard_imports
)]
#![doc(html_logo_url = "https://raw.githubusercontent.com/HellButcher/pulz/master/docs/logo.png")]
#![doc(html_no_source)]
#![doc = include_str!("../README.md")]

macro_rules! peel {
    ($macro:tt [$($args:tt)*] ) => ($macro! { $($args)* });
    ($macro:tt [$($args:tt)*] $name:ident.$index:tt, ) => ($macro! { $($args)* });
    ($macro:tt [$($args:tt)*] $name:ident.$index:tt, $($other:tt)+) => (peel!{ $macro [$($args)* $name.$index, ] $($other)+ } );
}

pub use pulz_schedule::*;

#[doc(hidden)]
pub enum Void {}

mod archetype;
pub mod component;
pub mod query;

mod entity;
mod entity_ref;
mod storage;
pub mod world;

pub use entity::Entity;
pub use entity_ref::{EntityMut, EntityRef};
pub use world::WorldExt;

struct WorldInner {
    entities: entity::Entities,
    components: component::Components,
    archetypes: archetype::Archetypes,

    // tracks removed components
    removed: component::ComponentMap<Vec<Entity>>,
}

impl Default for WorldInner {
    fn default() -> Self {
        Self {
            entities: entity::Entities::new(),
            components: component::Components::new(),
            archetypes: archetype::Archetypes::new(),
            removed: component::ComponentMap::new(),
        }
    }
}

fn get_or_init_component<'a, T>(
    res: &'a mut resource::Resources,
    comps: &'a mut component::Components,
    mut sparse: bool,
) -> (
    resource::ResourceId<storage::Storage<T>>,
    component::ComponentId<T>,
)
where
    T: Send + Sync + 'static,
{
    if let Some(component_id) = comps.get_id::<T>() {
        let component = comps.get(component_id).unwrap();
        (component.storage_id.typed(), component_id)
    } else if let Some(storage_id) = res.get_id::<storage::Storage<T>>() {
        sparse = matches!(
            res.get_mut_id(storage_id),
            Some(storage::Storage::Sparse(_))
        );
        let component_id = comps.insert(storage_id, sparse).unwrap();
        (storage_id, component_id)
    } else {
        let storage = if sparse {
            storage::Storage::new_sparse()
        } else {
            storage::Storage::new_dense()
        };
        let storage_id = res.insert::<storage::Storage<T>>(storage);
        let component_id = comps.insert(storage_id, sparse).unwrap();
        (storage_id, component_id)
    }
}
