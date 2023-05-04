use std::sync::Arc;

use ash::{extensions::khr, vk};
use pulz_render::{
    math::uvec2,
    texture::{Texture, TextureFormat},
};
use pulz_window::{RawWindow, Size2, Window, WindowId};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use slotmap::Key;
use tracing::debug;

use crate::{
    convert::VkInto,
    device::AshDevice,
    drop_guard::{Destroy, Guard},
    instance::AshInstance,
    AshRendererFull, Error, Result,
};

pub struct Surface {
    instance: Arc<AshInstance>,
    surface_raw: vk::SurfaceKHR,
}

impl Drop for Surface {
    fn drop(&mut self) {
        if self.surface_raw != vk::SurfaceKHR::null() {
            let ext_surface = self.instance.ext_surface().unwrap();
            unsafe {
                ext_surface.destroy_surface(self.surface_raw, None);
            }
        }
    }
}

impl Destroy for vk::SurfaceKHR {
    type Context = AshInstance;
    #[inline]
    unsafe fn destroy(self, instance: &AshInstance) {
        let ext_surface = instance.ext_surface().unwrap();
        ext_surface.destroy_surface(self, None);
    }
}

macro_rules! check_and_get_extension {
    ($self:ident => $ext:ty) => {{
        if !$self.has_instance_extension(<$ext>::name()) {
            return Err(Error::ExtensionNotSupported(<$ext>::name()));
        }
        <$ext>::new($self.entry(), $self)
    }};
}

