use std::{ffi::CStr, ops::Deref, os::raw::c_char, sync::Arc};

use ash::vk;
use pulz_render::graph::pass::PipelineBindPoint;
use tracing::{debug, info, warn};

use crate::{
    debug_utils, instance::AshInstance, AshRendererFlags, Error, ErrorNoExtension, Result,
};

pub struct AshDevice {
    device_raw: ash::Device,
    instance: Arc<AshInstance>,
    physical_device: vk::PhysicalDevice,
    device_extensions: Vec<&'static CStr>,
    debug_utils: Option<debug_utils::DeviceDebugUtils>,
    ext_swapchain: Option<ash::khr::swapchain::Device>,
    ext_sync2: Option<ash::khr::synchronization2::Device>,
    ext_raytracing_pipeline: Option<ash::khr::ray_tracing_pipeline::Device>,
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
        let (device_raw, queues) = instance.create_logical_device(
            physical_device,
            indices,
            device_extensions.iter().copied(),
        )?;

        let mut device = Self {
            instance: instance.clone(),
            physical_device,
            device_raw,
            device_extensions,
            debug_utils: None,
            ext_swapchain: None,
            ext_sync2: None,
            ext_raytracing_pipeline: None,
            queues,
        };

        if instance.debug_utils().is_ok() {
            device.debug_utils = Some(debug_utils::DeviceDebugUtils::new(instance, &device));
        }
        if device.has_device_extension(ash::khr::swapchain::NAME) {
            device.ext_swapchain = Some(ash::khr::swapchain::Device::new(instance, &device));
        }
        if device.has_device_extension(ash::khr::synchronization2::NAME) {
            device.ext_sync2 = Some(ash::khr::synchronization2::Device::new(instance, &device))
        }
        if device.has_device_extension(ash::khr::ray_tracing_pipeline::NAME) {
            device.ext_raytracing_pipeline = Some(ash::khr::ray_tracing_pipeline::Device::new(
                instance, &device,
            ))
        }

        Ok(Arc::new(device))
    }

    #[inline]
    pub fn instance(&self) -> &AshInstance {
        &self.instance
    }

    #[inline]
    pub fn instance_arc(&self) -> Arc<AshInstance> {
        self.instance.clone()
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
    pub(crate) fn debug_utils(&self) -> Result<&debug_utils::DeviceDebugUtils, ErrorNoExtension> {
        self.debug_utils
            .as_ref()
            .ok_or(ErrorNoExtension(ash::ext::debug_utils::NAME))
    }

    #[inline]
    pub(crate) fn ext_swapchain(&self) -> Result<&ash::khr::swapchain::Device, ErrorNoExtension> {
        self.ext_swapchain
            .as_ref()
            .ok_or(ErrorNoExtension(ash::khr::swapchain::NAME))
    }

    #[inline]
    pub(crate) fn ext_sync2(
        &self,
    ) -> Result<&ash::khr::synchronization2::Device, ErrorNoExtension> {
        self.ext_sync2
            .as_ref()
            .ok_or(ErrorNoExtension(ash::khr::synchronization2::NAME))
    }

    #[inline]
    pub(crate) fn ext_raytracing_pipeline(
        &self,
    ) -> Result<&ash::khr::ray_tracing_pipeline::Device, ErrorNoExtension> {
        self.ext_raytracing_pipeline
            .as_ref()
            .ok_or(ErrorNoExtension(ash::khr::ray_tracing_pipeline::NAME))
    }

    #[inline]
    pub unsafe fn object_name<H: vk::Handle>(&self, handle: H, name: &str) {
        if let Some(debug_utils) = &self.debug_utils {
            debug_utils.object_name(handle, name)
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
    extensions.push(ash::khr::swapchain::NAME);
    extensions.push(ash::khr::synchronization2::NAME);

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
            && (!extensions.contains(&ash::khr::swapchain::NAME)
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
                &vk::DeviceCreateInfo::default()
                    .queue_create_infos(&[vk::DeviceQueueCreateInfo::default()
                        .queue_family_index(indices.graphics_family)
                        .queue_priorities(&[1.0_f32])])
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
