use pulz_transform::math::USize2;
use serde::{Deserialize, Serialize};

use crate::{
    graph::{
        pass::PipelineBindPoint, PassDescription, RenderGraph, RenderGraphAssignments,
        ResourceIndex,
    },
    texture::{Texture, TextureFormat, TextureUsage},
};

crate::backend::define_gpu_resource!(GraphicsPass, GraphicsPassDescriptor);

#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum LoadOp {
    #[default]
    Load,
    Clear,
    DontCare,
}

#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum StoreOp {
    #[default]
    Store,
    DontCare,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]

pub struct LoadStoreOps {
    pub load_op: LoadOp,
    pub store_op: StoreOp,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct AttachmentDescriptor {
    pub format: TextureFormat,
    pub usage: TextureUsage,
    //pub initial_layout: TextureLayout,
    pub samples: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct SubpassDescriptor {
    input_attachments: Vec<u16>,
    color_attachments: Vec<u16>,
    depth_stencil_attachment: Option<u16>,
    //resolve_attachments: Vec<usize>,
}

impl SubpassDescriptor {
    #[inline]
    pub fn input_attachments(&self) -> &[u16] {
        &self.input_attachments
    }
    #[inline]
    pub fn color_attachments(&self) -> &[u16] {
        &self.color_attachments
    }
    #[inline]
    pub fn depth_stencil_attachment(&self) -> Option<u16> {
        self.depth_stencil_attachment
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct GraphicsPassDescriptor {
    attachments: Vec<AttachmentDescriptor>,
    load_store_ops: Vec<LoadStoreOps>,
    subpasses: Vec<SubpassDescriptor>,
}

impl GraphicsPassDescriptor {
    #[inline]
    pub fn attachments(&self) -> &[AttachmentDescriptor] {
        &self.attachments
    }
    #[inline]
    pub fn load_store_ops(&self) -> &[LoadStoreOps] {
        &self.load_store_ops
    }
    #[inline]
    pub fn subpasses(&self) -> &[SubpassDescriptor] {
        &self.subpasses
    }
}

pub struct GraphicsPassDescriptorWithTextures {
    pub graphics_pass: GraphicsPassDescriptor,
    pub resource_indices: Vec<u16>,
    pub textures: Vec<Texture>,
    pub size: USize2,
}

impl GraphicsPassDescriptorWithTextures {
    pub fn from_graph(
        graph: &RenderGraph,
        assignments: &RenderGraphAssignments,
        pass: &PassDescription,
    ) -> Option<Self> {
        if pass.bind_point() != PipelineBindPoint::Graphics {
            return None;
        }
        let mut attachment_indices = Vec::with_capacity(pass.textures().len());
        for (i, tex) in pass.textures().deps().iter().enumerate() {
            if tex.usage().is_attachment() {
                attachment_indices.push(i as ResourceIndex);
            }
        }

        let mut attachments = Vec::with_capacity(attachment_indices.len());
        let mut load_store_ops = Vec::with_capacity(attachment_indices.len());
        let mut textures = Vec::with_capacity(attachment_indices.len());
        let mut size = USize2::ZERO;
        for i in attachment_indices.iter().copied() {
            let a = &pass.textures()[i as usize];
            let resource_index = a.resource_index();
            let (tex, format, samples, dim) = assignments
                .get_texture(resource_index)
                .expect("unassigned resource");
            let dim = dim.subimage_extents();
            if size == USize2::ZERO {
                size = dim;
            } else if size != dim {
                // TODO: error handling
                panic!("all framebuffer textures need to have the same dimensions");
            }

            textures.push(tex);

            attachments.push(AttachmentDescriptor {
                format,
                samples,
                usage: a.usage(),
            });
            load_store_ops.push(LoadStoreOps {
                load_op: if a.is_read() {
                    LoadOp::Load
                } else {
                    // TODO: provide a way to use DONT_CARE or CLEAR
                    LoadOp::Clear
                },
                // TODO: is resource used in later pass? then STORE, else DONT_CARE
                store_op: StoreOp::Store,
            });
        }

        // map attachment_indices into their actual resource indices
        for i in &mut attachment_indices {
            // pass.textures() is sorted by resource-index!
            *i = pass.textures()[*i as usize].resource_index();
        }
        let map_attachment_index = |resource_index: &u16| {
            attachment_indices
                .binary_search(resource_index)
                .expect("unvalid resource index") as u16
        };

        let mut subpasses = Vec::with_capacity(pass.sub_pass_range().len());
        for sp in pass.sub_pass_range() {
            let sp = graph.get_subpass(sp).unwrap();
            let input_attachments = sp
                .input_attachments()
                .iter()
                .map(map_attachment_index)
                .collect();
            let color_attachments = sp
                .color_attachments()
                .iter()
                .map(map_attachment_index)
                .collect();
            let depth_stencil_attachment = sp
                .depth_stencil_attachment()
                .as_ref()
                .map(map_attachment_index);
            subpasses.push(SubpassDescriptor {
                input_attachments,
                color_attachments,
                depth_stencil_attachment,
            })
        }

        let graphics_pass = GraphicsPassDescriptor {
            attachments,
            load_store_ops,
            subpasses,
        };

        Some(Self {
            graphics_pass,
            resource_indices: attachment_indices,
            textures,
            size,
        })
    }
}
