use std::marker::PhantomData;

use super::{Graphics, Pass, PassGroup, PipelineType, SubPassDescription};
use crate::{
    buffer::{Buffer, BufferUsage},
    graph::{
        access::{ShaderStage, Stage},
        resources::{ResourceDeps, Slot, SlotAccess, WriteSlot},
        PassDescription, PassIndex, RenderGraphBuilder, SubPassIndex,
    },
    texture::{Texture, TextureDimensions, TextureFormat, TextureUsage},
};

impl RenderGraphBuilder {
    pub fn add_pass<Q, P>(&mut self, pass: P) -> P::Output
    where
        Q: PipelineType,
        P: PassGroup<Q>,
    {
        debug_assert!(self.is_reset);
        let begin_subpasses = self.subpasses.len();
        let index = self.passes.len() as PassIndex;
        let mut descr = PassDescription {
            index,
            name: pass.type_name(),
            bind_point: Q::BIND_POINT,
            textures: ResourceDeps::new(),
            buffers: ResourceDeps::new(),
            begin_subpasses,
            end_subpasses: begin_subpasses,
            active: false,
        };

        let output = pass.build(PassGroupBuilder(
            PassBuilderIntern {
                graph: self,
                pass: &mut descr,
                current_subpass: 0,
            },
            PhantomData,
        ));

        // update end marker
        let end_subpasses = self.subpasses.len();
        // only add pass, if not empty
        if begin_subpasses < end_subpasses {
            descr.end_subpasses = end_subpasses;
            self.passes.push(descr);
        }
        output
    }
}

struct PassBuilderIntern<'a> {
    graph: &'a mut RenderGraphBuilder,
    pass: &'a mut PassDescription,
    current_subpass: u16,
}

impl PassBuilderIntern<'_> {
    #[inline]
    fn current_subpass(&self) -> SubPassIndex {
        (self.pass.index, self.current_subpass)
    }

    fn writes_texture_intern(
        &mut self,
        slot: WriteSlot<Texture>,
        stages: Stage,
        usage: TextureUsage,
    ) -> WriteSlot<Texture> {
        let current_subpass = self.current_subpass();
        assert_ne!(
            slot.last_written_by, current_subpass,
            "trying to write to a texture multiple times in the same sub-pass"
        );
        self.pass.textures.access(&slot, true, stages, usage);
        self.graph.textures.writes(slot, current_subpass, usage)
    }

    fn reads_texture_intern(&mut self, slot: Slot<Texture>, stages: Stage, usage: TextureUsage) {
        assert_ne!(
            slot.last_written_by,
            self.current_subpass(),
            "trying to read and write a texture in the same sub-pass"
        );
        self.pass.textures.access(&slot, false, stages, usage);
        self.graph.textures.reads(slot, usage);
    }

    fn writes_buffer_intern(
        &mut self,
        slot: WriteSlot<Buffer>,
        stages: Stage,
        usage: BufferUsage,
    ) -> WriteSlot<Buffer> {
        let current_subpass = self.current_subpass();
        assert_ne!(
            slot.last_written_by, current_subpass,
            "trying to write to a buffer multiple times in the same sub-pass"
        );
        self.pass.buffers.access(&slot, true, stages, usage);
        self.graph.buffers.writes(slot, current_subpass, usage)
    }

    fn reads_buffer_intern(&mut self, slot: Slot<Buffer>, stages: Stage, usage: BufferUsage) {
        assert_ne!(
            slot.last_written_by,
            self.current_subpass(),
            "trying to read and write a buffer in the same sub-pass"
        );
        self.pass.buffers.access(&slot, false, stages, usage);
        self.graph.buffers.reads(slot, usage);
    }
}

pub struct PassGroupBuilder<'a, Q>(PassBuilderIntern<'a>, PhantomData<fn(Q)>);

impl<Q: PipelineType> PassGroupBuilder<'_, Q> {
    #[inline]
    pub fn set_active(&mut self) {
        self.0.pass.active = true;
    }

    #[inline]
    pub(super) fn push_sub_pass<P: Pass<Q>>(&mut self, sub_pass: P) -> P::Output {
        let mut descr = SubPassDescription::new(self.0.pass.index, sub_pass.type_name());
        let (output, run) = sub_pass.build(PassBuilder {
            base: PassBuilderIntern {
                graph: self.0.graph,
                pass: self.0.pass,
                current_subpass: self.0.current_subpass,
            },
            subpass: &mut descr,
            _pipeline_type: PhantomData,
        });
        self.0.current_subpass += 1;
        self.0.graph.subpasses.push(descr);
        self.0.graph.subpasses_run.push(run.erased());
        output
    }
}

