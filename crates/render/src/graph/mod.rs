use core::fmt;

use self::{
    access::Access,
    pass::{run::PassExec, PipelineBindPoint},
    resources::{ExtendedResourceData, Resource, ResourceDeps, ResourceSet},
};
use crate::{
    buffer::Buffer,
    draw::{DrawContext, DrawPhases},
    texture::Texture,
};

pub mod access;
#[macro_use]
pub mod resources;
pub mod builder;
pub mod deps;
pub mod pass;

pub type ResourceIndex = u16;
pub type PassIndex = u16;
type SubPassIndex = (u16, u16);

const PASS_UNDEFINED: PassIndex = !0;
const SUBPASS_UNDEFINED: SubPassIndex = (!0, !0);

#[derive(Hash, Debug)]
pub struct SubPassDescription {
    pass_index: PassIndex,
    name: &'static str,
    color_attachments: Vec<(ResourceIndex, Access)>,
    depth_stencil_attachment: Option<(ResourceIndex, Access)>,
    input_attachments: Vec<(ResourceIndex, Access)>,
}

#[derive(Hash, Debug)]
pub struct PassDescription {
    index: PassIndex,
    name: &'static str,
    bind_point: PipelineBindPoint,
    textures: ResourceDeps,
    buffers: ResourceDeps,
    begin_subpasses: usize,
    end_subpasses: usize, // exclusive!
    active: bool,
}

pub struct RenderGraph {
    init: bool,
    hash: u64,
    was_updated: bool,
    textures: ResourceSet<Texture>,
    textures_ext: Vec<ExtendedResourceData>,
    buffers: ResourceSet<Buffer>,
    buffers_ext: Vec<ExtendedResourceData>,
    subpasses: Vec<SubPassDescription>,
    subpasses_exec: Vec<PassExec<()>>,
    passes: Vec<PassDescription>,
    passes_topo_order: Vec<Vec<usize>>,
}

pub struct RenderGraphBuilder {
    is_reset: bool,
    textures: ResourceSet<Texture>,
    buffers: ResourceSet<Buffer>,
    subpasses: Vec<SubPassDescription>,
    subpasses_run: Vec<PassExec<()>>,
    passes: Vec<PassDescription>,
}

impl RenderGraph {
    #[inline]
    const fn new() -> Self {
        Self {
            init: false,
            hash: 0,
            was_updated: false,
            textures: ResourceSet::new(),
            textures_ext: Vec::new(),
            buffers: ResourceSet::new(),
            buffers_ext: Vec::new(),
            subpasses: Vec::new(),
            subpasses_exec: Vec::new(),
            passes: Vec::new(),
            passes_topo_order: Vec::new(),
        }
    }

    #[inline]
    pub const fn was_updated(&self) -> bool {
        self.was_updated
    }

    #[inline]
    pub const fn hash(&self) -> u64 {
        self.hash
    }

    #[inline]
    pub fn get_num_topological_groups(&self) -> usize {
        self.passes_topo_order.len()
    }

    #[inline]
    pub fn get_num_textures(&self) -> usize {
        self.textures.len()
    }

    #[inline]
    pub fn get_num_buffers(&self) -> usize {
        self.buffers.len()
    }

    #[inline]
    pub(crate) fn get_texture_info(&self, index: usize) -> Option<&Resource<Texture>> {
        self.textures.get(index)
    }

    #[inline]
    pub(crate) fn get_buffer_info(&self, index: usize) -> Option<&Resource<Buffer>> {
        self.buffers.get(index)
    }

    pub fn get_topological_group(
        &self,
        group: usize,
    ) -> impl Iterator<Item = &PassDescription> + '_ {
        self.passes_topo_order
            .get(group)
            .into_iter()
            .flatten()
            .map(|g| &self.passes[*g])
    }

    pub fn get_subpass(&self, sub_pass_index: usize) -> Option<&SubPassDescription> {
        self.subpasses.get(sub_pass_index)
    }

    pub fn get_pass(&self, pass_index: PassIndex) -> Option<&PassDescription> {
        self.passes.get(pass_index as usize)
    }

    pub fn execute_sub_pass(
        &self,
        sub_pass_index: usize,
        draw_context: DrawContext<'_>,
        draw_phases: &DrawPhases,
    ) {
        self.subpasses_exec[sub_pass_index].execute(draw_context, draw_phases)
    }
}

impl RenderGraphBuilder {
    #[inline]
    pub const fn new() -> Self {
        Self {
            is_reset: false,
            textures: ResourceSet::new(),
            buffers: ResourceSet::new(),
            subpasses: Vec::new(),
            subpasses_run: Vec::new(),
            passes: Vec::new(),
        }
    }
}

impl Default for RenderGraphBuilder {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Default for RenderGraph {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for RenderGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderGraph")
            .field("init", &self.init)
            .field("hash", &self.hash)
            .field("was_updated", &self.was_updated)
            .field("textures", &self.textures)
            .field("textures_ext", &self.textures_ext)
            .field("buffers", &self.buffers)
            .field("buffers_ext", &self.buffers_ext)
            .field("subpasses", &self.subpasses)
            .field("passes", &self.passes)
            .field("passes_topo_order", &self.passes_topo_order)
            .finish()
    }
}

impl SubPassDescription {
    #[inline]
    pub const fn pass_index(&self) -> PassIndex {
        self.pass_index
    }

    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[inline]
    pub fn color_attachments(&self) -> &[(ResourceIndex, Access)] {
        &self.color_attachments
    }
    #[inline]
    pub fn input_attachments(&self) -> &[(ResourceIndex, Access)] {
        &self.input_attachments
    }
    #[inline]
    pub fn depth_stencil_attachment(&self) -> Option<(ResourceIndex, Access)> {
        self.depth_stencil_attachment
    }
}

impl PassDescription {
    #[inline]
    pub const fn index(&self) -> PassIndex {
        self.index
    }
    #[inline]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[inline]
    pub const fn bind_point(&self) -> PipelineBindPoint {
        self.bind_point
    }

    #[inline]
    pub const fn textures(&self) -> &ResourceDeps {
        &self.textures
    }

    #[inline]
    pub const fn buffers(&self) -> &ResourceDeps {
        &self.buffers
    }

    #[inline]
    pub fn sub_pass_range(&self) -> std::ops::Range<usize> {
        self.begin_subpasses..self.end_subpasses
    }
}