impl AshInstance {
    #[cfg(all(
        unix,
        not(target_os = "android"),
        not(target_os = "macos"),
        not(target_os = "ios")
    ))]
    unsafe fn create_surface_xlib(
        &self,
        dpy: *mut vk::Display,
        window: vk::Window,
    ) -> Result<vk::SurfaceKHR> {
        let functions = check_and_get_extension!(self => khr::XlibSurface);
        let surface = functions.create_xlib_surface(
            &vk::XlibSurfaceCreateInfoKHR::builder()
                .dpy(dpy)
                .window(window),
            None,
        )?;
        Ok(surface)
    }

    #[cfg(all(
        unix,
        not(target_os = "android"),
        not(target_os = "macos"),
        not(target_os = "ios")
    ))]
    unsafe fn create_surface_xcb(
        &self,
        connection: *mut vk::xcb_connection_t,
        window: vk::xcb_window_t,
    ) -> Result<vk::SurfaceKHR> {
        let functions = check_and_get_extension!(self => khr::XcbSurface);
        let surface = functions.create_xcb_surface(
            &vk::XcbSurfaceCreateInfoKHR::builder()
                .connection(connection)
                .window(window),
            None,
        )?;
        Ok(surface)
    }

    #[cfg(all(
        unix,
        not(target_os = "android"),
        not(target_os = "macos"),
        not(target_os = "ios")
    ))]
    unsafe fn create_surface_wayland(
        &self,
        display: *mut vk::wl_display,
        surface: *mut vk::wl_surface,
    ) -> Result<vk::SurfaceKHR> {
        let functions = check_and_get_extension!(self => khr::WaylandSurface);
        let surface = functions.create_wayland_surface(
            &vk::WaylandSurfaceCreateInfoKHR::builder()
                .display(display)
                .surface(surface),
            None,
        )?;
        Ok(surface)
    }

    #[cfg(target_os = "android")]
    unsafe fn create_surface_android(
        &self,
        window: *mut vk::ANativeWindow,
    ) -> Result<vk::SurfaceKHR> {
        let functions = check_and_get_extension!(self => khr::AndroidSurface);
        let surface = functions.create_android_surface(
            &vk::AndroidSurfaceCreateInfoKHR::builder()
                .window(window)
                .build(),
            None,
        )?;
        Ok(surface)
    }

    #[cfg(target_os = "windows")]
    unsafe fn create_surface_win32(
        &self,
        hinstance: vk::HINSTANCE,
        hwnd: vk::HWND,
    ) -> Result<vk::SurfaceKHR> {
        let functions = check_and_get_extension!(self => khr::Win32Surface);
        let surface = functions.create_win32_surface(
            &vk::Win32SurfaceCreateInfoKHR::builder()
                .hinstance(hinstance)
                .hwnd(hwnd)
                .build(),
            None,
        )?;
        Ok(surface)
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    unsafe fn create_surface_metal(
        &self,
        layer: *const vk::CAMetalLayer,
    ) -> Result<vk::SurfaceKHR> {
        use ash::extensions::ext;
        let functions = check_and_get_extension!(self => ext::MetalSurface);
        let surface = functions.create_metal_surface(
            &vk::MetalSurfaceCreateInfoEXT::builder()
                .layer(layer)
                .build(),
            None,
        )?;
        Ok(surface)
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    unsafe fn create_surface_apple(&self, view: *mut c_void) -> Result<vk::SurfaceKHR> {
        use ash::extensions::ext;
        use core_graphics_types::{base::CGFloat, geometry::CGRect};
        use objc::{
            class, msg_send,
            runtime::{Object, BOOL, YES},
            sel, sel_impl,
        };

        // early check extension
        if !self.has_instance_extension(ext::MetalSurface::name()) {
            return Err(Error::ExtensionNotSupported(ext::MetalSurface::name()));
        }

        let layer = unsafe {
            let view = view as *mut Object;
            let existing: *mut Object = msg_send![view, layer];
            let layer_class = class!(CAMetalLayer);

            if !existing.is_null() && msg_send![existing, isKindOfClass: layer_class] == YES {
                existing
            } else {
                let layer: *mut Object = msg_send![layer_class, new];
                let _: () = msg_send![view, setLayer: layer];
                let bounds: CGRect = msg_send![view, bounds];
                let () = msg_send![layer, setBounds: bounds];

                let window: *mut Object = msg_send![view, window];
                if !window.is_null() {
                    let scale_factor: CGFloat = msg_send![window, backingScaleFactor];
                    let () = msg_send![layer, setContentsScale: scale_factor];
                }
                layer
            }
        };

        self.create_surface_metal(layer as *mut _)
    }

    /// SAFETY: display and window handles must be valid for the complete lifetime of surface
    unsafe fn create_surface_raw(
        &self,
        display_handle: RawDisplayHandle,
        window_handle: RawWindowHandle,
    ) -> Result<vk::SurfaceKHR> {
        // check for surface-extension
        self.ext_surface()?;

        match (display_handle, window_handle) {
            #[cfg(all(
                unix,
                not(target_os = "android"),
                not(target_os = "macos"),
                not(target_os = "ios")
            ))]
            (RawDisplayHandle::Xlib(d), RawWindowHandle::Xlib(w)) => {
                self.create_surface_xlib(d.display as *mut _, w.window)
            }
            #[cfg(all(
                unix,
                not(target_os = "android"),
                not(target_os = "macos"),
                not(target_os = "ios")
            ))]
            (RawDisplayHandle::Xcb(d), RawWindowHandle::Xcb(w)) => {
                self.create_surface_xcb(d.connection, w.window)
            }
            #[cfg(all(
                unix,
                not(target_os = "android"),
                not(target_os = "macos"),
                not(target_os = "ios")
            ))]
            (RawDisplayHandle::Wayland(d), RawWindowHandle::Wayland(w)) => {
                self.create_surface_wayland(d.display, w.surface)
            }
            #[cfg(target_os = "android")]
            (RawDisplayHandle::Android(_), RawWindowHandle::AndroidNdk(w)) => {
                self.create_surface_android(w.a_native_window)
            }
            #[cfg(target_os = "windows")]
            (RawDisplayHandle::Windows(_), RawWindowHandle::Windows(w)) => {
                self.create_surface_win32(w.hinstance, w.hwnd)?
            }
            #[cfg(target_os = "macos")]
            (RawDisplayHandle::AppKit(_), RawWindowHandle::AppKit(w)) => {
                self.create_surface_apple(w.ns_view)?
            }
            #[cfg(target_os = "ios")]
            (RawDisplayHandle::UiKit(_), RawWindowHandle::UiKit(w)) => {
                self.create_surface_apple(w.ui_view)?
            }

            _ => Err(Error::UnsupportedWindowSystem),
        }
    }

    /// SAFETY: display and window handles must be valid for the complete lifetime of surface
    pub(crate) fn new_surface(&self, window: &dyn RawWindow) -> Result<Guard<'_, vk::SurfaceKHR>> {
        let surface_raw = unsafe {
            self.create_surface_raw(window.raw_display_handle(), window.raw_window_handle())?
        };
        Ok(Guard::new(self, surface_raw))
    }
}

