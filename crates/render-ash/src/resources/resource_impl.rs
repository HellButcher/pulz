use ash::vk;
use pulz_render::{
    buffer::Buffer,
    pipeline::{
        BindGroupLayout, ComputePipeline, GraphicsPass, GraphicsPipeline, PipelineLayout,
        RayTracingPipeline,
    },
    shader::ShaderModule,
    texture::Texture,
};
use slotmap::SlotMap;

use super::{
    traits::{AshGpuResource, AshGpuResourceCached, AshGpuResourceCreate, AshGpuResourceRemove},
    AshResources, U64HashMap,
};
use crate::{
    alloc::{AshAllocator, GpuMemoryBlock},
    convert::{CreateInfoConverter2, CreateInfoConverter6, VkInto},
    shader::compie_into_spv,
    Result,
};

impl AshGpuResource for Buffer {
    type Raw = (vk::Buffer, Option<GpuMemoryBlock>);
    fn slotmap(res: &AshResources) -> &SlotMap<Self, Self::Raw> {
        &res.buffers
    }

    fn slotmap_mut(res: &mut AshResources) -> &mut SlotMap<Self, Self::Raw> {
        &mut res.buffers
    }
    unsafe fn create_raw(
        res: &mut AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let alloc = &mut res.alloc;
        let device = alloc.device_arc();
        let create_info: vk::BufferCreateInfo<'static> = descr.vk_into();
        let buf = device.create(&create_info)?;
        let mreq = device.get_buffer_memory_requirements(buf.raw());
        let mem = alloc.alloc(gpu_alloc::Request {
            size: mreq.size,
            align_mask: mreq.alignment,
            usage: gpu_alloc::UsageFlags::FAST_DEVICE_ACCESS,
            memory_types: mreq.memory_type_bits,
        })?;
        device.bind_buffer_memory(buf.raw(), *mem.memory(), mem.offset())?;
        Ok((buf.take(), Some(mem.take())))
    }
    unsafe fn destroy_raw(alloc: &mut AshAllocator, (buf, mem): Self::Raw) {
        if buf != vk::Buffer::null() {
            alloc.device().destroy_buffer(buf, None);
        }
        if let Some(mem) = mem {
            alloc.dealloc(mem);
        }
    }
}
impl AshGpuResourceCreate for Buffer {}
impl AshGpuResourceRemove for Buffer {
    fn put_to_garbage(garbage: &mut super::AshFrameGarbage, (buf, mem): Self::Raw) {
        if buf != vk::Buffer::null() {
            garbage.buffers.push(buf);
        }
        if let Some(mem) = mem {
            garbage.memory.push(mem);
        }
    }
}

impl AshGpuResource for Texture {
    type Raw = (vk::Image, vk::ImageView, Option<GpuMemoryBlock>);
    fn slotmap(res: &AshResources) -> &SlotMap<Self, Self::Raw> {
        &res.textures
    }

    fn slotmap_mut(res: &mut AshResources) -> &mut SlotMap<Self, Self::Raw> {
        &mut res.textures
    }
    unsafe fn create_raw(
        res: &mut AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let alloc = &mut res.alloc;
        let device = alloc.device_arc();
        let img_create_info: vk::ImageCreateInfo<'static> = descr.vk_into();
        let img = device.create(&img_create_info)?;
        let mreq = device.get_image_memory_requirements(img.raw());
        let mem = alloc.alloc(gpu_alloc::Request {
            size: mreq.size,
            align_mask: mreq.alignment,
            usage: gpu_alloc::UsageFlags::FAST_DEVICE_ACCESS,
            memory_types: mreq.memory_type_bits,
        })?;
        device.bind_image_memory(img.raw(), *mem.memory(), mem.offset())?;
        let mut view_create_info: vk::ImageViewCreateInfo<'static> = descr.vk_into();
        view_create_info.image = img.raw();
        let view = device.create(&view_create_info)?;
        Ok((img.take(), view.take(), Some(mem.take())))
    }

    unsafe fn destroy_raw(alloc: &mut AshAllocator, (img, view, mem): Self::Raw) {
        if view != vk::ImageView::null() {
            alloc.device().destroy(view);
        }
        if img != vk::Image::null() {
            alloc.device().destroy(img);
        }
        if let Some(mem) = mem {
            alloc.dealloc(mem);
        }
    }
}
impl AshGpuResourceCreate for Texture {}
impl AshGpuResourceRemove for Texture {
    fn put_to_garbage(garbage: &mut super::AshFrameGarbage, (image, image_view, mem): Self::Raw) {
        if image != vk::Image::null() {
            garbage.images.push(image);
        }
        if image_view != vk::ImageView::null() {
            garbage.image_views.push(image_view);
        }
        if let Some(mem) = mem {
            garbage.memory.push(mem);
        }
    }
}