impl PassGroupBuilder<'_, Graphics> {
    #[inline]
    pub fn sub_pass<P: Pass<Graphics>>(&mut self, sub_pass: P) -> P::Output {
        self.push_sub_pass(sub_pass)
    }
}

pub struct PassBuilder<'a, Q> {
    base: PassBuilderIntern<'a>,
    subpass: &'a mut SubPassDescription,
    _pipeline_type: PhantomData<fn(Q)>,
}

impl<Q> PassBuilder<'_, Q> {
    #[inline]
    pub fn set_texture_format(&mut self, slot: &Slot<Texture>, format: TextureFormat) {
        self.base.graph.textures.set_format(slot, format);
    }

    #[inline]
    pub fn set_texture_size(&mut self, slot: &Slot<Texture>, size: TextureDimensions) {
        self.base.graph.textures.set_size(slot, size);
    }

    #[inline]
    pub fn set_buffer_size(&mut self, slot: &Slot<Buffer>, size: usize) {
        self.base.graph.buffers.set_size(slot, size);
    }

    #[inline]
    pub fn reads_texture(&mut self, texture: Slot<Texture>, stages: ShaderStage) {
        self.base
            .reads_texture_intern(texture, stages.as_stage(), TextureUsage::SAMPLED)
    }

    #[inline]
    pub fn reads_storage_texture(&mut self, texture: Slot<Texture>, stages: ShaderStage) {
        self.base
            .reads_texture_intern(texture, stages.as_stage(), TextureUsage::STORAGE)
    }

    #[inline]
    pub fn writes_storage_texture(
        &mut self,
        texture: WriteSlot<Texture>,
        stages: ShaderStage,
    ) -> WriteSlot<Texture> {
        self.base
            .writes_texture_intern(texture, stages.as_stage(), TextureUsage::STORAGE)
    }

    #[inline]
    pub fn reads_uniform_buffer(&mut self, buffer: Slot<Buffer>, stages: ShaderStage) {
        self.base
            .reads_buffer_intern(buffer, stages.as_stage(), BufferUsage::UNIFORM)
    }

    #[inline]
    pub fn reads_storage_buffer(&mut self, buffer: Slot<Buffer>, stages: ShaderStage) {
        self.base
            .reads_buffer_intern(buffer, stages.as_stage(), BufferUsage::STORAGE)
    }

    #[inline]
    pub fn writes_storage_buffer(
        &mut self,
        buffer: WriteSlot<Buffer>,
        stages: ShaderStage,
    ) -> WriteSlot<Buffer> {
        self.base
            .writes_buffer_intern(buffer, stages.as_stage(), BufferUsage::STORAGE)
    }
}

impl PassBuilder<'_, Graphics> {
    #[inline]
    pub fn creates_color_attachment(&mut self) -> WriteSlot<Texture> {
        let slot = self.base.graph.textures.create();
        self.color_attachment(slot)
    }
    pub fn color_attachment(&mut self, texture: WriteSlot<Texture>) -> WriteSlot<Texture> {
        self.subpass.color_attachments.push(texture.index());
        self.base.writes_texture_intern(
            texture,
            Stage::COLOR_ATTACHMENT_OUTPUT,
            TextureUsage::COLOR_ATTACHMENT,
        )
    }

    #[inline]
    pub fn creates_depth_stencil_attachment(&mut self) -> WriteSlot<Texture> {
        let slot = self.base.graph.textures.create();
        self.depth_stencil_attachment(slot)
    }
    pub fn depth_stencil_attachment(&mut self, texture: WriteSlot<Texture>) -> WriteSlot<Texture> {
        self.subpass
            .depth_stencil_attachment
            .replace(texture.index());
        // TODO: support early & late fragment tests
        self.base.writes_texture_intern(
            texture,
            Stage::FRAGMENT_TESTS,
            TextureUsage::DEPTH_STENCIL_ATTACHMENT,
        )
    }

    pub fn input_attachment(&mut self, texture: Slot<Texture>) {
        self.subpass.input_attachments.push(texture.index());
        self.base.reads_texture_intern(
            texture,
            Stage::FRAGMENT_SHADER,
            TextureUsage::INPUT_ATTACHMENT,
        )
    }

    #[inline]
    pub fn reads_vertex_buffer(&mut self, buffer: Slot<Buffer>) {
        self.base
            .reads_buffer_intern(buffer, Stage::VERTEX_INPUT, BufferUsage::VERTEX)
    }

    #[inline]
    pub fn reads_index_buffer(&mut self, buffer: Slot<Buffer>) {
        self.base
            .reads_buffer_intern(buffer, Stage::VERTEX_INPUT, BufferUsage::INDEX)
    }
}
