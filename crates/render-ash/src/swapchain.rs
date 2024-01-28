use std::rc::Rc;

use ash::{extensions::khr, vk};
use pulz_render::{
    math::{uvec2, USize2},
    texture::{Texture, TextureFormat},
};
use pulz_window::{HasWindowAndDisplayHandle, Size2, Window, WindowDescriptor, WindowId, Windows};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use tracing::{debug, info};

use crate::{
    convert::VkInto,
    device::AshDevice,
    drop_guard::{Destroy, Guard},
    instance::AshInstance,
    resources::AshResources,
    AshRendererFull, Error, Result,
};

impl Destroy for vk::SurfaceKHR {
    type Context = AshInstance;
    #[inline]
    unsafe fn destroy(self, instance: &AshInstance) {
        if self != Self::null() {
            let ext_surface = instance.ext_surface().unwrap();
            ext_surface.destroy_surface(self, None);
        }
    }
}

impl Destroy for vk::SwapchainKHR {
    type Context = AshDevice;
    #[inline]
    unsafe fn destroy(self, device: &AshDevice) {
        if self != Self::null() {
            let ext_swapchain = device.ext_swapchain().unwrap();
            ext_swapchain.destroy_swapchain(self, None);
        }
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

    /// SAFETY: display and window handles must be valid for the complete lifetime of surface
    unsafe fn create_surface_raw(
        &self,
        raw_display_handle: RawDisplayHandle,
        raw_window_handle: RawWindowHandle,
    ) -> Result<vk::SurfaceKHR> {
        // check for surface-extension
        self.ext_surface()?;

        match (raw_display_handle, raw_window_handle) {
            #[cfg(all(
                unix,
                not(target_os = "android"),
                not(target_os = "macos"),
                not(target_os = "ios")
            ))]
            (RawDisplayHandle::Xlib(d), RawWindowHandle::Xlib(w)) => self.create_surface_xlib(
                d.display.ok_or(Error::WindowNotAvailable)?.as_ptr().cast(),
                w.window,
            ),
            #[cfg(all(
                unix,
                not(target_os = "android"),
                not(target_os = "macos"),
                not(target_os = "ios")
            ))]
            (RawDisplayHandle::Xcb(d), RawWindowHandle::Xcb(w)) => self.create_surface_xcb(
                d.connection.ok_or(Error::WindowNotAvailable)?.as_ptr(),
                w.window.get(),
            ),
            #[cfg(all(
                unix,
                not(target_os = "android"),
                not(target_os = "macos"),
                not(target_os = "ios")
            ))]
            (RawDisplayHandle::Wayland(d), RawWindowHandle::Wayland(w)) => {
                self.create_surface_wayland(d.display.as_ptr(), w.surface.as_ptr())
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
            (RawDisplayHandle::AppKit(_), RawWindowHandle::AppKit(h)) => {
                use raw_window_metal::{appkit, Layer};
                let layer = match appkit::metal_layer_from_handle(h) {
                    Layer::Existing(layer) | Layer::Allocated(layer) => layer.cast(),
                    Layer::None => return Err(vk::Result::ERROR_INITIALIZATION_FAILED),
                };
                self.create_surface_metal(layer)?
            }
            #[cfg(target_os = "ios")]
            (RawDisplayHandle::UiKit(_), RawWindowHandle::UiKit(w)) => {
                use raw_window_metal::{uikit, Layer};
                let layer = match uikit::metal_layer_from_handle(h) {
                    Layer::Existing(layer) | Layer::Allocated(layer) => layer.cast(),
                    Layer::None => return Err(vk::Result::ERROR_INITIALIZATION_FAILED),
                };
                self.create_surface_metal(layer)?
            }

            _ => Err(Error::UnsupportedWindowSystem),
        }
    }

    /// SAFETY: display and window handles must be valid for the complete lifetime of surface
    pub(crate) unsafe fn new_surface(
        &self,
        window: &dyn HasWindowAndDisplayHandle,
    ) -> Result<Guard<'_, vk::SurfaceKHR>> {
        fn map_handle_error(e: raw_window_handle::HandleError) -> Error {
            use raw_window_handle::HandleError;
            match e {
                HandleError::Unavailable => Error::WindowNotAvailable,
                _ => Error::UnsupportedWindowSystem,
            }
        }
        let raw_display_handle = window.display_handle().map_err(map_handle_error)?.as_raw();
        let raw_window_handle = window.window_handle().map_err(map_handle_error)?.as_raw();
        let surface_raw = self.create_surface_raw(raw_display_handle, raw_window_handle)?;
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