impl AshInstance {
    pub fn get_physical_device_surface_support(
        &self,
        physical_device: vk::PhysicalDevice,
        queue_family_index: u32,
        surface: vk::SurfaceKHR,
    ) -> bool {
        let Ok(ext_surface) = self.ext_surface() else {
            return false;
        };

        unsafe {
            ext_surface
                .get_physical_device_surface_support(physical_device, queue_family_index, surface)
                .unwrap_or(false)
        }
    }

    pub fn query_swapchain_support(
        &self,
        surface: vk::SurfaceKHR,
        physical_device: vk::PhysicalDevice,
    ) -> Option<SwapchainSupportDetail> {
        let Ok(ext_surface) = self.ext_surface() else {
            return None;
        };
        unsafe {
            let capabilities = ext_surface
                .get_physical_device_surface_capabilities(physical_device, surface)
                .ok()?;
            let formats = ext_surface
                .get_physical_device_surface_formats(physical_device, surface)
                .ok()?;
            let present_modes = ext_surface
                .get_physical_device_surface_present_modes(physical_device, surface)
                .ok()?;
            if formats.is_empty() || present_modes.is_empty() {
                None
            } else {
                Some(SwapchainSupportDetail {
                    capabilities,
                    formats,
                    present_modes,
                })
            }
        }
    }
}

pub struct SwapchainSupportDetail {
    capabilities: vk::SurfaceCapabilitiesKHR,
    formats: Vec<vk::SurfaceFormatKHR>,
    present_modes: Vec<vk::PresentModeKHR>,
}

impl SwapchainSupportDetail {
    pub fn preferred_format(&self) -> vk::SurfaceFormatKHR {
        for available_format in &self.formats {
            if available_format.format == vk::Format::B8G8R8A8_SRGB
                && available_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            {
                return *available_format;
            }
        }

        // return the first format from the list
        self.formats.first().cloned().unwrap()
    }
    pub fn preferred_present_mode(&self, suggested: vk::PresentModeKHR) -> vk::PresentModeKHR {
        if self.present_modes.contains(&suggested) {
            return suggested;
        }
        if suggested == vk::PresentModeKHR::FIFO || suggested == vk::PresentModeKHR::FIFO_RELAXED {
            // find any FIFO Mode
            for &present_mode in self.present_modes.iter() {
                if present_mode == vk::PresentModeKHR::FIFO
                    || present_mode == vk::PresentModeKHR::FIFO_RELAXED
                {
                    return present_mode;
                }
            }
        }
        if suggested != vk::PresentModeKHR::IMMEDIATE {
            // find any VSync Mode (not immediate)
            for &present_mode in self.present_modes.iter() {
                if present_mode != vk::PresentModeKHR::IMMEDIATE {
                    return present_mode;
                }
            }
        }
        self.present_modes.first().copied().unwrap()
    }

    pub fn preferred_composite_alpha(&self) -> vk::CompositeAlphaFlagsKHR {
        if self
            .capabilities
            .supported_composite_alpha
            .contains(vk::CompositeAlphaFlagsKHR::OPAQUE)
        {
            return vk::CompositeAlphaFlagsKHR::OPAQUE;
        }
        if self
            .capabilities
            .supported_composite_alpha
            .contains(vk::CompositeAlphaFlagsKHR::INHERIT)
        {
            return vk::CompositeAlphaFlagsKHR::INHERIT;
        }
        self.capabilities.supported_composite_alpha
    }
}

