use std::{ops::Index, sync::Arc};

use ash::vk;
use pulz_render::{
    buffer::Buffer,
    pipeline::{
        BindGroupLayout, ComputePipeline, GraphicsPass, GraphicsPipeline, PipelineLayout,
        RayTracingPipeline,
    },
    shader::ShaderModule,
    texture::Texture,
    utils::hash::U64HashMap,
};
use slotmap::SlotMap;

use crate::{
    alloc::{AshAllocator, GpuMemoryBlock},
    device::AshDevice,
    instance::AshInstance,
    Result,
};

mod replay;
mod resource_impl;
mod traits;

use self::{
    replay::RecordResource,
    traits::{
        AshGpuResource, AshGpuResourceCollection, AshGpuResourceCreate, AshGpuResourceRemove,
    },
};
pub struct AshResources {
    pub alloc: AshAllocator,
    record: Option<Box<dyn RecordResource>>,
    pipeline_cache: vk::PipelineCache,
    graphics_passes_cache: U64HashMap<GraphicsPass>,
    shader_modules_cache: U64HashMap<ShaderModule>,
    bind_group_layouts_cache: U64HashMap<BindGroupLayout>,
    pipeline_layouts_cache: U64HashMap<PipelineLayout>,
    graphics_pipelines_cache: U64HashMap<GraphicsPipeline>,
    compute_pipelines_cache: U64HashMap<ComputePipeline>,
    ray_tracing_pipelines_cache: U64HashMap<RayTracingPipeline>,
    pub graphics_passes: SlotMap<GraphicsPass, vk::RenderPass>,
    pub shader_modules: SlotMap<ShaderModule, vk::ShaderModule>,
    pub bind_group_layouts: SlotMap<BindGroupLayout, vk::DescriptorSetLayout>,
    pub pipeline_layouts: SlotMap<PipelineLayout, vk::PipelineLayout>,
    pub graphics_pipelines: SlotMap<GraphicsPipeline, vk::Pipeline>,
    pub compute_pipelines: SlotMap<ComputePipeline, vk::Pipeline>,
    pub ray_tracing_pipelines: SlotMap<RayTracingPipeline, vk::Pipeline>,
    pub buffers: SlotMap<Buffer, (vk::Buffer, Option<GpuMemoryBlock>)>,
    pub textures: SlotMap<Texture, (vk::Image, vk::ImageView, Option<GpuMemoryBlock>)>,
    frame_garbage: Vec<AshFrameGarbage>,
    current_frame: usize,
}

#[derive(Debug, Default)]
pub struct AshFrameGarbage {
    pub texture_handles: Vec<Texture>,
    pub buffer_handles: Vec<Buffer>,
    pub buffers: Vec<vk::Buffer>,
    pub images: Vec<vk::Image>,
    pub image_views: Vec<vk::ImageView>,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub swapchains: Vec<vk::SwapchainKHR>,
    pub memory: Vec<GpuMemoryBlock>,
}

impl AshResources {
    pub fn new(device: &Arc<AshDevice>, num_frames_in_flight: usize) -> Result<Self> {
        let alloc = AshAllocator::new(device)?;
        let mut frame_garbage = Vec::with_capacity(num_frames_in_flight);
        frame_garbage.resize_with(num_frames_in_flight, AshFrameGarbage::default);
        Ok(Self {
            alloc,
            record: None,
            graphics_passes_cache: U64HashMap::default(),
            shader_modules_cache: U64HashMap::default(),
            bind_group_layouts_cache: U64HashMap::default(),
            pipeline_layouts_cache: U64HashMap::default(),
            graphics_pipelines_cache: U64HashMap::default(),
            compute_pipelines_cache: U64HashMap::default(),
            ray_tracing_pipelines_cache: U64HashMap::default(),
            pipeline_cache: vk::PipelineCache::null(),
            graphics_passes: SlotMap::with_key(),
            shader_modules: SlotMap::with_key(),
            bind_group_layouts: SlotMap::with_key(),
            pipeline_layouts: SlotMap::with_key(),
            graphics_pipelines: SlotMap::with_key(),
            compute_pipelines: SlotMap::with_key(),
            ray_tracing_pipelines: SlotMap::with_key(),
            buffers: SlotMap::with_key(),
            textures: SlotMap::with_key(),
            frame_garbage,
            current_frame: 0,
        })
    }

    #[inline]
    pub fn instance(&self) -> &AshInstance {
        self.alloc.instance()
    }

    #[inline]
    pub fn device(&self) -> &AshDevice {
        self.alloc.device()
    }

    pub fn with_pipeline_cache(mut self, initial_data: &[u8]) -> Result<Self> {
        self.set_pipeline_cache(initial_data)?;
        Ok(self)
    }

    pub fn set_pipeline_cache(&mut self, initial_data: &[u8]) -> Result<()> {
        unsafe {
            if self.pipeline_cache != vk::PipelineCache::null() {
                self.alloc
                    .device()
                    .destroy_pipeline_cache(self.pipeline_cache, None);
                self.pipeline_cache = vk::PipelineCache::null();
            }
            self.pipeline_cache = self.alloc.device().create_pipeline_cache(
                &vk::PipelineCacheCreateInfo::default()
                    .initial_data(initial_data),
                None,
            )?;
        }
        Ok(())
    }

