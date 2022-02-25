use crate::{
    buffer::{BufferDescriptor, BufferId},
    draw::DrawCommandEncoder,
    pass::GraphicsPassDescriptor,
    pipeline::{
        BindGroupLayoutDescriptor, BindGroupLayoutId, ComputePipelineDescriptor, ComputePipelineId,
        GraphicsPipelineDescriptor, GraphicsPipelineId, PipelineLayoutDescriptor, PipelineLayoutId,
    },
    render_resource::RenderBackendResources,
    shader::{ShaderModuleDescriptor, ShaderModuleId},
    texture::{Image, TextureDescriptor, TextureId},
};

pub trait RenderBackend: 'static {
    fn create_buffer(&mut self, descriptor: &BufferDescriptor) -> BufferId;
    fn create_texture(&mut self, descriptor: &TextureDescriptor) -> TextureId;
    fn create_shader_module(&mut self, descriptor: &ShaderModuleDescriptor<'_>) -> ShaderModuleId;
    fn create_bind_group_layout(
        &mut self,
        descriptor: &BindGroupLayoutDescriptor<'_>,
    ) -> BindGroupLayoutId;
    fn create_pipeline_layout(
        &mut self,
        descriptor: &PipelineLayoutDescriptor<'_>,
    ) -> PipelineLayoutId;
    fn create_compute_pipeline(
        &mut self,
        descriptor: &ComputePipelineDescriptor<'_>,
    ) -> ComputePipelineId;
    fn create_graphics_pipeline(
        &mut self,
        descriptor: &GraphicsPipelineDescriptor<'_>,
    ) -> GraphicsPipelineId;

    fn write_image(&self, texture: TextureId, image: &Image);

    fn destroy_buffer(&mut self, id: BufferId);
    fn destroy_texture(&mut self, id: TextureId);
    fn destroy_shader_module(&mut self, id: ShaderModuleId);
}

pub trait RenderBackendTypes: RenderBackend {
    type Buffer;
    type Texture;
    type ShaderModule;
    type BindGroupLayout;
    type PipelineLayout;
    type ComputePipeline;
    type GraphicsPipeline;

    fn resources(&self) -> &RenderBackendResources<Self>;
    fn resources_mut(&mut self) -> &mut RenderBackendResources<Self>;
}

pub trait CommandEncoder {
    fn insert_debug_marker(&mut self, label: &str);
    fn push_debug_group(&mut self, label: &str);
    fn pop_debug_group(&mut self);
    fn graphics_pass(
        &mut self,
        descriptor: &GraphicsPassDescriptor<'_>,
        pass: &mut dyn FnMut(&mut dyn DrawCommandEncoder),
    );
}
