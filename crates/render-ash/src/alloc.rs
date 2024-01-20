use std::{mem::ManuallyDrop, sync::Arc};

use ash::vk;

use crate::{
    device::AshDevice,
    instance::{AshInstance, VK_API_VERSION},
    Result,
};

type GpuAllocator = gpu_alloc::GpuAllocator<vk::DeviceMemory>;
pub type GpuMemoryBlock = gpu_alloc::MemoryBlock<vk::DeviceMemory>;
pub use gpu_alloc::AllocationError;

pub struct AshAllocator {
    device: Arc<AshDevice>,
    gpu_allocator: GpuAllocator,
}

impl AshAllocator {
    pub fn new(device: &Arc<AshDevice>) -> Result<Self> {
        let gpu_alloc_props = unsafe {
            gpu_alloc_ash::device_properties(
                device.instance(),
                VK_API_VERSION,
                device.physical_device(),
            )?
        };

        // TODO: Config
        let gpu_alloc_config = gpu_alloc::Config::i_am_potato();

        Ok(Self {
            device: device.clone(),
            gpu_allocator: GpuAllocator::new(gpu_alloc_config, gpu_alloc_props),
        })
    }

    #[inline]
    pub fn instance(&self) -> &AshInstance {
        self.device.instance()
    }

    #[inline]
    pub fn instance_arc(&self) -> Arc<AshInstance> {
        self.device.instance_arc()
    }

    #[inline]
    pub fn device(&self) -> &AshDevice {
        &self.device
    }

    #[inline]
    pub fn device_arc(&self) -> Arc<AshDevice> {
        self.device.clone()
    }

    #[inline]
    pub unsafe fn alloc(
        &mut self,
        request: gpu_alloc::Request,
    ) -> Result<AshMemoryBlockGuard<'_>, AllocationError> {
        let block = self
            .gpu_allocator
            .alloc(gpu_alloc_ash::AshMemoryDevice::wrap(&self.device), request)?;
        Ok(AshMemoryBlockGuard {
            allocator: self,
            block: ManuallyDrop::new(block),
        })
    }

    #[inline]
    pub unsafe fn alloc_with_dedicated(
        &mut self,
        request: gpu_alloc::Request,
        dedicated: gpu_alloc::Dedicated,
    ) -> Result<AshMemoryBlockGuard<'_>, AllocationError> {
        let block = self.gpu_allocator.alloc_with_dedicated(
            gpu_alloc_ash::AshMemoryDevice::wrap(&self.device),
            request,
            dedicated,
        )?;
        Ok(AshMemoryBlockGuard {
            allocator: self,
            block: ManuallyDrop::new(block),
        })
    }

    #[inline]
    pub unsafe fn dealloc(&mut self, block: GpuMemoryBlock) {
        self.gpu_allocator
            .dealloc(gpu_alloc_ash::AshMemoryDevice::wrap(&self.device), block)
    }
}

pub struct AshMemoryBlockGuard<'a> {
    block: ManuallyDrop<GpuMemoryBlock>,
    allocator: &'a mut AshAllocator,
}

impl AshMemoryBlockGuard<'_> {
    #[inline]
    pub fn take(mut self) -> GpuMemoryBlock {
        let block = unsafe { ManuallyDrop::take(&mut self.block) };
        std::mem::forget(self);
        block
    }
}
impl Drop for AshMemoryBlockGuard<'_> {
    fn drop(&mut self) {
        unsafe {
            self.allocator.dealloc(ManuallyDrop::take(&mut self.block));
        }
    }
}

impl std::ops::Deref for AshMemoryBlockGuard<'_> {
    type Target = GpuMemoryBlock;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.block
    }
}
impl std::ops::DerefMut for AshMemoryBlockGuard<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.block
    }
}
