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

use camera::{Camera, Projection, RenderTarget};
use graph::{RenderGraph, RenderGraphBuilder};
use pulz_assets::Assets;
use pulz_ecs::{
    define_label_enum,
    label::{CoreSystemPhase, SystemPhase},
    prelude::*,
};

pub mod backend;
pub mod buffer;
pub mod camera;
pub mod draw;
pub mod graph;
pub mod mesh;
pub mod pipeline;
pub mod shader;
pub mod surface;
pub mod texture;
pub mod utils;

pub use pulz_window as window;

pub mod color {
    pub use palette::{Hsla, LinSrgba, Srgba};
}

#[doc(hidden)]
pub use ::encase as __encase;
pub use pulz_transform::math;
use surface::WindowSurfaces;

define_label_enum! {
    pub enum RenderSystemPhase: SystemPhase {
        UpdateCamera,
        Sorting,
        BuildGraph,
        UpdateGraph,
        Render,
    }
}

pub struct RenderModule;

impl Module for RenderModule {
    fn install_once(&self, res: &mut Resources) {
        Assets::<texture::Image>::install_into(res);

        res.init::<RenderGraphBuilder>();
        res.init::<RenderGraph>();
        res.init::<WindowSurfaces>();
        // TODO:
        //res.init::<TextureCache>();
        //render_graph::graph::RenderGraph::install_into(res, schedule);

        let mut world = res.world_mut();
        world.init::<RenderTarget>();
        world.init::<Camera>();
        world.init::<Projection>();
    }

    fn install_systems(schedule: &mut Schedule) {
        schedule.add_phase_chain([
            RenderSystemPhase::UpdateCamera,
            RenderSystemPhase::Sorting,
            RenderSystemPhase::BuildGraph,
            RenderSystemPhase::UpdateGraph,
            RenderSystemPhase::Render,
        ]);
        // update projection and camera
        schedule
            .add_system(camera::update_projections_from_render_targets)
            .after(CoreSystemPhase::Update)
            .before(RenderSystemPhase::UpdateCamera);
        schedule
            .add_system(camera::update_cameras_from_projections)
            .into_phase(RenderSystemPhase::UpdateCamera);
        // SORTING after update UPDATE
        schedule.add_phase_dependency(CoreSystemPhase::Update, RenderSystemPhase::Sorting);
        schedule
            .add_system(RenderGraphBuilder::reset)
            .before(RenderSystemPhase::BuildGraph);
        schedule
            .add_system(RenderGraph::build_from_builder)
            .after(RenderSystemPhase::BuildGraph)
            .before(RenderSystemPhase::UpdateGraph);
    }
}
