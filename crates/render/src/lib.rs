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

use assets::Assets;

use ecs::{resource::Resources, schedule::Schedule};
use texture::TextureCache;

pub mod backend;
pub mod buffer;
pub mod cache;
pub mod camera;
pub mod draw;
pub mod mesh;
pub mod pass;
pub mod pipeline;
pub mod render_asset;
pub mod render_graph;
pub mod render_resource;
pub mod shader;
pub mod texture;
pub mod view;

pub mod color {
    pub use palette::{Hsla, LinSrgba, Srgba};
}

pub mod math {
    pub use transform::math::*;
    pub type Point2 = Vec2;
    pub type Size2 = Vec2;
    pub type USize2 = UVec2;
    pub type USize3 = UVec3;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderSystemLabel {
    UpdateGraph,
    RunGraph,
}

pub fn install_into(res: &mut Resources, schedule: &mut Schedule) {
    Assets::<texture::Image>::install_into(res, schedule);
    res.init::<TextureCache>();
    render_graph::graph::RenderGraph::install_into(res, schedule);
}