pub struct SurfaceSwapchain {
    device: Arc<AshDevice>,
    surface_raw: vk::SurfaceKHR,
    swapchain_raw: vk::SwapchainKHR,
    size: Size2,
    image_count: u32,
    surface_format: vk::SurfaceFormatKHR,
    present_mode: vk::PresentModeKHR,
    image_usage: vk::ImageUsageFlags,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    texture_id: Texture,
    acquired_image: u32,
    retired_swapchains: Vec<vk::SwapchainKHR>,
    retired_image_views: Vec<vk::ImageView>,
}

impl AshDevice {
    pub(crate) fn new_swapchain(
        self: &Arc<Self>,
        surface: Guard<'_, vk::SurfaceKHR>,
        window_descriptor: &Window,
    ) -> Result<SurfaceSwapchain> {
        let (image_count, present_mode) = if window_descriptor.vsync {
            (3, vk::PresentModeKHR::MAILBOX)
        } else {
            (2, vk::PresentModeKHR::IMMEDIATE)
        };
        let mut swapchain = SurfaceSwapchain {
            device: self.clone(),
            surface_raw: surface.take(),
            swapchain_raw: vk::SwapchainKHR::null(),
            size: window_descriptor.size,
            image_count,
            surface_format: Default::default(),
            present_mode,
            // TODO: custom usage
            image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST,
            images: Vec::new(),
            image_views: Vec::new(),
            texture_id: Texture::null(),
            acquired_image: !0,
            retired_swapchains: Vec::new(),
            retired_image_views: Vec::new(),
        };
        swapchain.configure()?;
        Ok(swapchain)
    }
}

impl SurfaceSwapchain {
    #[inline]
    pub fn size(&self) -> Size2 {
        self.size
    }

    #[inline]
    pub fn format(&self) -> vk::Format {
        self.surface_format.format
    }

    #[inline]
    pub fn texture_format(&self) -> TextureFormat {
        self.surface_format.format.vk_into()
    }

    #[inline]
    pub fn texture_id(&self) -> Texture {
        self.texture_id
    }

    #[inline]
    pub fn present_mode(&self) -> vk::PresentModeKHR {
        self.present_mode
    }

    #[inline]
    pub fn image_usage(&self) -> vk::ImageUsageFlags {
        self.image_usage
    }