impl AshGpuResource for GraphicsPass {
    type Raw = vk::RenderPass;
    fn slotmap(res: &AshResources) -> &SlotMap<Self, Self::Raw> {
        &res.graphics_passes
    }
    fn slotmap_mut(res: &mut AshResources) -> &mut SlotMap<Self, Self::Raw> {
        &mut res.graphics_passes
    }
    unsafe fn create_raw(
        res: &mut AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let mut conv = CreateInfoConverter6::new();
        let create_info = conv.graphics_pass(descr);
        let raw = res.device().create(create_info)?;
        Ok(raw.take())
    }
    unsafe fn destroy_raw(alloc: &mut AshAllocator, raw: Self::Raw) {
        if raw != vk::RenderPass::null() {
            alloc.device().destroy(raw);
        }
    }
}
impl AshGpuResourceCached for GraphicsPass {
    #[inline]
    fn get_hashs_mut(res: &mut AshResources) -> &mut U64HashMap<Self> {
        &mut res.graphics_passes_cache
    }
}
impl AshGpuResource for ShaderModule {
    type Raw = vk::ShaderModule;
    fn slotmap(res: &AshResources) -> &SlotMap<Self, Self::Raw> {
        &res.shader_modules
    }
    fn slotmap_mut(res: &mut AshResources) -> &mut SlotMap<Self, Self::Raw> {
        &mut res.shader_modules
    }
    unsafe fn create_raw(
        res: &mut AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let code = compie_into_spv(&descr.source)?;
        let create_info = vk::ShaderModuleCreateInfo::default().code(&code);
        let raw = res.device().create(&create_info)?;
        if let Some(label) = descr.label {
            res.device().object_name(raw.raw(), label);
        }
        Ok(raw.take())
    }
    unsafe fn destroy_raw(alloc: &mut AshAllocator, raw: Self::Raw) {
        if raw != vk::ShaderModule::null() {
            alloc.device().destroy(raw);
        }
    }
}
impl AshGpuResourceCached for ShaderModule {
    #[inline]
    fn get_hashs_mut(res: &mut AshResources) -> &mut U64HashMap<Self> {
        &mut res.shader_modules_cache
    }
}
impl AshGpuResource for BindGroupLayout {
    type Raw = vk::DescriptorSetLayout;
    fn slotmap(res: &AshResources) -> &SlotMap<Self, Self::Raw> {
        &res.bind_group_layouts
    }
    fn slotmap_mut(res: &mut AshResources) -> &mut SlotMap<Self, Self::Raw> {
        &mut res.bind_group_layouts
    }

    unsafe fn create_raw(
        res: &mut AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let mut conv = CreateInfoConverter2::new();
        let create_info = conv.bind_group_layout(descr);
        let raw = res.device().create(create_info)?;
        if let Some(label) = descr.label {
            res.device().object_name(raw.raw(), label);
        }
        Ok(raw.take())
    }
    unsafe fn destroy_raw(alloc: &mut AshAllocator, raw: Self::Raw) {
        if raw != vk::DescriptorSetLayout::null() {
            alloc.device().destroy(raw);
        }
    }
}
impl AshGpuResourceCached for BindGroupLayout {
    #[inline]
    fn get_hashs_mut(res: &mut AshResources) -> &mut U64HashMap<Self> {
        &mut res.bind_group_layouts_cache
    }
}
impl AshGpuResource for PipelineLayout {
    type Raw = vk::PipelineLayout;
    fn slotmap(res: &AshResources) -> &SlotMap<Self, Self::Raw> {
        &res.pipeline_layouts
    }
    fn slotmap_mut(res: &mut AshResources) -> &mut SlotMap<Self, Self::Raw> {
        &mut res.pipeline_layouts
    }

    unsafe fn create_raw(
        res: &mut AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let mut conv = CreateInfoConverter2::new();
        let create_info = conv.pipeline_layout(res, descr);
        let raw = res.device().create(create_info)?;
        if let Some(label) = descr.label {
            res.device().object_name(raw.raw(), label);
        }
        Ok(raw.take())
    }
    unsafe fn destroy_raw(alloc: &mut AshAllocator, raw: Self::Raw) {
        if raw != vk::PipelineLayout::null() {
            alloc.device().destroy(raw);
        }
    }
}
impl AshGpuResourceCached for PipelineLayout {
    #[inline]
    fn get_hashs_mut(res: &mut AshResources) -> &mut U64HashMap<Self> {
        &mut res.pipeline_layouts_cache
    }
}
impl AshGpuResource for GraphicsPipeline {
    type Raw = vk::Pipeline;
    fn slotmap(res: &AshResources) -> &SlotMap<Self, Self::Raw> {
        &res.graphics_pipelines
    }
    fn slotmap_mut(res: &mut AshResources) -> &mut SlotMap<Self, Self::Raw> {
        &mut res.graphics_pipelines
    }