pub struct AshSurfaceSwapchain {
    surface_raw: vk::SurfaceKHR,
    swapchain_raw: vk::SwapchainKHR,
    size: Size2,
    image_count: u32,
    surface_format: vk::SurfaceFormatKHR,
    present_mode: vk::PresentModeKHR,
    image_usage: vk::ImageUsageFlags,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,
    textures: Vec<Texture>,
    acquired_image: u32,
    window: Rc<dyn HasWindowAndDisplayHandle>, // for keeping ownership
}

impl AshSurfaceSwapchain {
    fn window_swapchain_config(window: &WindowDescriptor) -> (u32, vk::PresentModeKHR, USize2) {
        let (image_count, present_mode) = if window.vsync {
            (3, vk::PresentModeKHR::MAILBOX)
        } else {
            (2, vk::PresentModeKHR::IMMEDIATE)
        };
        (image_count, present_mode, window.size)
    }

    fn new_unconfigured(
        window: Rc<dyn HasWindowAndDisplayHandle>,
        surface_raw: vk::SurfaceKHR,
    ) -> Self {
        Self {
            surface_raw,
            swapchain_raw: vk::SwapchainKHR::null(),
            size: USize2::new(0, 0),
            image_count: 0,
            surface_format: Default::default(),
            present_mode: vk::PresentModeKHR::IMMEDIATE,
            // TODO: custom usage
            image_usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST,
            images: Vec::new(),
            image_views: Vec::new(),
            textures: Vec::new(),
            acquired_image: !0,
            window,
        }
    }

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

    #[inline]
    pub const fn is_acquired(&self) -> bool {
        self.acquired_image != !0
    }

    pub fn put_to_garbage(&mut self, res: &mut AshResources) -> vk::SwapchainKHR {
        let old_swapchain = self.swapchain_raw;
        if old_swapchain != vk::SwapchainKHR::null() {
            self.swapchain_raw = vk::SwapchainKHR::null();
            for texture_id in self.textures.drain(..) {
                res.textures.remove(texture_id); // forget texture without destroy!
            }
            let garbage = res.current_frame_garbage_mut();
            garbage.image_views.append(&mut self.image_views);
            garbage.swapchains.push(old_swapchain);
            self.images.clear(); // images owned by swapchain!
        }
        old_swapchain
    }

    /// #SAFETY: there must not be any outstanding operations
    pub unsafe fn destroy_with_surface(mut self, res: &mut AshResources) -> Result<()> {
        let swapchain = self.swapchain_raw;
        for texture_id in self.textures.drain(..) {
            res.textures.remove(texture_id); // forget texture without destroy!
        }
        for image_view in self.image_views.drain(..) {
            res.device().destroy_image_view(image_view, None);
        }
        self.images.clear(); // images owned by swapchain!
        if swapchain != vk::SwapchainKHR::null() {
            self.swapchain_raw = vk::SwapchainKHR::null();
            res.device()
                .ext_swapchain()?
                .destroy_swapchain(swapchain, None);
        }
        let surface = self.surface_raw;
        if surface != vk::SurfaceKHR::null() {
            self.surface_raw = vk::SurfaceKHR::null();
            if let Ok(ext_surface) = res.device().instance().ext_surface() {
                ext_surface.destroy_surface(surface, None);
            }
        }
        Ok(())
    }

    fn configure_with(&mut self, res: &mut AshResources, window: &WindowDescriptor) -> Result<()> {
        let (suggested_image_count, suggested_present_mode, suggessted_size) =
            Self::window_swapchain_config(window);
        self.image_count = suggested_image_count;
        self.size = suggessted_size;
        self.present_mode = suggested_present_mode;
        self.configure(res)
    }

