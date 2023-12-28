use std::{
    collections::{hash_map::DefaultHasher, VecDeque},
    hash::{Hash, Hasher},
};

use pulz_bitset::BitSet;
use tracing::{debug, trace};

use super::{
    access::ResourceAccess,
    deps::DependencyMatrix,
    resources::{ExtendedResourceData, Slot},
    RenderGraph, RenderGraphBuilder,
};
use crate::{buffer::Buffer, texture::Texture};

pub trait GraphImport<R: ResourceAccess> {
    fn import(&self) -> R::ExternHandle;
}

pub trait GraphExport<R: ResourceAccess> {
    fn export(&self) -> R::ExternHandle;
}

impl RenderGraphBuilder {
    pub fn import_texture<I>(&mut self, import_from: &I) -> Slot<Texture>
    where
        I: GraphImport<Texture>,
    {
        let f = import_from.import();
        self.textures.import(f)
    }

    pub fn import_buffer<I>(&mut self, import_from: &I) -> Slot<Buffer>
    where
        I: GraphImport<Buffer>,
    {
        let f = import_from.import();
        self.buffers.import(f)
    }

    pub fn export_texture<E>(&mut self, slot: Slot<Texture>, export_to: &E)
    where
        E: GraphExport<Texture>,
    {
        let f = export_to.export();
        let last_written_by_pass = slot.last_written_by.0;
        self.passes[last_written_by_pass as usize].active = true;
        self.textures.export(slot, f)
    }

    pub fn export_buffer<E>(&mut self, slot: Slot<Buffer>, export_to: &E)
    where
        E: GraphExport<Buffer>,
    {
        let f = export_to.export();
        let last_written_by_pass = slot.last_written_by.0;
        self.passes[last_written_by_pass as usize].active = true;
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
        self.textures_ext.clear();
        self.buffers.reset();
        self.buffers_ext.clear();
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

        let active = self.build_active_set();

        let mut m = self.build_dependency_matrix(&active);
        m.remove_self_references();

        self.passes_topo_order = m.into_topological_order();

        debug!("Topological order: {:?}", self.passes_topo_order);

        self.update_resource_topo_group_range();

        self.init = true;
    }

    fn build_active_set(&mut self) -> BitSet {
        let mut todo = VecDeque::new();
        let mut active = BitSet::with_capacity_for(self.passes.len());
        for p in &self.passes {
            if p.active {
                todo.push_back(p.index);
                active.insert(p.index as usize);
            }
        }
        while let Some(next) = todo.pop_front() {
            let p = &self.passes[next as usize];
            p.textures.mark_deps(&mut active, &mut todo);
            p.buffers.mark_deps(&mut active, &mut todo);
        }
        active
    }

    fn build_dependency_matrix(&self, active: &BitSet) -> DependencyMatrix {
        let mut m = DependencyMatrix::new(self.passes.len());
        // TODO: only mark used nodes
        for p in &self.passes {
            if active.contains(p.index as usize) {
                p.textures.mark_pass_dependency_matrix(&mut m, p.index);
                p.buffers.mark_pass_dependency_matrix(&mut m, p.index);
            }
        }
        m
    }

    fn update_resource_topo_group_range(&mut self) {
        self.textures_ext
            .resize_with(self.textures.len(), ExtendedResourceData::new);
        self.buffers_ext
            .resize_with(self.buffers.len(), ExtendedResourceData::new);
        for (i, group) in self.passes_topo_order.iter().enumerate() {
            for p in group.iter().copied() {
                let p = &self.passes[p];
                p.textures
                    .update_resource_topo_group_range(&mut self.textures_ext, i as u16);
                p.buffers
                    .update_resource_topo_group_range(&mut self.buffers_ext, i as u16);
            }
        }
    }
}

fn swap_graph_data(builder: &mut RenderGraphBuilder, dest: &mut RenderGraph) {
    std::mem::swap(&mut builder.textures, &mut dest.textures);
    std::mem::swap(&mut builder.buffers, &mut dest.buffers);
    std::mem::swap(&mut builder.subpasses, &mut dest.subpasses);
    std::mem::swap(&mut builder.subpasses_run, &mut dest.subpasses_exec);
    std::mem::swap(&mut builder.passes, &mut dest.passes);
}
