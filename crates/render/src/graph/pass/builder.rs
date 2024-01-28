use std::marker::PhantomData;

use super::{Graphics, Pass, PassGroup, PipelineType, SubPassDescription};
use crate::{
    buffer::Buffer,
    graph::{
        access::{Access, ShaderStage},
        resources::{ResourceDeps, Slot, SlotAccess, WriteSlot},
        PassDescription, PassIndex, RenderGraphBuilder, SubPassIndex,
    },
    texture::{Texture, TextureDimensions, TextureFormat},
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
        access: Access,
    ) -> WriteSlot<Texture> {
        let current_subpass = self.current_subpass();
        assert_ne!(
            slot.last_written_by, current_subpass,
            "trying to write to a texture multiple times in the same sub-pass"
        );
        self.pass.textures.access(&slot, access);
        self.graph.textures.writes(slot, current_subpass, access)
    }

    fn reads_texture_intern(&mut self, slot: Slot<Texture>, access: Access) {
        assert_ne!(
            slot.last_written_by,
            self.current_subpass(),
            "trying to read and write a texture in the same sub-pass"
        );
        self.pass.textures.access(&slot, access);
        self.graph.textures.reads(slot, access);
    }

    fn writes_buffer_intern(
        &mut self,
        slot: WriteSlot<Buffer>,
        access: Access,
    ) -> WriteSlot<Buffer> {
        let current_subpass = self.current_subpass();
        assert_ne!(
            slot.last_written_by, current_subpass,
            "trying to write to a buffer multiple times in the same sub-pass"
        );
        self.pass.buffers.access(&slot, access);
        self.graph.buffers.writes(slot, current_subpass, access)
    }

    fn reads_buffer_intern(&mut self, slot: Slot<Buffer>, access: Access) {
        assert_ne!(
            slot.last_written_by,
            self.current_subpass(),
            "trying to read and write a buffer in the same sub-pass"
        );
        self.pass.buffers.access(&slot, access);
        self.graph.buffers.reads(slot, access);
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
            .reads_texture_intern(texture, stages.as_access() | Access::SAMPLED_READ)
    }

    #[inline]
    pub fn reads_texture_storage(&mut self, texture: Slot<Texture>, stages: ShaderStage) {
        self.base
            .reads_texture_intern(texture, stages.as_access() | Access::SHADER_READ)
    }

    #[inline]
    pub fn writes_texture_storage(
        &mut self,
        texture: WriteSlot<Texture>,
        stages: ShaderStage,
    ) -> WriteSlot<Texture> {
        self.base
            .writes_texture_intern(texture, stages.as_access() | Access::SHADER_WRITE)
    }

    #[inline]
    pub fn reads_uniform_buffer(&mut self, buffer: Slot<Buffer>, stages: ShaderStage) {
        self.base
            .reads_buffer_intern(buffer, stages.as_access() | Access::UNIFORM_READ)
    }

    #[inline]
    pub fn reads_buffer(&mut self, buffer: Slot<Buffer>, stages: ShaderStage) {
        self.base
            .reads_buffer_intern(buffer, stages.as_access() | Access::SHADER_READ)
    }

    #[inline]
    pub fn writes_buffer(
        &mut self,
        buffer: WriteSlot<Buffer>,
        stages: ShaderStage,
    ) -> WriteSlot<Buffer> {
        self.base
            .writes_buffer_intern(buffer, stages.as_access() | Access::SHADER_WRITE)
    }
}

impl PassBuilder<'_, Graphics> {
    #[inline]
    pub fn creates_color_attachment(&mut self) -> WriteSlot<Texture> {
        let slot = self.base.graph.textures.create();
        self.color_attachment(slot)
    }
    pub fn color_attachment(&mut self, texture: WriteSlot<Texture>) -> WriteSlot<Texture> {
        self.subpass
            .color_attachments
            .push((texture.index(), Access::COLOR_ATTACHMENT_WRITE));
        self.base
            .writes_texture_intern(texture, Access::COLOR_ATTACHMENT_WRITE)
    }

    #[inline]
    pub fn creates_depth_stencil_attachment(&mut self) -> WriteSlot<Texture> {
        let slot = self.base.graph.textures.create();
        self.depth_stencil_attachment(slot)
    }
    #[inline]
    pub fn depth_stencil_attachment(&mut self, texture: WriteSlot<Texture>) -> WriteSlot<Texture> {
        self.write_depth_stencil_attachment_intern(texture, Access::DEPTH_STENCIL_ATTACHMENT_WRITE)
    }

    fn write_depth_stencil_attachment_intern(
        &mut self,
        texture: WriteSlot<Texture>,
        access: Access,
    ) -> WriteSlot<Texture> {
        let old = self
            .subpass
            .depth_stencil_attachment
            .replace((texture.index(), access));
        assert!(old.is_none(), "only one depth stencil attachment allowed");
        // TODO: support early & late fragment tests
        // TODO: support readonly, write depth only, write stencil only
        self.base.writes_texture_intern(texture, access)
    }

    #[inline]
    pub fn color_input_attachment(&mut self, texture: Slot<Texture>) {
        self.input_attachment_intern(texture, Access::COLOR_INPUT_ATTACHMENT_READ)
    }

    #[inline]
    pub fn depth_stencil_input_attachment(&mut self, texture: Slot<Texture>) {
        self.input_attachment_intern(texture, Access::DEPTH_STENCIL_INPUT_ATTACHMENT_READ)
    }

    fn input_attachment_intern(&mut self, texture: Slot<Texture>, access: Access) {
        self.subpass
            .input_attachments
            .push((texture.index(), access));
        self.base.reads_texture_intern(texture, access)
    }

    #[inline]
    pub fn vertex_buffer(&mut self, buffer: Slot<Buffer>) {
        self.base
            .reads_buffer_intern(buffer, Access::VERTEX_ATTRIBUTE_READ)
    }

    #[inline]
    pub fn index_buffer(&mut self, buffer: Slot<Buffer>) {
        self.base.reads_buffer_intern(buffer, Access::INDEX_READ)
    }

    #[inline]
    pub fn indirect_command_buffer(&mut self, buffer: Slot<Buffer>) {
        self.base
            .reads_buffer_intern(buffer, Access::INDIRECT_COMMAND_READ)
    }
}
