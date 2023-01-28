use std::{ffi::CStr, ops::Deref, os::raw::c_char, sync::Arc};

use ash::{extensions::khr, vk};
use pulz_render::graph::pass::PipelineBindPoint;
use tracing::{debug, info, warn};

use crate::{
    instance::{AshInstance, VK_API_VERSION},
    Error, ErrorNoExtension, Result,
};

pub type GpuAllocator = gpu_alloc::GpuAllocator<vk::DeviceMemory>;
pub type GpuMemoryBlock = gpu_alloc::MemoryBlock<vk::DeviceMemory>;
pub use gpu_alloc::AllocationError;

pub struct AshDevice {
    instance: Arc<AshInstance>,
    physical_device: vk::PhysicalDevice,
    device_raw: ash::Device,
    device_extensions: Vec<&'static CStr>,
    gpu_allocator: GpuAllocator,
    ext_swapchain: Option<khr::Swapchain>,
    ext_sync2: Option<khr::Synchronization2>,
    ext_raytracing_pipeline: Option<khr::RayTracingPipeline>,
    queues: Queues,
}

impl Deref for AshDevice {
    type Target = ash::Device;
    #[inline]
    fn deref(&self) -> &ash::Device {
        &self.device_raw
    }
}

impl AshInstance {
    pub(crate) fn new_device(
        self: &Arc<Self>,
        surface_opt: vk::SurfaceKHR,
    ) -> Result<Arc<AshDevice>> {
        let (physical_device, indices, device_extensions) =
            self.pick_physical_device(surface_opt)?;
        AshDevice::new(self, physical_device, indices, device_extensions)
    }
}

impl AshDevice {
    fn new(
        instance: &Arc<AshInstance>,
        physical_device: vk::PhysicalDevice,
        indices: QueueFamilyIndices,
        device_extensions: Vec<&'static CStr>,
    ) -> Result<Arc<Self>> {
        let gpu_alloc_props =
            unsafe { gpu_alloc_ash::device_properties(instance, VK_API_VERSION, physical_device)? };

        let (device_raw, queues) = instance.create_logical_device(
            physical_device,
            indices,
            device_extensions.iter().copied(),
        )?;

        // TODO: Config
        let gpu_alloc_config = gpu_alloc::Config::i_am_potato();

        let mut device = Self {
            instance: instance.clone(),
            physical_device,
            device_raw,
            device_extensions,
            gpu_allocator: GpuAllocator::new(gpu_alloc_config, gpu_alloc_props),
            ext_swapchain: None,
            ext_sync2: None,
            ext_raytracing_pipeline: None,
            queues,
        };

        if device.has_device_extension(khr::Swapchain::name()) {
            device.ext_swapchain = Some(khr::Swapchain::new(instance, &device));
        }
        if device.has_device_extension(khr::Synchronization2::name()) {
            device.ext_sync2 = Some(khr::Synchronization2::new(instance, &device))
        }
        if device.has_device_extension(khr::RayTracingPipeline::name()) {
            device.ext_raytracing_pipeline = Some(khr::RayTracingPipeline::new(instance, &device))
        }

        Ok(Arc::new(device))
    }

    #[inline]
    pub fn instance(&self) -> &AshInstance {
        &self.instance
    }

    #[inline]
    pub fn physical_device(&self) -> vk::PhysicalDevice {
        self.physical_device
    }

    #[inline]
    pub fn has_device_extension(&self, name: &CStr) -> bool {
        self.device_extensions.contains(&name)
    }

    #[inline]
    pub fn queues(&self) -> &Queues {
        &self.queues
    }

    #[inline]
    pub(crate) fn ext_swapchain(&self) -> Result<&khr::Swapchain, ErrorNoExtension> {
        self.ext_swapchain
            .as_ref()
            .ok_or(ErrorNoExtension(khr::Swapchain::name()))
    }

    #[inline]
    pub(crate) fn ext_sync2(&self) -> Result<&khr::Synchronization2, ErrorNoExtension> {
        self.ext_sync2
            .as_ref()
            .ok_or(ErrorNoExtension(khr::Synchronization2::name()))
    }

    #[inline]
    pub(crate) fn ext_raytracing_pipeline(
        &self,
    ) -> Result<&khr::RayTracingPipeline, ErrorNoExtension> {
        self.ext_raytracing_pipeline
            .as_ref()
            .ok_or(ErrorNoExtension(khr::RayTracingPipeline::name()))
    }

