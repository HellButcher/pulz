use self::{
    pass::{run::PassExec, PipelineBindPoint},
    resources::{Buffer, ResourceDeps, ResourceSet, SlotAccess, Texture},
};
use crate::draw::{DrawContext, DrawPhases};

pub mod access;
#[macro_use]
pub mod resources;
pub mod builder;
pub mod deps;
pub mod pass;

#[derive(Hash)]
pub struct PassDescription {
    index: usize,
    group_index: usize,
    name: &'static str,
    bind_point: PipelineBindPoint,
    textures: ResourceDeps<Texture>,
    buffers: ResourceDeps<Buffer>,
    color_attachments: Vec<u16>,
    depth_stencil_attachments: Option<u16>,
    input_attachments: Vec<u16>,
}

#[derive(Hash)]
pub struct PassGroupDescription {
    index: usize,
    name: &'static str,
    bind_point: PipelineBindPoint,
    begin_passes: usize,
    end_passes: usize, // exclusive!
}

pub struct RenderGraph {
    init: bool,
    hash: u64,
    was_updated: bool,
    textures: ResourceSet<Texture>,
    buffers: ResourceSet<Buffer>,
    passes: Vec<PassDescription>,
    passes_exec: Vec<PassExec<()>>,
    groups: Vec<PassGroupDescription>,
    topo_order: Vec<Vec<usize>>,
}

pub struct RenderGraphBuilder {
    is_reset: bool,
    textures: ResourceSet<Texture>,
    buffers: ResourceSet<Buffer>,
    passes: Vec<PassDescription>,
    passes_run: Vec<PassExec<()>>,
    groups: Vec<PassGroupDescription>,
}

impl RenderGraph {
    #[inline]
    const fn new() -> Self {
        Self {
            init: false,
            hash: 0,
            was_updated: false,
            textures: ResourceSet::new(),
            buffers: ResourceSet::new(),
            passes: Vec::new(),
            passes_exec: Vec::new(),
            groups: Vec::new(),
            topo_order: Vec::new(),
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
        self.topo_order.len()
    }

    pub fn get_topological_group(
        &self,
        group: usize,
    ) -> impl Iterator<Item = &PassGroupDescription> + '_ {
        self.topo_order
            .get(group)
            .into_iter()
            .flatten()
            .map(|g| &self.groups[*g])
    }

    #[inline]
    pub fn get_num_passes(&self) -> usize {
        self.passes.len()
    }

    pub fn get_pass(&self, pass: usize) -> Option<&PassDescription> {
        self.passes.get(pass)
    }

    #[inline]
    pub fn get_num_pass_groups(&self) -> usize {
        self.groups.len()
    }

    pub fn get_pass_group(&self, pass: usize) -> Option<&PassGroupDescription> {
        self.groups.get(pass)
    }

    pub fn execute_pass(
        &self,
        pass: usize,
        draw_context: DrawContext<'_>,
        draw_phases: &DrawPhases,
    ) {
        self.passes_exec[pass].execute(draw_context, draw_phases)
    }
}

impl RenderGraphBuilder {
    #[inline]
    pub const fn new() -> Self {
        Self {
            is_reset: false,
            textures: ResourceSet::new(),
            buffers: ResourceSet::new(),
            passes: Vec::new(),
            passes_run: Vec::new(),
            groups: Vec::new(),
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

impl PassDescription {
    #[inline]
    pub const fn index(&self) -> usize {
        self.index
    }

    #[inline]
    pub const fn group_index(&self) -> usize {
        self.group_index
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
    pub const fn textures(&self) -> &ResourceDeps<Texture> {
        &self.textures
    }

    #[inline]
    pub const fn buffers(&self) -> &ResourceDeps<Buffer> {
        &self.buffers
    }
}

impl PassGroupDescription {
    #[inline]
    pub const fn group_index(&self) -> usize {
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
    pub fn range(&self) -> std::ops::Range<usize> {
        self.begin_passes..self.end_passes
    }
}
