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

use camera::{Camera, RenderTarget};
use pulz_assets::Assets;
use pulz_ecs::{define_label_enum, label::SystemPhase, prelude::*};

pub mod backend;
pub mod buffer;
pub mod camera;
pub mod mesh;
pub mod pipeline;
pub mod shader;
pub mod texture;
pub mod view;

pub use pulz_window as window;

pub mod color {
    pub use palette::{Hsla, LinSrgba, Srgba};
}

#[doc(hidden)]
pub use ::encase as __encase;
pub use pulz_transform::math;

define_label_enum! {
    pub enum RenderSystemPhase: SystemPhase {
        Sorting,
        UpdateGraph,
        Render,
    }
}

pub struct RenderModule;

impl Module for RenderModule {
    fn install_once(&self, res: &mut Resources) {
        Assets::<texture::Image>::install_into(res);

        let mut world = res.world_mut();
        world.init::<RenderTarget>();
        world.init::<Camera>();
    }

    fn install_systems(schedule: &mut Schedule) {
        schedule.add_phase_chain([
            RenderSystemPhase::Sorting,
            RenderSystemPhase::UpdateGraph,
            RenderSystemPhase::Render,
        ]);
    }
}
