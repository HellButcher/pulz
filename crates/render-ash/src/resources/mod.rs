use std::{collections::HashMap, hash::Hasher, ops::Index, sync::Arc};

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

use crate::{device::AshDevice, Result};

mod replay;
mod resource_impl;
mod traits;

use self::{
    replay::RecordResource,
    traits::{AshGpuResourceCreate, AshGpuResourceRemove},
};

#[derive(Default)]
struct PreHashedHasherHasher(u64);
type PreHashedHasherBuildHasher = std::hash::BuildHasherDefault<PreHashedHasherHasher>;

impl Hasher for PreHashedHasherHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut hash = self.0;
        for byte in bytes.iter() {
            hash <<= 8;
            hash |= *byte as u64;
        }
        self.0 = hash;
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.0 = i;
    }

    #[inline]
    fn write_i64(&mut self, i: i64) {
        self.0 = i as u64;
    }
}

type PreHashedU64Map<V> = HashMap<u64, V, PreHashedHasherBuildHasher>;

pub struct AshResources {
    device: Arc<AshDevice>,
    record: Option<Box<dyn RecordResource>>,
    pipeline_cache: vk::PipelineCache,
    graphics_passes_cache: PreHashedU64Map<GraphicsPass>,
    shader_modules_cache: PreHashedU64Map<ShaderModule>,
    bind_group_layouts_cache: PreHashedU64Map<BindGroupLayout>,
    pipeline_layouts_cache: PreHashedU64Map<PipelineLayout>,
    graphics_pipelines_cache: PreHashedU64Map<GraphicsPipeline>,
    compute_pipelines_cache: PreHashedU64Map<ComputePipeline>,
    ray_tracing_pipelines_cache: PreHashedU64Map<RayTracingPipeline>,
    pub graphics_passes: SlotMap<GraphicsPass, vk::RenderPass>,
    pub shader_modules: SlotMap<ShaderModule, vk::ShaderModule>,
    pub bind_group_layouts: SlotMap<BindGroupLayout, vk::DescriptorSetLayout>,
    pub pipeline_layouts: SlotMap<PipelineLayout, vk::PipelineLayout>,
    pub graphics_pipelines: SlotMap<GraphicsPipeline, vk::Pipeline>,
    pub compute_pipelines: SlotMap<ComputePipeline, vk::Pipeline>,
    pub ray_tracing_pipelines: SlotMap<RayTracingPipeline, vk::Pipeline>,
    pub buffers: SlotMap<Buffer, vk::Buffer>,
    pub textures: SlotMap<Texture, (vk::Image, vk::ImageView)>,
}

impl AshResources {
    pub fn new(device: &Arc<AshDevice>) -> Self {
        Self {
            device: device.clone(),
            record: None,
            graphics_passes_cache: HashMap::default(),
            shader_modules_cache: HashMap::default(),
            bind_group_layouts_cache: HashMap::default(),
            pipeline_layouts_cache: HashMap::default(),
            graphics_pipelines_cache: HashMap::default(),
            compute_pipelines_cache: HashMap::default(),
            ray_tracing_pipelines_cache: HashMap::default(),
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
        }
    }

    #[inline]
    pub fn device(&self) -> &AshDevice {
        &self.device
    }

    pub fn with_pipeline_cache(mut self, initial_data: &[u8]) -> Result<Self> {
        self.set_pipeline_cache(initial_data)?;
        Ok(self)
    }

    pub fn set_pipeline_cache(&mut self, initial_data: &[u8]) -> Result<()> {
        unsafe {
            if self.pipeline_cache != vk::PipelineCache::null() {
                self.device
                    .destroy_pipeline_cache(self.pipeline_cache, None);
                self.pipeline_cache = vk::PipelineCache::null();
            }
            self.pipeline_cache = self.device.create_pipeline_cache(
                &vk::PipelineCacheCreateInfo::builder()
                    .initial_data(initial_data)
                    .build(),
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
            let data = self.device.get_pipeline_cache_data(self.pipeline_cache)?;
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
    pub fn get_raw<R>(&self, key: R) -> Option<R::Raw>
    where
        R: AshGpuResourceCreate,
    {
        R::get_raw(self, key).copied()
    }

    #[inline]
    pub fn clear<R>(&mut self)
    where
        R: AshGpuResourceCreate,
    {
        R::clear(self)
    }

    #[inline]
    pub fn remove<R>(&mut self, key: R) -> bool
    where
        R: AshGpuResourceRemove,
    {
        R::remove(self, key)
    }

    pub fn clear_all(&mut self) {
        self.clear::<RayTracingPipeline>();
        self.clear::<ComputePipeline>();
        self.clear::<GraphicsPipeline>();
        self.clear::<BindGroupLayout>();
        self.clear::<ShaderModule>();
        self.clear::<GraphicsPass>();
        self.clear::<Texture>();
        self.clear::<Buffer>();
    }
}

impl<R: AshGpuResourceCreate> Index<R> for AshResources {
    type Output = R::Raw;
    #[inline]
    fn index(&self, index: R) -> &Self::Output {
        R::get_raw(self, index).expect("invalid resource")
    }
}

impl Drop for AshResources {
    #[inline]
    fn drop(&mut self) {
        self.clear_all();
        if self.pipeline_cache != vk::PipelineCache::null() {
            unsafe {
                self.device
                    .destroy_pipeline_cache(self.pipeline_cache, None);
            }
        }
    }
}
