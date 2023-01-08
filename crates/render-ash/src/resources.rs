use ash::vk;
use pulz_render::{
    backend::GpuResource,
    buffer::Buffer,
    pipeline::{
        BindGroupLayout, ComputePipeline, GraphicsPipeline, PipelineLayout, RayTracingPipeline,
    },
    shader::ShaderModule,
    texture::Texture,
};
use slotmap::SlotMap;

use crate::{
    convert::{CreateInfoConverter, VkInto},
    device::AshDevice,
    shader::compie_into_spv,
    Result,
};

pub trait AshGpuResource: GpuResource {
    type Raw;

    unsafe fn create(
        device: &AshDevice,
        res: &AshResources,
        descriptor: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw>;
    unsafe fn create_many(
        device: &AshDevice,
        res: &AshResources,
        descriptors: &[Self::Descriptor<'_>],
    ) -> Result<Vec<Self::Raw>> {
        descriptors
            .iter()
            .map(|d| Self::create(device, res, d))
            .collect()
    }
    unsafe fn destroy(device: &AshDevice, raw: Self::Raw);
}

impl AshGpuResource for Buffer {
    type Raw = vk::Buffer;

    unsafe fn create(
        device: &AshDevice,
        _res: &AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let create_info: vk::BufferCreateInfo = descr.vk_into();
        let raw = device.create_buffer(&create_info, None)?;
        Ok(raw)
    }

    unsafe fn destroy(device: &AshDevice, raw: Self::Raw) {
        if raw != vk::Buffer::null() {
            device.destroy_buffer(raw, None);
        }
    }
}

impl AshGpuResource for Texture {
    type Raw = (vk::Image, vk::ImageView);

    unsafe fn create(
        device: &AshDevice,
        _res: &AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let img_create_info: vk::ImageCreateInfo = descr.vk_into();
        let img = device.create(&img_create_info)?;
        let view_create_info: vk::ImageViewCreateInfo = descr.vk_into();
        let view = device.create(&view_create_info)?;
        Ok((img.take(), view.take()))
    }

    unsafe fn destroy(device: &AshDevice, (img, view): Self::Raw) {
        if view != vk::ImageView::null() {
            device.destroy(view);
        }
        if img != vk::Image::null() {
            device.destroy(img);
        }
    }
}

impl AshGpuResource for ShaderModule {
    type Raw = vk::ShaderModule;
    unsafe fn create(
        device: &AshDevice,
        _res: &AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let code = compie_into_spv(&descr.source)?;
        let create_info = vk::ShaderModuleCreateInfo::builder().code(&code).build();
        let raw = device.create(&create_info)?;
        if let Some(label) = descr.label {
            device.object_name(raw.raw(), label);
        }
        Ok(raw.take())
    }
    unsafe fn destroy(device: &AshDevice, raw: Self::Raw) {
        if raw != vk::ShaderModule::null() {
            device.destroy(raw);
        }
    }
}
impl AshGpuResource for BindGroupLayout {
    type Raw = vk::DescriptorSetLayout;
    unsafe fn create(
        device: &AshDevice,
        _res: &AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let mut conv = CreateInfoConverter::new();
        let create_info = conv.bind_group_layout(descr);
        let raw = device.create(create_info)?;
        if let Some(label) = descr.label {
            device.object_name(raw.raw(), label);
        }
        Ok(raw.take())
    }
    unsafe fn destroy(device: &AshDevice, raw: Self::Raw) {
        if raw != vk::DescriptorSetLayout::null() {
            device.destroy(raw);
        }
    }
}
impl AshGpuResource for PipelineLayout {
    type Raw = vk::PipelineLayout;

    unsafe fn create(
        device: &AshDevice,
        res: &AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let mut conv = CreateInfoConverter::new();
        let create_info = conv.pipeline_layout(res, descr);
        let raw = device.create(create_info)?;
        if let Some(label) = descr.label {
            device.object_name(raw.raw(), label);
        }
        Ok(raw.take())
    }
    unsafe fn destroy(device: &AshDevice, raw: Self::Raw) {
        if raw != vk::PipelineLayout::null() {
            device.destroy(raw);
        }
    }
}
impl AshGpuResource for GraphicsPipeline {
    type Raw = vk::Pipeline;

    unsafe fn create_many(
        device: &AshDevice,
        res: &AshResources,
        descrs: &[Self::Descriptor<'_>],
    ) -> Result<Vec<Self::Raw>> {
        let mut conv = CreateInfoConverter::new();
        let create_infos = conv.graphics_pipeline_descriptor(res, descrs);
        match device.create_graphics_pipelines(vk::PipelineCache::null(), create_infos, None) {
            Ok(raw) => {
                let raw = device.hold(raw);
                if let Ok(debug_utils) = device.instance().ext_debug_utils() {
                    for (i, d) in descrs.iter().enumerate() {
                        if let Some(label) = d.label {
                            debug_utils.object_name(device.handle(), raw[i], label);
                        }
                    }
                }
                Ok(raw.take())
            }
            Err((pipelines, e)) => {
                device.destroy(pipelines);
                Err(e.into())
            }
        }
    }

    unsafe fn create(
        device: &AshDevice,
        res: &AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let raw = Self::create_many(device, res, std::slice::from_ref(descr))?;
        Ok(raw[0])
    }

    unsafe fn destroy(device: &AshDevice, raw: Self::Raw) {
        if raw != vk::Pipeline::null() {
            device.destroy_pipeline(raw, None);
        }
    }
}
impl AshGpuResource for ComputePipeline {
    type Raw = vk::Pipeline;

    unsafe fn create_many(
        device: &AshDevice,
        res: &AshResources,
        descrs: &[Self::Descriptor<'_>],
    ) -> Result<Vec<Self::Raw>> {
        let mut conv = CreateInfoConverter::new();
        let create_infos = conv.compute_pipeline_descriptor(res, descrs);
        match device.create_compute_pipelines(vk::PipelineCache::null(), create_infos, None) {
            Ok(raw) => {
                let raw = device.hold(raw);
                if let Ok(debug_utils) = device.instance().ext_debug_utils() {
                    for (i, d) in descrs.iter().enumerate() {
                        if let Some(label) = d.label {
                            debug_utils.object_name(device.handle(), raw[i], label);
                        }
                    }
                }
                Ok(raw.take())
            }
            Err((pipelines, e)) => {
                device.destroy(pipelines);
                Err(e.into())
            }
        }
    }

    unsafe fn create(
        device: &AshDevice,
        res: &AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let raw = Self::create_many(device, res, std::slice::from_ref(descr))?;
        Ok(raw[0])
    }

    unsafe fn destroy(device: &AshDevice, raw: Self::Raw) {
        if raw != vk::Pipeline::null() {
            device.destroy_pipeline(raw, None);
        }
    }
}
impl AshGpuResource for RayTracingPipeline {
    type Raw = vk::Pipeline;

    unsafe fn create_many(
        device: &AshDevice,
        res: &AshResources,
        descrs: &[Self::Descriptor<'_>],
    ) -> Result<Vec<Self::Raw>> {
        let ext = device.ext_raytracing_pipeline()?;
        let mut conv = CreateInfoConverter::new();
        let create_infos = conv.ray_tracing_pipeline_descriptor(res, descrs);
        let raw = ext.create_ray_tracing_pipelines(
            vk::DeferredOperationKHR::null(),
            vk::PipelineCache::null(),
            create_infos,
            None,
        )?;
        let raw = device.hold(raw);
        if let Ok(debug_utils) = device.instance().ext_debug_utils() {
            for (i, d) in descrs.iter().enumerate() {
                if let Some(label) = d.label {
                    debug_utils.object_name(device.handle(), raw[i], label);
                }
            }
        }
        Ok(raw.take())
    }

    unsafe fn create(
        device: &AshDevice,
        res: &AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let raw = Self::create_many(device, res, std::slice::from_ref(descr))?;
        Ok(raw[0])
    }

    unsafe fn destroy(device: &AshDevice, raw: Self::Raw) {
        if raw != vk::Pipeline::null() {
            device.destroy_pipeline(raw, None);
        }
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

        impl Drop for AshResources {
            fn drop(&mut self) {
                fn check_empty<R: AshGpuResource>(map: &SlotMap<R, R::Raw>) {
                    if !map.is_empty() {
                        panic!(
                            "gpu resources for {} dropped without calling `destroy`",
                            std::any::type_name::<R>()
                        );
                    }
                }

                $(
                    check_empty(&self.$namefield);
                )*
            }
        }
    };
}

define_resources! {
    pub struct AshResources {
        pub buffers<Buffer, vk::Buffer>,
        pub textures<Texture, (vk::Image, vk::ImageView)>,
        pub shader_modules<ShaderModule, vk::ShaderModule>,
        pub bind_group_layouts<BindGroupLayout, vk::DescriptorSetLayout>,
        pub pipeline_layouts<PipelineLayout, vk::PipelineLayout>,
        pub graphics_pipelines<GraphicsPipeline, vk::Pipeline>,
        pub compute_pipelines<ComputePipeline, vk::Pipeline>,
        pub ray_tracing_pipelines<RayTracingPipeline, vk::Pipeline>,
    }
}

impl AshResources {
    pub fn create<R>(&mut self, device: &AshDevice, descriptor: &R::Descriptor<'_>) -> Result<R>
    where
        R: AshGpuResource,
        Self: AsMut<SlotMap<R, R::Raw>>,
    {
        let raw = unsafe { R::create(device, self, descriptor)? };
        let key = self.as_mut().insert(raw);
        Ok(key)
    }

    pub fn create_many<R>(
        &mut self,
        device: &AshDevice,
        descriptors: &[R::Descriptor<'_>],
    ) -> Result<Vec<R>>
    where
        R: AshGpuResource,
        Self: AsMut<SlotMap<R, R::Raw>>,
    {
        let raw = unsafe { R::create_many(device, self, descriptors)? };
        let keys = raw.into_iter().map(|r| self.as_mut().insert(r)).collect();
        Ok(keys)
    }

    pub fn destroy<R>(&mut self, device: &AshDevice, key: R) -> bool
    where
        R: AshGpuResource,
        Self: AsMut<SlotMap<R, R::Raw>>,
    {
        if let Some(raw) = self.as_mut().remove(key) {
            unsafe { R::destroy(device, raw) };
            true
        } else {
            false
        }
    }

    pub fn destroy_all(&mut self, device: &AshDevice) {
        fn destroy_all_in_map<R: AshGpuResource>(map: &mut SlotMap<R, R::Raw>, device: &AshDevice) {
            for (_key, raw) in map.drain() {
                unsafe { R::destroy(device, raw) };
            }
        }

        // Reverse order!
        destroy_all_in_map(&mut self.ray_tracing_pipelines, device);
        destroy_all_in_map(&mut self.compute_pipelines, device);
        destroy_all_in_map(&mut self.graphics_pipelines, device);
        destroy_all_in_map(&mut self.pipeline_layouts, device);
        destroy_all_in_map(&mut self.bind_group_layouts, device);
        destroy_all_in_map(&mut self.shader_modules, device);
        destroy_all_in_map(&mut self.textures, device);
        destroy_all_in_map(&mut self.buffers, device);
    }
}
