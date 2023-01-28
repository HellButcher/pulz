use pulz_render::{
    backend::GpuResource,
    buffer::Buffer,
    pipeline::{BindGroupLayout, ComputePipeline, GraphicsPipeline, PipelineLayout},
    shader::ShaderModule,
    texture::Texture,
};
use slotmap::SlotMap;

use crate::{convert as c, Result};

pub trait WgpuResource: GpuResource + 'static {
    type Wgpu: 'static;

    fn create(
        device: &wgpu::Device,
        res: &WgpuResources,
        descriptor: &Self::Descriptor<'_>,
    ) -> Result<Self::Wgpu>;
}

impl WgpuResource for Buffer {
    type Wgpu = wgpu::Buffer;

    fn create(
        device: &wgpu::Device,
        _res: &WgpuResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Wgpu> {
        let descr = c::convert_buffer_descriptor(descr);
        let raw = device.create_buffer(&descr);
        Ok(raw)
    }
}

impl WgpuResource for Texture {
    type Wgpu = (wgpu::Texture, wgpu::TextureView);

    fn create(
        device: &wgpu::Device,
        _res: &WgpuResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Wgpu> {
        let tex_descr = c::convert_texture_descriptor(descr)?;
        let tex = device.create_texture(&tex_descr);
        let view_descr = c::convert_texture_view_descriptor(descr);
        let view = tex.create_view(&view_descr);
        Ok((tex, view))
    }
}

impl WgpuResource for ShaderModule {
    type Wgpu = wgpu::ShaderModule;

    fn create(
        device: &wgpu::Device,
        _res: &WgpuResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Wgpu> {
        let descr = c::convert_shader_module_descriptor(descr);
        let raw = device.create_shader_module(descr);
        Ok(raw)
    }
}

impl WgpuResource for BindGroupLayout {
    type Wgpu = wgpu::BindGroupLayout;

    fn create(
        device: &wgpu::Device,
        _res: &WgpuResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Wgpu> {
        let mut tmp = Vec::new();
        let descr = c::convert_bind_group_layout_descriptor(descr, &mut tmp);
        let raw = device.create_bind_group_layout(&descr);
        Ok(raw)
    }
}

impl WgpuResource for PipelineLayout {
    type Wgpu = wgpu::PipelineLayout;

    fn create(
        device: &wgpu::Device,
        res: &WgpuResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Wgpu> {
        let mut tmp = Vec::new();
        let descr = c::convert_pipeline_layout_descriptor(res, descr, &mut tmp);
        let raw = device.create_pipeline_layout(&descr);
        Ok(raw)
    }
}

impl WgpuResource for GraphicsPipeline {
    type Wgpu = wgpu::RenderPipeline;

    fn create(
        device: &wgpu::Device,
        res: &WgpuResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Wgpu> {
        let mut tmp1 = Vec::new();
        let mut tmp2 = Vec::new();
        let mut tmp3 = Vec::new();
        let descr =
            c::convert_graphics_pipeline_descriptor(res, descr, &mut tmp1, &mut tmp2, &mut tmp3)?;
        let raw = device.create_render_pipeline(&descr);
        Ok(raw)
    }
}

impl WgpuResource for ComputePipeline {
    type Wgpu = wgpu::ComputePipeline;

    fn create(
        device: &wgpu::Device,
        res: &WgpuResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Wgpu> {
        let descr = c::convert_compute_pipeline_descriptor(res, descr)?;
        let raw = device.create_compute_pipeline(&descr);
        Ok(raw)
    }
}

macro_rules! define_resources {
    (
        $v:vis struct $name:ident {
            $(
                $vfield:vis $namefield:ident<$keytype:ty, $ashtype:ty>
            ),*
            $(,)?
        }
    ) => {
        $v struct $name {
            $(
                $vfield $namefield: ::slotmap::basic::SlotMap<$keytype, $ashtype>
            ),*
        }

        impl $name {
            pub fn new() -> Self {
                Self {
                    $(
                        $namefield: ::slotmap::basic::SlotMap::with_key(),
                    )*
                }
            }

        }

        $(
            impl AsRef<::slotmap::basic::SlotMap<$keytype,$ashtype>> for $name {
                fn as_ref(&self) -> &::slotmap::basic::SlotMap<$keytype,$ashtype> {
                    &self.$namefield
                }
            }

            impl AsMut<::slotmap::basic::SlotMap<$keytype,$ashtype>> for $name {
                fn as_mut(&mut self) -> &mut ::slotmap::basic::SlotMap<$keytype,$ashtype> {
                    &mut self.$namefield
                }
            }
        )*
    };
}

define_resources! {
    pub struct WgpuResources {
        pub buffers<Buffer, wgpu::Buffer>,
        pub textures<Texture, (wgpu::Texture, wgpu::TextureView)>,
        pub shader_modules<ShaderModule, wgpu::ShaderModule>,
        pub bind_group_layouts<BindGroupLayout, wgpu::BindGroupLayout>,
        pub pipeline_layouts<PipelineLayout, wgpu::PipelineLayout>,
        pub render_pipelines<GraphicsPipeline, wgpu::RenderPipeline>,
        pub compute_pipelines<ComputePipeline, wgpu::ComputePipeline>,
    }
}

impl WgpuResources {
    pub fn create<R>(&mut self, device: &wgpu::Device, descriptor: &R::Descriptor<'_>) -> Result<R>
    where
        R: WgpuResource,
        Self: AsMut<SlotMap<R, R::Wgpu>>,
    {
        let raw = R::create(device, self, descriptor)?;
        let key = self.as_mut().insert(raw);
        Ok(key)
    }
}