    #[inline]
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    pub fn clear_value(&self) -> vk::ClearColorValue {
        match self.surface_format.format {
            vk::Format::R8_SINT
            | vk::Format::R8G8_SINT
            | vk::Format::R8G8B8_SINT
            | vk::Format::B8G8R8_SINT
            | vk::Format::R8G8B8A8_SINT
            | vk::Format::B8G8R8A8_SINT
            | vk::Format::A8B8G8R8_SINT_PACK32
            | vk::Format::A2R10G10B10_SINT_PACK32
            | vk::Format::A2B10G10R10_SINT_PACK32
            | vk::Format::R16_SINT
            | vk::Format::R16G16_SINT
            | vk::Format::R16G16B16_SINT
            | vk::Format::R16G16B16A16_SINT
            | vk::Format::R32_SINT
            | vk::Format::R32G32_SINT
            | vk::Format::R32G32B32_SINT
            | vk::Format::R32G32B32A32_SINT
            | vk::Format::R64_SINT
            | vk::Format::R64G64_SINT
            | vk::Format::R64G64B64_SINT
            | vk::Format::R64G64B64A64_SINT => vk::ClearColorValue {
                int32: [i32::MIN, i32::MIN, i32::MIN, i32::MAX],
            },

            vk::Format::R8_UINT
            | vk::Format::R8G8_UINT
            | vk::Format::R8G8B8_UINT
            | vk::Format::B8G8R8_UINT
            | vk::Format::R8G8B8A8_UINT
            | vk::Format::B8G8R8A8_UINT
            | vk::Format::A8B8G8R8_UINT_PACK32
            | vk::Format::A2R10G10B10_UINT_PACK32
            | vk::Format::A2B10G10R10_UINT_PACK32
            | vk::Format::R16_UINT
            | vk::Format::R16G16_UINT
            | vk::Format::R16G16B16_UINT
            | vk::Format::R16G16B16A16_UINT
            | vk::Format::R32_UINT
            | vk::Format::R32G32_UINT
            | vk::Format::R32G32B32_UINT
            | vk::Format::R32G32B32A32_UINT
            | vk::Format::R64_UINT
            | vk::Format::R64G64_UINT
            | vk::Format::R64G64B64_UINT
            | vk::Format::R64G64B64A64_UINT => vk::ClearColorValue {
                uint32: [0, 0, 0, u32::MAX],
            },

            _ => vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 1.0],
            },
        }
    }

    #[inline]
    pub const fn is_acquired(&self) -> bool {
        self.acquired_image != !0
    }

    pub fn configure_with(
        &mut self,
        suggested_size: Size2,
        suggested_present_mode: vk::PresentModeKHR,
    ) -> Result<()> {
        self.size = suggested_size;
        self.present_mode = suggested_present_mode;
        self.configure()
    }

    pub fn configure(&mut self) -> Result<()> {
        // TODO: also reconfigure on resize, and when presenting results in `Outdated/Lost`
        // TODO: pass swapchain format to graph

        let ext_swapchain = self.device.ext_swapchain()?;

        let swapchain_support_info = self
            .device
            .instance()
            .query_swapchain_support(self.surface_raw, self.device.physical_device())
            .ok_or(Error::NoSwapchainSupport)?;

        if !swapchain_support_info
            .capabilities
            .supported_usage_flags
            .contains(self.image_usage)
        {
            return Err(Error::SwapchainUsageNotSupported {
                requested: self.image_usage,
                supported: swapchain_support_info.capabilities.supported_usage_flags,
            });
        }

        self.surface_format = swapchain_support_info.preferred_format();

        self.present_mode = swapchain_support_info.preferred_present_mode(self.present_mode);

        let vk::Extent2D { width, height } = swapchain_support_info.capabilities.current_extent;
        if width != u32::MAX {
            self.size = uvec2(width, height)
        } else {
            // clamp size
            if self.size.x < swapchain_support_info.capabilities.min_image_extent.width {
                self.size.x = swapchain_support_info.capabilities.min_image_extent.width;
            } else if self.size.x > swapchain_support_info.capabilities.max_image_extent.width {
                self.size.x = swapchain_support_info.capabilities.max_image_extent.width;
            }
            if self.size.y < swapchain_support_info.capabilities.min_image_extent.height {
                self.size.y = swapchain_support_info.capabilities.min_image_extent.height;
            } else if self.size.y > swapchain_support_info.capabilities.max_image_extent.height {
                self.size.y = swapchain_support_info.capabilities.max_image_extent.height;
            }
        }

        if self.image_count < swapchain_support_info.capabilities.min_image_count {
            self.image_count = swapchain_support_info.capabilities.min_image_count;
        }
        if swapchain_support_info.capabilities.max_image_count > 0
            && self.image_count > swapchain_support_info.capabilities.max_image_count
        {
            self.image_count = swapchain_support_info.capabilities.max_image_count;
        }

        let shared_queue_family_indices = [
            self.device.queues().graphics_family,
            self.device.queues().present_family,
        ];
        let old_swapchain = self.swapchain_raw;
        if old_swapchain != vk::SwapchainKHR::null() {
            // swapchain is retired, even if `create_swapchain` fails
            self.swapchain_raw = vk::SwapchainKHR::null();
            self.retired_swapchains.push(old_swapchain);
        }
        self.swapchain_raw = unsafe {
            ext_swapchain.create_swapchain(
                &vk::SwapchainCreateInfoKHR::builder()
                    .surface(self.surface_raw)
                    .min_image_count(self.image_count)
                    .image_format(self.surface_format.format)
                    .image_color_space(self.surface_format.color_space)
                    .image_extent(vk::Extent2D {
                        width: self.size.x,
                        height: self.size.y,
                    })
                    .image_array_layers(1)
                    .image_usage(self.image_usage)
                    .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                    .queue_family_indices(
                        if shared_queue_family_indices[0] != shared_queue_family_indices[1] {
                            &shared_queue_family_indices
                        } else {
                            &[]
                        },
                    )
                    .pre_transform(swapchain_support_info.capabilities.current_transform)
                    .composite_alpha(swapchain_support_info.preferred_composite_alpha())
                    .present_mode(self.present_mode)
                    .clipped(true)
                    .old_swapchain(old_swapchain)
                    .image_array_layers(1)
                    .build(),
                None,
            )?
        };

        self.images = unsafe { ext_swapchain.get_swapchain_images(self.swapchain_raw)? };
        self.retired_image_views.append(&mut self.image_views);
        for image in &self.images {
            unsafe {
                let image_view = self.device.create_image_view(
                    &vk::ImageViewCreateInfo::builder()
                        .image(*image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(self.surface_format.format)
                        .subresource_range(
                            vk::ImageSubresourceRange::builder()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .base_mip_level(0)
                                .level_count(1)
                                .base_array_layer(0)
                                .layer_count(1)
                                .build(),
                        )
                        .build(),
                    None,
                )?;
                self.image_views.push(image_view);
            }
        }

        Ok(())
    }

    fn acquire_next_image(
        &mut self,
        timeout: u64,
        signal_semaphore: vk::Semaphore,
    ) -> Result<Option<AcquiredSwapchainImage>> {
        let device = self.device.clone();
        let ext_swapchain = device.ext_swapchain()?;

        if self.is_acquired() {
            return Err(Error::SwapchainImageAlreadyAcquired);
        }

        // TODO: better sync mechanism

        let result = unsafe {
            match ext_swapchain.acquire_next_image(
                self.swapchain_raw,
                timeout,
                signal_semaphore,
                vk::Fence::null(),
            ) {
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    // re-configure and re-acquire
                    self.configure()?;
                    ext_swapchain.acquire_next_image(
                        self.swapchain_raw,
                        timeout,
                        signal_semaphore,
                        vk::Fence::null(),
                    )
                }

                // TODO: handle Surface-lost
                //     destroy old surface & swapchain,
                //     re-create surface and swapchain,
                //     and acquire again
                other => other,
            }
        };

        let (index, suboptimal) = match result {
            Ok(r) => r,
            Err(vk::Result::NOT_READY) | Err(vk::Result::TIMEOUT) => {
                return Ok(None);
            }
            Err(vk::Result::ERROR_SURFACE_LOST_KHR) => return Err(Error::SurfaceLost),
            Err(e) => return Err(e.into()),
        };
        self.acquired_image = index;
        Ok(Some(AcquiredSwapchainImage {
            index,
            image: self.images[index as usize],
            image_view: self.image_views[index as usize],
            suboptimal,
        }))
    }
}