    #[inline]
    pub unsafe fn alloc(
        &mut self,
        request: gpu_alloc::Request,
    ) -> Result<GpuMemoryBlock, AllocationError> {
        self.gpu_allocator.alloc(
            gpu_alloc_ash::AshMemoryDevice::wrap(&self.device_raw),
            request,
        )
    }

    #[inline]
    pub unsafe fn alloc_with_dedicated(
        &mut self,
        request: gpu_alloc::Request,
        dedicated: gpu_alloc::Dedicated,
    ) -> Result<GpuMemoryBlock, AllocationError> {
        self.gpu_allocator.alloc_with_dedicated(
            gpu_alloc_ash::AshMemoryDevice::wrap(&self.device_raw),
            request,
            dedicated,
        )
    }

    #[inline]
    pub unsafe fn dealloc(&mut self, block: GpuMemoryBlock) {
        self.gpu_allocator.dealloc(
            gpu_alloc_ash::AshMemoryDevice::wrap(&self.device_raw),
            block,
        )
    }

    #[inline]
    pub unsafe fn object_name<H: vk::Handle>(&self, handle: H, name: &str) {
        if let Ok(debug_utils) = self.instance.ext_debug_utils() {
            debug_utils.object_name(self.handle(), handle, name)
        }
    }
}

impl Drop for AshDevice {
    fn drop(&mut self) {
        if self.device_raw.handle() != vk::Device::null() {
            unsafe {
                self.device_raw.destroy_device(None);
            }
        }
    }
}

fn get_device_extensions(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
) -> Result<Vec<&'static CStr>> {
    let available_extensions =
        unsafe { instance.enumerate_device_extension_properties(physical_device)? };

    let mut extensions = Vec::with_capacity(4);
    extensions.push(khr::Swapchain::name());
    extensions.push(khr::Synchronization2::name());

    // Only keep available extensions.
    extensions.retain(|&ext| {
        if available_extensions
            .iter()
            .any(|avail_ext| unsafe { CStr::from_ptr(avail_ext.extension_name.as_ptr()) == ext })
        {
            debug!("Device extension ✅ YES {:?}", ext);
            true
        } else {
            warn!("Device extension ❌ NO  {:?}", ext);
            false
        }
    });

    Ok(extensions)
}

impl AshInstance {
    fn pick_physical_device(
        &self,
        for_surface_opt: vk::SurfaceKHR,
    ) -> Result<(vk::PhysicalDevice, QueueFamilyIndices, Vec<&'static CStr>)> {
        let physical_devices = unsafe { self.enumerate_physical_devices()? };

        info!(
            "{} devices (GPU) found with vulkan support.",
            physical_devices.len()
        );

        let mut result = None;
        for (i, &physical_device) in physical_devices.iter().enumerate() {
            self.log_device_infos(physical_device, i);

            if let Some((indices, extensions)) =
                self.check_physical_device_suitable(physical_device, for_surface_opt)
            {
                if result.is_none() {
                    result = Some((i, physical_device, indices, extensions))
                }
            }
        }

        match result {
            Some((i, physical_device, indices, extensions)) => {
                info!("Selected device: #{}", i);
                Ok((physical_device, indices, extensions))
            }
            None => {
                warn!("Unable to find a suitable GPU!");
                Err(Error::NoAdapter)
            }
        }
    }

    fn log_device_infos(&self, physical_device: vk::PhysicalDevice, device_index: usize) {
        let device_properties = unsafe { self.get_physical_device_properties(physical_device) };
        let _device_features = unsafe { self.get_physical_device_features(physical_device) };
        let device_queue_families =
            unsafe { self.get_physical_device_queue_family_properties(physical_device) };

        let device_name = unsafe { CStr::from_ptr(device_properties.device_name.as_ptr()) };

        info!(
            "Device #{}\tName: {:?}, id: {:?}, type: {:?}",
            device_index, device_name, device_properties.device_id, device_properties.device_type
        );

        info!("\tQueue Families: {}", device_queue_families.len());
        for (i, queue_family) in device_queue_families.iter().enumerate() {
            info!(
                "\t  #{}:{:4} x {:?}",
                i, queue_family.queue_count, queue_family.queue_flags
            );
        }
    }

    fn check_physical_device_suitable(
        &self,
        physical_device: vk::PhysicalDevice,
        for_surface_opt: vk::SurfaceKHR,
    ) -> Option<(QueueFamilyIndices, Vec<&'static CStr>)> {
        let indices =
            QueueFamilyIndices::from_physical_device(self, physical_device, for_surface_opt)?;
        let extensions = get_device_extensions(self, physical_device).ok()?;

        if for_surface_opt != vk::SurfaceKHR::null()
            && (!extensions.contains(&khr::Swapchain::name())
                || self
                    .query_swapchain_support(for_surface_opt, physical_device)
                    .is_none())
        {
            return None;
        }

        Some((indices, extensions))
    }

