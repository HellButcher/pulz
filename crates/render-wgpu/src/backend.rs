use std::ops::{Deref, DerefMut};

use pulz_render::backend::CommandEncoder;
use pulz_window::WindowId;

pub enum BackendTexture {
    Texture {
        texture: wgpu::Texture,
        view: wgpu::TextureView,
    },
    Surface {
        window: WindowId,
        view: wgpu::TextureView,
    },
}

impl BackendTexture {
    #[inline]
    pub fn view(&self) -> &wgpu::TextureView {
        match self {
            Self::Texture { view, .. } => view,
            Self::Surface { view, .. } => view,
        }
    }
}

// impl RenderBackend for WgpuRendererBackend {
//     type Error = ConversionError;
//     fn create_buffer(&mut self, desc: &BufferDescriptor) -> Result<BufferId> {
//         let desc = convert_buffer_descriptor(desc);
//         let buffer = self.device.create_buffer(&desc);
//         self.resources.buffers.insert(buffer)
//     }
//     fn create_texture(&mut self, desc: &TextureDescriptor) -> Result<TextureId> {
//         let tex_desc = convert_texture_descriptor(desc);
//         let view_desc = convert_texture_view_descriptor(desc);
//         let texture = self.device.create_texture(&tex_desc);
//         let view = texture.create_view(&view_desc);
//         self.resources
//             .textures
//             .insert(BackendTexture::Texture { texture, view })
//     }
//     fn create_shader_module(&mut self, desc: &ShaderModuleDescriptor<'_>) -> ShaderModuleId {
//         debug!("creating shader module `{:?}`", desc.label);
//         let desc = convert_shader_module_descriptor(desc);
//         let shader_module = self.device.create_shader_module(&desc);
//         self.resources.shader_modules.insert(shader_module)
//     }
//     fn create_bind_group_layout(
//         &mut self,
//         desc: &BindGroupLayoutDescriptor<'_>,
//     ) -> BindGroupLayoutId {
//         let mut tmp1 = Vec::new();
//         let desc = convert_bind_group_layout_descriptor(desc, &mut tmp1);
//         let bind_group_layout = self.device.create_bind_group_layout(&desc);
//         self.resources.bind_group_layouts.insert(bind_group_layout)
//     }
//     fn create_pipeline_layout(&mut self, desc: &PipelineLayoutDescriptor<'_>) -> PipelineLayoutId {
//         let mut tmp1 = Vec::new();
//         let desc = convert_pipeline_layout_descriptor(self.resources(), desc, &mut tmp1);
//         let pipeline_layout = self.device.create_pipeline_layout(&desc);
//         self.resources.pipeline_layouts.insert(pipeline_layout)
//     }
//     fn create_compute_pipeline(
//         &mut self,
//         desc: &ComputePipelineDescriptor<'_>,
//     ) -> ComputePipelineId {
//         let desc = convert_compute_pipeline_descriptor(self.resources(), desc).unwrap();
//         let compute_pipeline = self.device.create_compute_pipeline(&desc);
//         self.resources.compute_pipelines.insert(compute_pipeline)
//     }
//     fn create_graphics_pipeline(
//         &mut self,
//         desc: &GraphicsPipelineDescriptor<'_>,
//     ) -> GraphicsPipelineId {
//         let mut tmp1 = Vec::new();
//         let mut tmp2 = Vec::new();
//         let mut tmp3 = Vec::new();
//         let desc = convert_graphics_pipeline_descriptor(
//             self.resources(),
//             desc,
//             &mut tmp1,
//             &mut tmp2,
//             &mut tmp3,
//         )
//         .unwrap();
//         let graphics_pipeline = self.device.create_render_pipeline(&desc);
//         self.resources.graphics_pipelines.insert(graphics_pipeline)
//     }

//     fn write_image(&self, texture: TextureId, image: &render::texture::Image) {
//         let texture = self
//             .resources
//             .textures
//             .get(texture)
//             .expect("invalid texture handle");
//         let BackendTexture::Texture { texture, .. } = texture else {
//             panic!("trying to write to surface texture");
//         };
//         self.queue.write_texture(
//             ImageCopyTexture {
//                 texture,
//                 mip_level: 1,
//                 origin: Origin3d::ZERO,
//                 aspect: wgpu::TextureAspect::All,
//             },
//             &image.data,
//             image.descriptor.wgpu_into(),
//             image.descriptor.wgpu_into(),
//         );
//     }

//     fn destroy_buffer(&mut self, id: BufferId) {
//         self.resources.buffers.remove(id);
//     }
//     fn destroy_texture(&mut self, id: TextureId) {
//         self.resources.textures.remove(id);
//     }
//     fn destroy_shader_module(&mut self, id: ShaderModuleId) {
//         self.resources.shader_modules.remove(id);
//     }
// }

