use std::marker::PhantomData;

use super::{Graphics, Pass, PassDescription, PassGroup, PipelineType};
use crate::graph::{
    access::Stage,
    resources::{Buffer, BufferUsage, Slot, SlotAccess, Texture, TextureUsage, WriteSlot},
    PassGroupDescription, RenderGraphBuilder,
};

impl RenderGraphBuilder {
    pub fn add_pass<Q: PipelineType, P: PassGroup<Q>>(&mut self, pass_group: P) -> P::Output {
        debug_assert!(self.is_reset);
        let begin_passes = self.passes.len();
        let group_index = self.groups.len();
        self.groups.push(PassGroupDescription {
            index: group_index,
            name: pass_group.type_name(),
            bind_point: Q::BIND_POINT,
            begin_passes,
            end_passes: begin_passes,
        });

        let output = pass_group.build(PassGroupBuilder {
            base: self,
            group_index,
            _pipeline_type: PhantomData,
        });

        // update end marker
        let end_passes = self.passes.len();
        if begin_passes == end_passes {
            // group was empty, remove it!
            self.groups.pop();
        } else {
            self.groups[group_index].end_passes = end_passes;
        }
        output
    }
}

pub struct PassGroupBuilder<'a, Q> {
    base: &'a mut RenderGraphBuilder,
    group_index: usize,
    _pipeline_type: PhantomData<fn(Q)>,
}
impl<Q> PassGroupBuilder<'_, Q> {
    #[inline]
    pub fn creates_texture(&mut self) -> WriteSlot<Texture> {
        self.base.textures.create()
    }

    #[inline]
    pub fn creates_buffer(&mut self) -> WriteSlot<Buffer> {
        self.base.buffers.create()
    }
}

impl<Q: PipelineType> PassGroupBuilder<'_, Q> {
    #[inline]
    pub(super) fn push_pass<P: Pass<Q>>(&mut self, pass: P) -> P::Output {
        let index = self.base.passes.len();
        let mut descr =
            PassDescription::new(index, self.group_index, pass.type_name(), Q::BIND_POINT);
        let (output, run) = pass.build(PassBuilder {
            base: self.base,
            pass: &mut descr,
            _pipeline_type: PhantomData,
        });
        self.base.passes.push(descr);
        self.base.passes_run.push(run.erased());
        output
    }
}

impl PassGroupBuilder<'_, Graphics> {
    #[inline]
    pub fn sub_pass<P: Pass<Graphics>>(&mut self, pass: P) -> P::Output {
        self.push_pass(pass)
    }
}

pub struct PassBuilder<'a, Q> {
    base: &'a mut RenderGraphBuilder,
    pass: &'a mut PassDescription,
    _pipeline_type: PhantomData<fn(Q)>,
}

impl<Q> PassBuilder<'_, Q> {
    #[inline]
    pub fn creates_texture(&mut self, usage: TextureUsage, stages: Stage) -> WriteSlot<Texture> {
        let slot = self.base.textures.create();
        self.writes_texture(slot, stages, usage)
    }

    #[inline]
    pub fn writes_or_creates_texture(
        &mut self,
        slot: Option<WriteSlot<Texture>>,
        stages: Stage,
        usage: TextureUsage,
    ) -> WriteSlot<Texture> {
        let slot = slot.unwrap_or_else(|| self.base.textures.create());
        self.writes_texture(slot, stages, usage)
    }

    pub fn writes_texture(
        &mut self,
        slot: WriteSlot<Texture>,
        stages: Stage,
        usage: TextureUsage,
    ) -> WriteSlot<Texture> {
        self.pass.textures.access(&slot, true, stages, usage);
        let last_written_by_pass = slot.last_written_by_pass as usize;
        if last_written_by_pass != self.pass.index {
            return self.base.textures.writes(slot, self.pass.index);
        }
        slot
    }

    #[inline]
    pub fn reads_texture(&mut self, slot: Slot<Texture>, stages: Stage, usage: TextureUsage) {
        self.pass.textures.access(&slot, false, stages, usage);
        let last_written_by_pass = slot.last_written_by_pass as usize;
        if last_written_by_pass != self.pass.index {
            self.base.textures.reads(slot);
        }
    }

    #[inline]
    pub fn creates_buffer(&mut self, usage: BufferUsage, stages: Stage) -> WriteSlot<Buffer> {
        let slot = self.base.buffers.create();
        self.writes_buffer(slot, stages, usage)
    }

    #[inline]
    pub fn writes_or_creates_buffer(
        &mut self,
        slot: Option<WriteSlot<Buffer>>,
        stages: Stage,
        usage: BufferUsage,
    ) -> WriteSlot<Buffer> {
        let slot = slot.unwrap_or_else(|| self.base.buffers.create());
        self.writes_buffer(slot, stages, usage)
    }

    pub fn writes_buffer(
        &mut self,
        slot: WriteSlot<Buffer>,
        stages: Stage,
        usage: BufferUsage,
    ) -> WriteSlot<Buffer> {
        self.pass.buffers.access(&slot, true, stages, usage);
        let last_written_by_pass = slot.last_written_by_pass as usize;
        if last_written_by_pass != self.pass.index {
            return self.base.buffers.writes(slot, self.pass.index);
        }
        slot
    }

    pub fn reads_buffer(&mut self, slot: Slot<Buffer>, stages: Stage, usage: BufferUsage) {
        self.pass.buffers.access(&slot, false, stages, usage);
        let last_written_by_pass = slot.last_written_by_pass as usize;
        if last_written_by_pass != self.pass.index {
            self.base.buffers.reads(slot);
        }
    }
}

impl PassBuilder<'_, Graphics> {
    #[inline]
    pub fn creates_color_attachment(&mut self) -> WriteSlot<Texture> {
        let slot = self.base.textures.create();
        self.color_attachment(slot)
    }

    pub fn color_attachment(&mut self, texture: WriteSlot<Texture>) -> WriteSlot<Texture> {
        self.pass.color_attachments.push(texture.index());
        self.writes_texture(
            texture,
            Stage::COLOR_ATTACHMENT_OUTPUT,
            TextureUsage::COLOR_ATTACHMENT,
        )
    }

    #[inline]
    pub fn creates_depth_stencil_attachment(&mut self) -> WriteSlot<Texture> {
        let slot = self.base.textures.create();
        self.depth_stencil_attachment(slot)
    }
    pub fn depth_stencil_attachment(&mut self, texture: WriteSlot<Texture>) -> WriteSlot<Texture> {
        self.pass.depth_stencil_attachments.replace(texture.index());
        // TODO: support early & late fragment tests
        self.writes_texture(
            texture,
            Stage::FRAGMENT_TESTS,
            TextureUsage::DEPTH_STENCIL_ATTACHMENT,
        )
    }

    pub fn input_attachment(&mut self, texture: Slot<Texture>) {
        self.pass.input_attachments.push(texture.index());
        self.reads_texture(
            texture,
            Stage::FRAGMENT_SHADER,
            TextureUsage::INPUT_ATTACHMENT,
        )
    }
}