    unsafe fn create_raw(
        res: &mut AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let mut conv = CreateInfoConverter2::new();
        let create_infos = conv.graphics_pipeline_descriptor(res, std::slice::from_ref(descr));
        match res
            .device()
            .create_graphics_pipelines(res.pipeline_cache, create_infos, None)
        {
            Ok(raw) => {
                let raw = res.device().hold(raw[0]);
                if let Some(label) = descr.label {
                    res.device().object_name(raw.raw(), label);
                }
                Ok(raw.take())
            }
            Err((pipelines, e)) => {
                res.device().destroy(pipelines);
                Err(e.into())
            }
        }
    }

    unsafe fn destroy_raw(alloc: &mut AshAllocator, raw: Self::Raw) {
        if raw != vk::Pipeline::null() {
            alloc.device().destroy_pipeline(raw, None);
        }
    }
}
impl AshGpuResourceCached for GraphicsPipeline {
    #[inline]
    fn get_hashs_mut(res: &mut AshResources) -> &mut U64HashMap<Self> {
        &mut res.graphics_pipelines_cache
    }
}
impl AshGpuResource for ComputePipeline {
    type Raw = vk::Pipeline;
    fn slotmap(res: &AshResources) -> &SlotMap<Self, Self::Raw> {
        &res.compute_pipelines
    }
    fn slotmap_mut(res: &mut AshResources) -> &mut SlotMap<Self, Self::Raw> {
        &mut res.compute_pipelines
    }

    unsafe fn create_raw(
        res: &mut AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let mut conv = CreateInfoConverter2::new();
        let create_infos = conv.compute_pipeline_descriptor(res, std::slice::from_ref(descr));
        match res
            .device()
            .create_compute_pipelines(res.pipeline_cache, create_infos, None)
        {
            Ok(raw) => {
                let raw = res.device().hold(raw[0]);
                if let Some(label) = descr.label {
                    res.device().object_name(raw.raw(), label);
                }
                Ok(raw.take())
            }
            Err((pipelines, e)) => {
                res.device().destroy(pipelines);
                Err(e.into())
            }
        }
    }

    unsafe fn destroy_raw(alloc: &mut AshAllocator, raw: Self::Raw) {
        if raw != vk::Pipeline::null() {
            alloc.device().destroy_pipeline(raw, None);
        }
    }
}
impl AshGpuResourceCached for ComputePipeline {
    #[inline]
    fn get_hashs_mut(res: &mut AshResources) -> &mut U64HashMap<Self> {
        &mut res.compute_pipelines_cache
    }
}
impl AshGpuResource for RayTracingPipeline {
    type Raw = vk::Pipeline;

    fn slotmap(res: &AshResources) -> &SlotMap<Self, Self::Raw> {
        &res.ray_tracing_pipelines
    }

    fn slotmap_mut(res: &mut AshResources) -> &mut SlotMap<Self, Self::Raw> {
        &mut res.ray_tracing_pipelines
    }
    unsafe fn create_raw(
        res: &mut AshResources,
        descr: &Self::Descriptor<'_>,
    ) -> Result<Self::Raw> {
        let ext = res.device().ext_raytracing_pipeline()?;
        let mut conv = CreateInfoConverter2::new();
        let create_infos = conv.ray_tracing_pipeline_descriptor(res, std::slice::from_ref(descr));
        let raw = ext.create_ray_tracing_pipelines(
            vk::DeferredOperationKHR::null(),
            res.pipeline_cache,
            create_infos,
            None,
        ).map_err(|(_,e)|e)?;
        let raw = res.device().hold(raw[0]);
        if let Some(label) = descr.label {
            res.device().object_name(raw.raw(), label);
        }
        Ok(raw.take())
    }

    unsafe fn destroy_raw(res: &mut AshAllocator, raw: Self::Raw) {
        if raw != vk::Pipeline::null() {
            res.device().destroy_pipeline(raw, None);
        }
    }
}
impl AshGpuResourceCached for RayTracingPipeline {
    #[inline]
    fn get_hashs_mut(res: &mut AshResources) -> &mut U64HashMap<Self> {
        &mut res.ray_tracing_pipelines_cache
    }
}