// impl RenderResourceBackend<render_resource::Buffer> for WgpuRendererBackend {
//     type Error = ();
//     type Target = Buffer;

//     fn create(
//         &mut self,
//         descriptor: &render_resource::BufferDescriptor,
//     ) -> Result<Self::Target, Self::Error> {
//         let desc = convert_buffer_descriptor(desc);
//         let buffer = self.device.create_buffer(&desc);
//         Ok(buffer)
//     }

//     fn destroy(&mut self, buffer: Buffer) {
//         self.device.destroy_buffer(buffer)
//     }
// }

// impl RenderResourceBackend<render_resource::Texture> for WgpuRendererBackend {
//     type Error = ();
//     type Target = BackendTexture;

//     fn create(
//         &mut self,
//         descriptor: &render_resource::TextureDescriptor,
//     ) -> Result<Self::Target, Self::Error> {
//         let tex_desc = convert_texture_descriptor(desc);
//         let view_desc = convert_texture_view_descriptor(desc);
//         let texture = self.device.create_texture(&tex_desc);
//         let view = texture.create_view(&view_desc);
//         Ok(BackendTexture::Texture { texture, view })
//     }

//     fn destroy(&mut self, texture: BackendTexture) {
//         self.device.destroy_texture(texture)
//     }
// }

// impl RenderResourceBackend<render_resource::ShaderModule> for WgpuRendererBackend {
//     type Error = ();
//     type Target = ShaderModule;

//     fn create(
//         &mut self,
//         descriptor: &render_resource::ShaderModuleDescriptor,
//     ) -> Result<Self::Target, Self::Error> {
//         debug!("creating shader module `{:?}`", desc.label);
//         let desc = convert_shader_module_descriptor(desc);
//         let shader_module = self.device.create_shader_module(&desc);
//         Ok(shader_module)
//     }

//     fn destroy(&mut self, module: ShaderModule) {
//         self.device.destroy_shader_module(module)
//     }
// }

// impl RenderBackendTypes for WgpuRendererBackend {
//     type Buffer = Buffer;
//     type Texture = BackendTexture;
//     type ShaderModule = ShaderModule;
//     type BindGroupLayout = BindGroupLayout;
//     type PipelineLayout = PipelineLayout;
//     type GraphicsPipeline = RenderPipeline;
//     type ComputePipeline = ComputePipeline;

//     #[inline]
//     fn resources(&self) -> &RenderBackendResources<Self> {
//         &self.resources
//     }

//     #[inline]
//     fn resources_mut(&mut self) -> &mut RenderBackendResources<Self> {
//         &mut self.resources
//     }
// }

pub struct WgpuCommandEncoder<T>(pub T);

impl<T> Deref for WgpuCommandEncoder<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for WgpuCommandEncoder<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl WgpuCommandEncoder<wgpu::CommandEncoder> {
    pub fn finish(self) -> wgpu::CommandBuffer {
        self.0.finish()
    }
}

impl CommandEncoder for WgpuCommandEncoder<wgpu::CommandEncoder> {
    // fn graphics_pass(
    //     &mut self,
    //     desc: &render::pass::GraphicsPassDescriptor<'_>,
    //     pass_fn: &mut dyn FnMut(&mut dyn render::draw::DrawCommandEncoder),
    // ) {
    //     let mut tmp1 = Vec::new();
    //     let desc = convert_render_pass(self.1, desc, &mut tmp1).unwrap();
    //     let pass = self.0.begin_render_pass(&desc);
    //     let mut pass_encoder = CommandEncoder(pass, self.1);
    //     pass_fn(&mut pass_encoder);
    // }

    fn insert_debug_marker(&mut self, label: &str) {
        self.0.insert_debug_marker(label)
    }

    fn push_debug_group(&mut self, label: &str) {
        self.0.push_debug_group(label)
    }

    fn pop_debug_group(&mut self) {
        self.0.pop_debug_group();
    }
}

impl<'a> CommandEncoder for WgpuCommandEncoder<wgpu::RenderPass<'a>> {
    // fn set_pipeline(&mut self, pipeline: GraphicsPipelineId) {
    //     self.0.set_pipeline(&self.1.graphics_pipelines[pipeline])
    // }
    // fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>) {
    //     self.0.draw_indexed(indices, base_vertex, instances)
    // }
    // fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
    //     self.0.draw(vertices, instances)
    // }

    fn insert_debug_marker(&mut self, label: &str) {
        self.0.insert_debug_marker(label)
    }

    fn push_debug_group(&mut self, label: &str) {
        self.0.push_debug_group(label)
    }

    fn pop_debug_group(&mut self) {
        self.0.pop_debug_group();
    }
}