impl Drop for SurfaceSwapchain {
    fn drop(&mut self) {
        if self.swapchain_raw != vk::SwapchainKHR::null()
            || self.surface_raw != vk::SurfaceKHR::null()
            || !self.retired_swapchains.is_empty()
            || !self.retired_image_views.is_empty()
        {
            unsafe {
                self.device.device_wait_idle().unwrap();
            }
        }

        for image_view in self.image_views.drain(..) {
            unsafe {
                self.device.destroy_image_view(image_view, None);
            }
        }
        // don't destroy images obtained from get_swapchain_images! They are destroyed together with the swapchain object.

        let ext_swapchain = self.device.ext_swapchain().unwrap();
        for r in self.retired_swapchains.drain(..) {
            unsafe {
                ext_swapchain.destroy_swapchain(r, None);
            }
        }

        self.images.clear();
        if self.swapchain_raw != vk::SwapchainKHR::null() {
            unsafe {
                ext_swapchain.destroy_swapchain(self.swapchain_raw, None);
            }
        }

        if self.surface_raw != vk::SurfaceKHR::null() {
            let ext_surface = self.device.instance().ext_surface().unwrap();
            unsafe {
                ext_surface.destroy_surface(self.surface_raw, None);
            }
        }
    }
}

impl AshRendererFull {
    pub(crate) fn insert_swapchain(
        &mut self,
        swapchain: SurfaceSwapchain,
        window_id: WindowId,
    ) -> Result<()> {
        if let Some(old_swapchain) = self.surfaces.insert(window_id, swapchain) {
            self.res.textures.remove(old_swapchain.texture_id);
        }
        let surface = &mut self.surfaces[window_id];
        surface.texture_id = self
            .res
            .textures
            .insert((vk::Image::null(), vk::ImageView::null()));
        Ok(())
    }

