use std::ops::{Deref, DerefMut};

use pulz_window::{DisplayHandle, Size2, Window, WindowHandle};
use tracing::info;

use crate::{Error, Result};

pub struct Surface {
    surface: wgpu::Surface<'static>,
    size: Size2,
    vsync: bool,
    format: wgpu::TextureFormat,
}

impl Surface {
    /// UNSAFE: needs to ensure, whndow is alive while surface is alive
    pub unsafe fn create(
        instance: &wgpu::Instance,
        window: &Window,
        display_handle: DisplayHandle<'_>,
        window_handle: WindowHandle<'_>,
    ) -> Result<Self> {
        fn map_handle_error(e: raw_window_handle::HandleError) -> Error {
            use raw_window_handle::HandleError;
            match e {
                HandleError::Unavailable => Error::WindowNotAvailable,
                _ => Error::UnsupportedWindowSystem,
            }
        }
        let raw_display_handle = display_handle.as_raw();
        let raw_window_handle = window_handle.as_raw();
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle,
                raw_window_handle,
            })?
        };
        Ok(Self {
            surface,
            size: window.size,
            vsync: window.vsync,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
        })
    }

    #[inline]
    pub fn size(&self) -> Size2 {
        self.size
    }

    #[inline]
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn update(&mut self, window: &Window) -> bool {
        let mut changed = false;
        if self.vsync != window.vsync {
            info!("window vsync changed: {} => {}", self.vsync, window.vsync);
            self.vsync = window.vsync;
            changed = true;
        }
        if self.size != window.size {
            info!("window size changed: {} => {}", self.size, window.size);
            self.size = window.size;
            changed = true;
        }
        changed
    }

    pub fn configure(&mut self, adapter: &wgpu::Adapter, device: &wgpu::Device) {
        // TODO: also reconfigure on resize, and when presenting results in `Outdated/Lost`
        let surface_config = self
            .surface
            .get_default_config(adapter, self.size.x, self.size.y)
            .expect("surface not supported by adapter");

        self.surface.configure(device, &surface_config);
    }
}

impl Deref for Surface {
    type Target = wgpu::Surface<'static>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.surface
    }
}

impl DerefMut for Surface {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.surface
    }
}