    fn configure(&mut self, res: &mut AshResources) -> Result<()> {
        debug!(
            "re-creating swapchain, recreate={:?}",
            self.swapchain_raw != vk::SwapchainKHR::null()
        );
        // check swapchain support
        res.device().ext_swapchain()?;

        // TODO: also reconfigure on resize, and when presenting results in `Outdated/Lost`
        // TODO: pass swapchain format to graph

        let swapchain_support_info = res
            .instance()
            .query_swapchain_support(self.surface_raw, res.device().physical_device())
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
            res.device().queues().graphics_family,
            res.device().queues().present_family,
        ];
        // old swapchain is retired, even if `create_swapchain` fails
        let old_swapchain = self.put_to_garbage(res);
        self.swapchain_raw = unsafe {
            res.device().ext_swapchain()?.create_swapchain(
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

        debug_assert_ne!(vk::SwapchainKHR::null(), self.swapchain_raw);
        debug_assert_eq!(0, self.images.len());
        debug_assert_eq!(0, self.image_views.len());
        debug_assert_eq!(0, self.textures.len());
        self.images = unsafe {
            res.device()
                .ext_swapchain()?
                .get_swapchain_images(self.swapchain_raw)?
        };

        for image in self.images.iter().copied() {
            unsafe {
                let image_view = res.device().create_image_view(
                    &vk::ImageViewCreateInfo::builder()
                        .image(image)
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
                let texture = res.textures.insert((image, image_view, None));
                self.textures.push(texture);
            }
        }

        Ok(())
    }

    pub(crate) fn acquire_next_image(
        &mut self,
        res: &mut AshResources,
        signal_semaphore: vk::Semaphore,
    ) -> Result<Option<AcquiredSwapchainImage>> {
        if self.is_acquired() {
            return Err(Error::SwapchainImageAlreadyAcquired);
        }

        let result = unsafe {
            match res.device().ext_swapchain()?.acquire_next_image(
                self.swapchain_raw,
                !0,
                signal_semaphore,
                vk::Fence::null(),
            ) {
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    // re-configure and re-acquire
                    self.configure(res)?;
                    res.device().ext_swapchain()?.acquire_next_image(
                        self.swapchain_raw,
                        !0,
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
            texture: self.textures[index as usize],
            suboptimal,
        }))
    }
}

impl Drop for AshSurfaceSwapchain {
    fn drop(&mut self) {
        assert_eq!(0, self.images.len());
        assert_eq!(0, self.image_views.len());
        assert_eq!(0, self.textures.len());
        assert_eq!(vk::SwapchainKHR::null(), self.swapchain_raw);
        assert_eq!(vk::SurfaceKHR::null(), self.surface_raw);
    }
}

impl AshRendererFull {
    pub(crate) fn init_swapchain(
        &mut self,
        window_id: WindowId,
        window_descriptor: &Window,
        window: Rc<dyn HasWindowAndDisplayHandle>,
        surface: Guard<'_, vk::SurfaceKHR>,
    ) -> Result<&mut AshSurfaceSwapchain> {
        assert!(self
            .surfaces
            .insert(
                window_id,
                AshSurfaceSwapchain::new_unconfigured(window, surface.take())
            )
            .is_none());
        let swapchain = self.surfaces.get_mut(window_id).unwrap();
        swapchain.configure_with(&mut self.res, window_descriptor)?;
        Ok(swapchain)
    }

    pub(crate) fn destroy_swapchain(&mut self, window_id: WindowId) -> Result<()> {
        let Some(swapchain) = self.surfaces.remove(window_id) else {
            return Err(Error::WindowNotAvailable);
        };
        self.res.wait_idle_and_clear_garbage()?;
        unsafe {
            swapchain.destroy_with_surface(&mut self.res)?;
        }
        Ok(())
    }

    pub(crate) fn destroy_all_swapchains(&mut self) -> Result<()> {
        self.res.wait_idle_and_clear_garbage()?;
        unsafe {
            for (_window_id, swapchain) in self.surfaces.drain() {
                swapchain.destroy_with_surface(&mut self.res)?;
            }
        }
        Ok(())
    }

    pub(crate) fn reconfigure_swapchains(&mut self, windows: &Windows) {
        self.surfaces.retain(|window_id, surface_swapchain| {
            let Some(window) = windows.get(window_id) else {
                surface_swapchain.put_to_garbage(&mut self.res);
                return false;
            };
            //TODO: re-create also the surface, when SURFACE_LOST was returned in earlier calls.
            //TODO: better resize check (don't compare size, but use a 'dirty'-flag, or listener)
            //TODO: sync
            if window.size != surface_swapchain.size() {
                info!(
                    "surface sized changed: {} => {}",
                    surface_swapchain.size(),
                    window.size
                );
                surface_swapchain
                    .configure_with(&mut self.res, window)
                    .unwrap();
            }
            true
        });
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
                return Err(e.into());
            }
        }

        for (result, surface_swapchain) in results
            .into_iter()
            .zip(acquired_surface_swapchains.into_iter())
        {
            if result == vk::Result::SUBOPTIMAL_KHR || result == vk::Result::ERROR_OUT_OF_DATE_KHR {
                surface_swapchain.configure(&mut self.res)?;
            } else if result == vk::Result::ERROR_SURFACE_LOST_KHR {
                // TODO: re-create surface and re-configure swapchain
            }
        }

        Ok(())
    }
}

pub struct AcquiredSwapchainImage {
    index: u32,
    pub image: vk::Image,
    pub image_view: vk::ImageView,
    pub texture: Texture,
    pub suboptimal: bool,
}