    pub(crate) fn acquire_swapchain_image(
        &mut self,
        window_id: WindowId,
        timeout: u64,
        signal_semaphore: vk::Semaphore,
    ) -> Result<Option<Texture>> {
        let _ = tracing::trace_span!("AquireImage").entered();
        let surface_swapchain = self.surfaces.get_mut(window_id).expect("swapchain");
        // TODO: create surface & swapchain on demand?
        Ok(surface_swapchain
            .acquire_next_image(timeout, signal_semaphore)?
            .map(|_swapchain_image| {
                //TODO: associate texture
                todo!()
            }))
    }

    pub(crate) fn get_num_acquired_swapchains(&self) -> usize {
        self.surfaces
            .iter()
            .filter(|(_, s)| s.is_acquired())
            .count()
    }

    pub(crate) fn present_acquired_swapchain_images(
        &mut self,
        wait_semaphores: &[vk::Semaphore],
    ) -> Result<()> {
        let _ = tracing::trace_span!("Present").entered();
        let ext_swapchain = self.device.ext_swapchain()?;
        let mut acquired_surface_swapchains: Vec<_> = self
            .surfaces
            .iter_mut()
            .map(|(_, s)| s)
            .filter(|s| s.is_acquired())
            .collect();
        let swapchains: Vec<_> = acquired_surface_swapchains
            .iter()
            .map(|s| s.swapchain_raw)
            .collect();
        let image_indices: Vec<_> = acquired_surface_swapchains
            .iter()
            .map(|s| s.acquired_image)
            .collect();
        let mut results = Vec::new();
        results.resize(acquired_surface_swapchains.len(), vk::Result::SUCCESS);
        let result = unsafe {
            ext_swapchain.queue_present(
                self.device.queues().present,
                &vk::PresentInfoKHR::builder()
                    .wait_semaphores(wait_semaphores)
                    .swapchains(&swapchains)
                    .image_indices(&image_indices)
                    .results(&mut results)
                    .build(),
            )
        };

        // reset image index
        for surface_swapchain in &mut acquired_surface_swapchains {
            surface_swapchain.acquired_image = !0;
        }

        match result {
            Ok(_)
            | Err(vk::Result::SUBOPTIMAL_KHR)
            | Err(vk::Result::ERROR_OUT_OF_DATE_KHR)
            | Err(vk::Result::ERROR_SURFACE_LOST_KHR) => (),
            Err(e) => {
                self.retire_swapchains();
                return Err(e.into());
            }
        }

        for (result, surface_swapchain) in results
            .into_iter()
            .zip(acquired_surface_swapchains.into_iter())
        {
            if result == vk::Result::SUBOPTIMAL_KHR || result == vk::Result::ERROR_OUT_OF_DATE_KHR {
                surface_swapchain.configure()?;
            } else if result == vk::Result::ERROR_SURFACE_LOST_KHR {
                // TODO: re-create surface and re-configure swapchain
            }
        }

        self.retire_swapchains();

        Ok(())
    }

    fn retire_swapchains(&mut self) {
        let frame = &mut self.frames[self.current_frame];
        for (_, surface_swapchain) in &mut self.surfaces {
            frame
                .retired_swapchains
                .append(&mut surface_swapchain.retired_swapchains);
            frame
                .retired_image_views
                .append(&mut surface_swapchain.retired_image_views);
        }
    }
}

pub struct AcquiredSwapchainImage {
    index: u32,
    pub image: vk::Image,
    pub image_view: vk::ImageView,
    pub suboptimal: bool,
}
