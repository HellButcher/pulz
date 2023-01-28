use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use tracing::{debug, trace};

use super::{
    access::ResourceAccess, deps::DependencyMatrix, resources::Slot, RenderGraph,
    RenderGraphBuilder,
};
use crate::{buffer::Buffer, texture::Texture};

pub trait GetExternalResource<R: ResourceAccess> {
    fn get_external_resource(&self) -> (R, R::Meta);
}

impl<R, F> GetExternalResource<R> for F
where
    R: ResourceAccess,
    F: Fn() -> (R, R::Meta),
{
    fn get_external_resource(&self) -> (R, R::Meta) {
        self()
    }
}

pub trait GraphImport {
    type Resource: ResourceAccess;

    fn import(&self) -> Box<dyn GetExternalResource<Self::Resource>>;
}

pub trait GraphExport {
    type Resource: ResourceAccess;

    fn export(&self) -> Box<dyn GetExternalResource<Self::Resource>>;
}

impl RenderGraphBuilder {
    pub fn import_texture<I>(&mut self, import_from: &I) -> Slot<Texture>
    where
        I: GraphImport<Resource = Texture>,
    {
        let f = import_from.import();
        self.textures.import(f)
    }

    pub fn import_buffer<I>(&mut self, import_from: &I) -> Slot<Buffer>
    where
        I: GraphImport<Resource = Buffer>,
    {
        let f = import_from.import();
        self.buffers.import(f)
    }

    pub fn export_texture<E>(&mut self, slot: Slot<Texture>, export_to: &E)
    where
        E: GraphExport<Resource = Texture>,
    {
        let f = export_to.export();
        self.textures.export(slot, f)
    }

    pub fn export_buffer<E>(&mut self, slot: Slot<Buffer>, export_to: &E)
    where
        E: GraphExport<Resource = Buffer>,
    {
        let f = export_to.export();
        self.buffers.export(slot, f)
    }

    fn hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        Hash::hash(&self.textures, &mut hasher);
        Hash::hash(&self.buffers, &mut hasher);
        Hash::hash(&self.subpasses, &mut hasher);
        Hash::hash(&self.passes, &mut hasher);
        hasher.finish()
    }

    pub(crate) fn reset(&mut self) {
        debug_assert!(!self.is_reset);
        self.is_reset = true;
        self.textures.reset();
        self.buffers.reset();
        self.subpasses.clear();
        self.subpasses_run.clear();
        self.passes.clear();
    }
}

impl RenderGraph {
    fn reset(&mut self) {
        self.init = false;
        self.was_updated = true;
        self.textures.reset();
        self.buffers.reset();
        self.subpasses.clear();
        self.subpasses_exec.clear();
        self.passes.clear();
        self.passes_topo_order.clear();
    }

    pub(crate) fn build_from_builder(&mut self, builder: &mut RenderGraphBuilder) {
        debug_assert!(builder.is_reset);
        builder.is_reset = false;

        let builder_hash = builder.hash();
        if self.init
            && builder_hash == self.hash
            && builder.textures.len() == self.textures.len()
            && builder.buffers.len() == self.buffers.len()
            && builder.subpasses.len() == self.subpasses.len()
            && builder.subpasses_run.len() == self.subpasses_exec.len()
            && builder.passes.len() == self.passes.len()
        {
            // graph not changed: swap data from builder (rest stays the same)
            self.was_updated = false;
            swap_graph_data(builder, self);
            trace!("RenderGraph not changed");
            return;
        }

        debug!(
            "Updating RenderGraph with {} passes...",
            builder.subpasses.len()
        );

        self.reset();
        swap_graph_data(builder, self);
        self.hash = builder_hash;

        // TODO: detect unused nodes / dead-stripping

        let mut m = self.build_dependency_matrix();
        m.remove_self_references();

        self.passes_topo_order = m.into_topological_order();

        debug!("Topological order: {:?}", self.passes_topo_order);

        // TODO: resource aliasing (e.g. share Image resource when )

        self.init = true;
    }

    fn build_dependency_matrix(&self) -> DependencyMatrix {
        let mut m = DependencyMatrix::new(self.passes.len());
        // TODO: only mark used nodes
        for p in &self.passes {
            p.textures.mark_pass_dependency_matrix(&mut m, p.index);
            p.buffers.mark_pass_dependency_matrix(&mut m, p.index);
        }
        m
    }
}

fn swap_graph_data(builder: &mut RenderGraphBuilder, dest: &mut RenderGraph) {
    std::mem::swap(&mut builder.textures, &mut dest.textures);
    std::mem::swap(&mut builder.buffers, &mut dest.buffers);
    std::mem::swap(&mut builder.subpasses, &mut dest.subpasses);
    std::mem::swap(&mut builder.subpasses_run, &mut dest.subpasses_exec);
    std::mem::swap(&mut builder.passes, &mut dest.passes);
}
