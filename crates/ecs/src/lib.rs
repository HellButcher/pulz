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

use component::ComponentSet;
pub use pulz_schedule::*;

#[doc(hidden)]
pub enum Void {}

pub mod archetype;
pub mod component;
pub mod query;

pub mod entity;
mod entity_ref;
pub mod removed;
pub mod storage;
pub mod world;

pub use component::Component;
pub use entity::{Entity, EntityMut, EntityRef};
use pulz_schedule::schedule::Schedule;
pub use world::WorldExt;

use crate::storage::AnyStorage;

pub mod prelude {
    pub use pulz_schedule::prelude::*;

    pub use crate::{
        component::Component,
        entity::{Entity, EntityMut, EntityRef},
        query::Query,
        world::{World, WorldExt},
    };
}

struct WorldInner {
    entities: entity::Entities,
    components: component::Components,
    archetypes: archetype::Archetypes,

    tmp_removed: ComponentSet,
    tmp_inserted: ComponentSet,
    // tracks removed components
    //removed: component::ComponentMap<Vec<Entity>>,
}

impl Default for WorldInner {
    fn default() -> Self {
        Self {
            entities: entity::Entities::new(),
            components: component::Components::new(),
            archetypes: archetype::Archetypes::new(),

            tmp_removed: ComponentSet::new(),
            tmp_inserted: ComponentSet::new(),
            //removed: component::ComponentMap::new(),
        }
    }
}

fn insert_sorted<T: Ord>(vec: &mut Vec<T>, value: T) {
    if let Err(pos) = vec.binary_search(&value) {
        vec.insert(pos, value);
    }
}

fn get_or_init_component<'a, T>(
    res: &'a mut resource::Resources,
    comps: &'a mut component::Components,
) -> (resource::ResourceId<T::Storage>, component::ComponentId<T>)
where
    T: Component,
{
    use storage::Storage;
    if let Some(component_id) = comps.id::<T>() {
        let component = comps.get(component_id).unwrap();
        (component.storage_id.typed(), component_id)
    } else {
        let storage_id = res.init::<T::Storage>();
        res.init_meta_id::<dyn AnyStorage, _>(storage_id);
        let component_id = comps.insert(storage_id, T::Storage::SPARSE).unwrap();
        {
            let schedule = res.get_mut::<Schedule>().unwrap();
            <T::Storage as Storage>::install_systems(schedule);
        }

        (storage_id, component_id)
    }
}
