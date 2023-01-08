use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use pulz_ecs::prelude::*;
use tracing::{debug, trace, Callsite};

use super::{
    access::ResourceAccess,
    deps::DependencyMatrix,
    resources::{Slot, Texture},
    RenderGraph, RenderGraphBuilder,
};
use crate::buffer::Buffer;

pub trait GraphImport {
    type Resource: ResourceAccess;
}

pub trait GraphExport {
    type Resource: ResourceAccess;
}

impl RenderGraphBuilder {
    pub fn import_texture<I>(&mut self, _import_from: &I) -> Slot<Texture>
    where
        I: GraphImport<Resource = Texture>,
    {
        // TODO: associate resource
        self.textures.import()
    }

    pub fn import_buffer<I>(&mut self, _import_from: &I) -> Slot<Buffer>
    where
        I: GraphImport<Resource = Buffer>,
    {
        // TODO: associate resource
        self.buffers.import()
    }

    pub fn export_texture<E>(&mut self, slot: Slot<Texture>, _export_to: &E)
    where
        E: GraphExport<Resource = Texture>,
    {
        // TODO: associate resource
        self.textures.export(slot)
    }

    pub fn export_buffer<E>(&mut self, slot: Slot<Buffer>, _export_to: &E)
    where
        E: GraphExport<Resource = Buffer>,
    {
        // TODO: associate resource
        self.buffers.export(slot)
    }

    fn hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        Hash::hash(&self.textures, &mut hasher);
        Hash::hash(&self.buffers, &mut hasher);
        Hash::hash(&self.passes, &mut hasher);
        Hash::hash(&self.groups, &mut hasher);
        hasher.finish()
    }

    fn reset(&mut self) {
        self.is_reset = true;
        self.textures.reset();
        self.buffers.reset();
        self.passes.clear();
        self.passes_run.clear();
        self.groups.clear();
    }

    pub(crate) fn build_graph_system(
        mut builder: ResMut<'_, Self>,
        mut graph: ResMut<'_, RenderGraph>,
    ) {
        debug_assert!(builder.is_reset);
        builder.is_reset = false;

        let builder_hash = builder.hash();
        if graph.init
            && builder_hash == graph.hash
            && builder.textures.len() == graph.textures.len()
            && builder.buffers.len() == graph.buffers.len()
            && builder.passes.len() == graph.passes.len()
            && builder.passes_run.len() == graph.passes_exec.len()
            && builder.groups.len() == graph.groups.len()
        {
            // graph not changed: swap data from builder (rest stays the same)
            graph.was_updated = false;
            swap_graph_data(&mut builder, &mut graph);
            trace!("RenderGraph not changed");
            return;
        }

        debug!(
            "Updating RenderGraph with {} passes...",
            builder.passes.len()
        );

        graph.reset();
        swap_graph_data(&mut builder, &mut graph);
        graph.hash = builder_hash;

        // TODO: detect unused nodes / dead-stripping

        let mut m = graph.build_group_dependency_matrix();
        m.remove_self_references();

        graph.topo_order = m.into_topological_order();

        debug!("Topological order: {:?}", graph.topo_order);

        // TODO: resource aliasing (e.g. share Image resource when )

        graph.init = true;
    }

    pub(crate) fn reset_graph_builder(mut builder: ResMut<'_, Self>) {
        debug_assert!(!builder.is_reset);
        builder.reset()
    }
}

impl RenderGraph {
    fn reset(&mut self) {
        self.init = false;
        self.was_updated = true;
        self.textures.reset();
        self.buffers.reset();
        self.passes.clear();
        self.passes_exec.clear();
        self.groups.clear();
        self.topo_order.clear();
    }

    fn build_pass_dependency_matrix(&self) -> DependencyMatrix {
        let mut m = DependencyMatrix::new(self.passes.len());
        // TODO: only mark used nodes
        for p in &self.passes {
            p.textures.mark_pass_dependency_matrix(&mut m, p.index);
            p.buffers.mark_pass_dependency_matrix(&mut m, p.index);
        }
        m
    }

    fn build_group_dependency_matrix(&self) -> DependencyMatrix {
        let mut m = DependencyMatrix::new(self.passes.len());
        // TODO: only mark used nodes
        for p in &self.passes {
            p.textures
                .mark_group_dependency_matrix(&mut m, &self.passes, p.group_index);
        }
        m
    }
}

fn swap_graph_data(builder: &mut RenderGraphBuilder, dest: &mut RenderGraph) {
    std::mem::swap(&mut builder.textures, &mut dest.textures);
    std::mem::swap(&mut builder.buffers, &mut dest.buffers);
    std::mem::swap(&mut builder.passes, &mut dest.passes);
    std::mem::swap(&mut builder.passes_run, &mut dest.passes_exec);
    std::mem::swap(&mut builder.groups, &mut dest.groups);
}
