use pulz_transform::math::USize2;
use serde::{Deserialize, Serialize};

use crate::{
    graph::{
        access::Access,
        pass::PipelineBindPoint,
        resources::{PhysicalResourceAccessTracker, PhysicalResources},
        PassDescription, RenderGraph, ResourceIndex,
    },
    texture::TextureFormat,
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
    pub access: Access,
    pub initial_access: Access,
    pub final_access: Access,
    pub samples: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct SubpassDescriptor {
    input_attachments: Vec<(u16, Access)>,
    color_attachments: Vec<(u16, Access)>,
    depth_stencil_attachment: Option<(u16, Access)>,
    //resolve_attachments: Vec<usize>,
}

impl SubpassDescriptor {
    #[inline]
    pub fn input_attachments(&self) -> &[(u16, Access)] {
        &self.input_attachments
    }
    #[inline]
    pub fn color_attachments(&self) -> &[(u16, Access)] {
        &self.color_attachments
    }
    #[inline]
    pub fn depth_stencil_attachment(&self) -> Option<(u16, Access)> {
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
        resource_access: &mut PhysicalResourceAccessTracker,
        pass: &PassDescription,
    ) -> Option<Self> {
        if pass.bind_point() != PipelineBindPoint::Graphics {
            return None;
        }
        let mut attachment_indices = Vec::with_capacity(pass.textures().len());
        for (i, tex) in pass.textures().deps().iter().enumerate() {
            if tex.access().is_graphics_attachment() {
                attachment_indices.push(i as ResourceIndex);
            }
        }

        let mut attachments = Vec::with_capacity(attachment_indices.len());
        let mut load_store_ops = Vec::with_capacity(attachment_indices.len());
        let mut size = USize2::ZERO;
        for i in attachment_indices.iter().copied() {
            let a = &pass.textures()[i as usize];
            let resource_index = a.resource_index();
            let physical_resource = physical_resources
                .get_texture(resource_index)
                .expect("unassigned resource");
            let dim = physical_resource.size.subimage_extents();
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
            }

            let current_access =
                resource_access.get_current_texture_access(physical_resources, resource_index);

            attachments.push(AttachmentDescriptor {
                format: physical_resource.format,
                samples: 1, // TODO: multi-sample
                access: a.access(),
                initial_access: current_access,
                final_access: current_access,
            });

            load_store_ops.push(load_store);
        }

        // map attachment_indices into their actual resource indices
        for i in &mut attachment_indices {
            // pass.textures() is sorted by resource-index!
            *i = pass.textures()[*i as usize].resource_index();
        }

        let mut map_attachment_index_and_update_usage =
            |resource_index: u16, mut current_access: Access| {
                let a = attachment_indices
                    .binary_search(&resource_index)
                    .expect("unvalid resource index") as u16;
                if current_access.is_empty() {
                    current_access = attachments[a as usize].final_access;
                } else {
                    attachments[a as usize].final_access = current_access;
                }
                (a, current_access)
            };

        let mut subpasses = Vec::with_capacity(pass.sub_pass_range().len());
        for sp in pass.sub_pass_range() {
            let sp = graph.get_subpass(sp).unwrap();
            let input_attachments = sp
                .input_attachments()
                .iter()
                .copied()
                .map(|(r, u)| map_attachment_index_and_update_usage(r, u))
                .collect();
            let color_attachments = sp
                .color_attachments()
                .iter()
                .copied()
                .map(|(r, u)| map_attachment_index_and_update_usage(r, u))
                .collect();
            let depth_stencil_attachment = sp
                .depth_stencil_attachment()
                .map(|(r, u)| map_attachment_index_and_update_usage(r, u));
            subpasses.push(SubpassDescriptor {
                input_attachments,
                color_attachments,
                depth_stencil_attachment,
            })
            // update
        }

        // TODO: if this pass is the last pass accessing this resource (and resource not extern), then STOREOP = DON'T CARE
        // TODO: if this pass is the last pass accessing this resource, and usage us PRESENT, then finalLayout=PRESENT

        for (a, r) in attachment_indices.iter().copied().enumerate() {
            let attachment = &mut attachments[a];
            let physical_resource = physical_resources.get_texture(r).unwrap();
            // TODO: only present, if this is the last pass accessing this resource!
            if physical_resource.access.intersects(Access::PRESENT) {
                attachment.final_access = Access::PRESENT;
            }

            resource_access.update_texture_access(physical_resources, r, attachment.final_access);
        }

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
