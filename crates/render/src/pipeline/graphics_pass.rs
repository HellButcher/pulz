use pulz_transform::math::USize2;
use serde::{Deserialize, Serialize};

use crate::{
    graph::{
        pass::PipelineBindPoint, resources::PhysicalResources, PassDescription, RenderGraph,
        ResourceIndex,
    },
    texture::{TextureFormat, TextureLayout, TextureUsage},
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
    pub initial_layout: TextureLayout,
    pub final_layout: TextureLayout,
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

pub struct ExtendedGraphicsPassDescriptor {
    pub graphics_pass: GraphicsPassDescriptor,
    pub resource_indices: Vec<u16>,
    pub size: USize2,
}

impl ExtendedGraphicsPassDescriptor {
    pub fn from_graph(
        graph: &RenderGraph,
        physical_resources: &PhysicalResources,
        current_texture_layouts: &mut [TextureLayout],
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
        let mut size = USize2::ZERO;
        for i in attachment_indices.iter().copied() {
            let a = &pass.textures()[i as usize];
            let resource_index = a.resource_index();
            let (_tex, format, samples, dim) = physical_resources
                .get_texture(resource_index)
                .expect("unassigned resource");
            let dim = dim.subimage_extents();
            if size == USize2::ZERO {
                size = dim;
            } else if size != dim {
                // TODO: error handling
                panic!("all framebuffer textures need to have the same dimensions");
            }

            let mut load_store = LoadStoreOps {
                // TODO: provide a way to use DONT_CARE or CLEAR
                load_op: LoadOp::Clear,
                // TODO: is resource used in later pass? then STORE, else DONT_CARE
                store_op: StoreOp::Store,
            };
            if a.is_read() {
                load_store.load_op = LoadOp::Load;
            } else {
                // overide to undefined
                current_texture_layouts[resource_index as usize] = TextureLayout::Undefined;
            }
            let current_texture_layout = current_texture_layouts[resource_index as usize];

            attachments.push(AttachmentDescriptor {
                format,
                samples,
                usage: a.usage(),
                initial_layout: current_texture_layout,
                final_layout: current_texture_layout,
            });

            load_store_ops.push(load_store);
        }

        // map attachment_indices into their actual resource indices
        for i in &mut attachment_indices {
            // pass.textures() is sorted by resource-index!
            *i = pass.textures()[*i as usize].resource_index();
        }
        let mut map_attachment_index = |resource_index: &u16, mut layout: TextureLayout| {
            if layout == TextureLayout::Undefined {
                layout = current_texture_layouts[*resource_index as usize];
            } else {
                current_texture_layouts[*resource_index as usize] = layout;
            };
            let a = attachment_indices
                .binary_search(resource_index)
                .expect("unvalid resource index") as u16;
            attachments[a as usize].final_layout = layout;
            a
        };

        let mut subpasses = Vec::with_capacity(pass.sub_pass_range().len());
        for sp in pass.sub_pass_range() {
            let sp = graph.get_subpass(sp).unwrap();
            let input_attachments = sp
                .input_attachments()
                .iter()
                .map(|r| map_attachment_index(r, TextureLayout::InputAttachment))
                .collect();
            let color_attachments = sp
                .color_attachments()
                .iter()
                .map(|r| map_attachment_index(r, TextureLayout::ColorAttachment))
                .collect();
            let depth_stencil_attachment = sp
                .depth_stencil_attachment()
                .as_ref()
                .map(|r| map_attachment_index(r, TextureLayout::DepthStencilAttachment));
            subpasses.push(SubpassDescriptor {
                input_attachments,
                color_attachments,
                depth_stencil_attachment,
            })
        }

        // TODO: if this pass is the last pass accessing this resource (and resource not extern), then STOREOP = DON'T CARE
        // TODO: if this pass is the last pass accessing this resource, and usage us PRESENT, then finalLayout=PRESENT

        let graphics_pass = GraphicsPassDescriptor {
            attachments,
            load_store_ops,
            subpasses,
        };

        Some(Self {
            graphics_pass,
            resource_indices: attachment_indices,
            size,
        })
    }
}