    #[inline]
    fn create_logical_device<'a>(
        &self,
        physical_device: vk::PhysicalDevice,
        indices: QueueFamilyIndices,
        extensions: impl IntoIterator<Item = &'a CStr>,
    ) -> Result<(ash::Device, Queues)> {
        let extensions_ptr: Vec<_> = extensions.into_iter().map(CStr::as_ptr).collect();
        self._create_logical_device(physical_device, indices, &extensions_ptr)
    }

    fn _create_logical_device(
        &self,
        physical_device: vk::PhysicalDevice,
        indices: QueueFamilyIndices,
        extensions_ptr: &[*const c_char],
    ) -> Result<(ash::Device, Queues)> {
        let device = unsafe {
            self.create_device(
                physical_device,
                &vk::DeviceCreateInfo::builder()
                    .queue_create_infos(&[vk::DeviceQueueCreateInfo::builder()
                        .queue_family_index(indices.graphics_family)
                        .queue_priorities(&[1.0_f32])
                        .build()])
                    .enabled_extension_names(extensions_ptr),
                // .enabled_features(&vk::PhysicalDeviceFeatures {
                //     ..Default::default() // default just enable no feature.
                // })
                None,
            )?
        };

        let queues = Queues::from_device(&device, indices);

        Ok((device, queues))
    }
}

pub struct QueueFamilyIndices {
    pub graphics_family: u32,
    pub compute_family: u32,
    pub present_family: u32,
}

impl QueueFamilyIndices {
    fn from_physical_device(
        instance: &AshInstance,
        physical_device: vk::PhysicalDevice,
        for_surface_opt: vk::SurfaceKHR,
    ) -> Option<Self> {
        let queue_families =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

        #[derive(Default)]
        struct OptIndices {
            graphics: Option<u32>,
            compute: Option<u32>,
            present: Option<u32>,
        }

        impl OptIndices {
            fn check_complete(&self) -> Option<QueueFamilyIndices> {
                Some(QueueFamilyIndices {
                    graphics_family: self.graphics?,
                    compute_family: self.compute?,
                    present_family: self.present?,
                })
            }
        }

        let mut indices = OptIndices::default();
        for (i, queue_family) in queue_families.iter().enumerate() {
            let i = i as u32;
            if queue_family.queue_count == 0 {
                continue;
            }
            if indices.graphics.is_none()
                && queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
            {
                indices.graphics = Some(i);
                if for_surface_opt == vk::SurfaceKHR::null() {
                    indices.present = Some(i);
                }
            }

            if indices.compute.is_none()
                && queue_family.queue_flags.contains(vk::QueueFlags::COMPUTE)
            {
                indices.compute = Some(i);
            }

            if indices.present.is_none()
                && for_surface_opt != vk::SurfaceKHR::null()
                && instance.get_physical_device_surface_support(physical_device, i, for_surface_opt)
            {
                indices.present = Some(i);
            }

            if let Some(result) = indices.check_complete() {
                return Some(result);
            }
        }

        None
    }
}

pub struct Queues {
    pub indices: QueueFamilyIndices,
    pub graphics: vk::Queue,
    pub compute: vk::Queue,
    pub present: vk::Queue,
}

impl Queues {
    pub fn from_device(device: &ash::Device, indices: QueueFamilyIndices) -> Self {
        unsafe {
            let graphics = device.get_device_queue(indices.graphics_family, 0);
            let compute = device.get_device_queue(indices.compute_family, 0);
            let present = device.get_device_queue(indices.present_family, 0);
            Self {
                indices,
                graphics,
                compute,
                present,
            }
        }
    }

    pub fn for_bind_point(&self, bind_point: PipelineBindPoint) -> (vk::Queue, u32) {
        match bind_point {
            PipelineBindPoint::Graphics | PipelineBindPoint::RayTracing => {
                (self.graphics, self.indices.graphics_family)
            }
            PipelineBindPoint::Compute => (self.compute, self.indices.compute_family),
        }
    }
}

impl Deref for Queues {
    type Target = QueueFamilyIndices;
    #[inline]
    fn deref(&self) -> &QueueFamilyIndices {
        &self.indices
    }
}