    pub fn get_pipeline_cache_data(&self) -> Result<Vec<u8>> {
        if self.pipeline_cache == vk::PipelineCache::null() {
            return Ok(Vec::new());
        }
        unsafe {
            let data = self
                .alloc
                .device()
                .get_pipeline_cache_data(self.pipeline_cache)?;
            Ok(data)
        }
    }

    #[inline]
    pub fn create<R>(&mut self, descriptor: &R::Descriptor<'_>) -> Result<R>
    where
        R: AshGpuResourceCreate,
    {
        R::create(self, descriptor)
    }

    #[inline]
    pub fn get_raw<R>(&self, key: R) -> Option<&R::Raw>
    where
        R: AshGpuResourceCreate,
    {
        R::slotmap(self).get(key)
    }

    #[inline]
    pub fn destroy<R>(&mut self, key: R) -> bool
    where
        R: AshGpuResourceRemove,
    {
        R::destroy(self, key)
    }

    pub(crate) fn wait_idle_and_clear_garbage(&mut self) -> Result<()> {
        unsafe {
            self.alloc.device().device_wait_idle()?;
            for frame in 0..self.frame_garbage.len() {
                self.clear_garbage(frame);
            }
        }
        Ok(())
    }

    pub(crate) fn wait_idle_and_clear_all(&mut self) -> Result<()> {
        self.wait_idle_and_clear_garbage()?;
        // SAFETY: clear save, because clear garbage waits until device is idle
        unsafe {
            self.ray_tracing_pipelines.clear_destroy(&mut self.alloc);
            self.ray_tracing_pipelines_cache.clear();
            self.compute_pipelines.clear_destroy(&mut self.alloc);
            self.compute_pipelines_cache.clear();
            self.graphics_pipelines.clear_destroy(&mut self.alloc);
            self.graphics_pipelines_cache.clear();
            self.pipeline_layouts.clear_destroy(&mut self.alloc);
            self.pipeline_layouts_cache.clear();
            self.bind_group_layouts.clear_destroy(&mut self.alloc);
            self.bind_group_layouts_cache.clear();
            self.shader_modules.clear_destroy(&mut self.alloc);
            self.shader_modules_cache.clear();
            self.graphics_passes.clear_destroy(&mut self.alloc);
            self.graphics_passes_cache.clear();
            self.textures.clear_destroy(&mut self.alloc);
            self.buffers.clear_destroy(&mut self.alloc);
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn current_frame_garbage_mut(&mut self) -> &mut AshFrameGarbage {
        &mut self.frame_garbage[self.current_frame]
    }

    pub(crate) unsafe fn clear_garbage(&mut self, frame: usize) {
        let garbage = &mut self.frame_garbage[frame];
        let mut textures = std::mem::take(&mut garbage.texture_handles);
        for texture in textures.drain(..) {
            if let Some(raw) = self.textures.remove(texture) {
                Texture::put_to_garbage(garbage, raw)
            }
        }
        garbage.texture_handles = textures;
        let mut buffers = std::mem::take(&mut garbage.buffer_handles);
        for buffer in buffers.drain(..) {
            if let Some(raw) = self.buffers.remove(buffer) {
                Buffer::put_to_garbage(garbage, raw)
            }
        }
        garbage.buffer_handles = buffers;
        garbage.clear_frame(&mut self.alloc);
    }

    /// # SAFETY
    /// caller must ensure, that the next frame has finished
    pub(crate) unsafe fn next_frame_and_clear_garbage(&mut self) {
        self.current_frame = (self.current_frame + 1) % self.frame_garbage.len();
        self.clear_garbage(self.current_frame);
    }
}

impl AshFrameGarbage {
    unsafe fn clear_frame(&mut self, alloc: &mut AshAllocator) {
        let device = alloc.device();
        self.framebuffers
            .drain(..)
            .for_each(|r| device.destroy_framebuffer(r, None));
        self.image_views
            .drain(..)
            .for_each(|r| device.destroy_image_view(r, None));
        self.images
            .drain(..)
            .for_each(|r| device.destroy_image(r, None));
        self.buffers
            .drain(..)
            .for_each(|r| device.destroy_buffer(r, None));
        if let Ok(ext_swapchain) = device.ext_swapchain() {
            self.swapchains
                .drain(..)
                .for_each(|r| ext_swapchain.destroy_swapchain(r, None));
        }
        self.memory.drain(..).for_each(|r| alloc.dealloc(r));
    }
}

impl<R: AshGpuResource> Index<R> for AshResources {
    type Output = R::Raw;
    #[inline]
    fn index(&self, index: R) -> &Self::Output {
        R::slotmap(self).get(index).expect("invalid resource")
    }
}

impl Drop for AshResources {
    #[inline]
    fn drop(&mut self) {
        self.wait_idle_and_clear_all().unwrap();
        if self.pipeline_cache != vk::PipelineCache::null() {
            unsafe {
                self.alloc
                    .device()
                    .destroy_pipeline_cache(self.pipeline_cache, None);
            }
        }
    }
}
