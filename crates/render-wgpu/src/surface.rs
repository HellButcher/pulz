use std::{
    ops::{Deref, DerefMut},
    rc::Rc,
};

use pulz_window::{HasWindowAndDisplayHandle, Size2, Window};
use tracing::info;

use crate::{Error, Result};

pub struct Surface {
    surface: wgpu::Surface<'static>,
    size: Size2,
    vsync: bool,
    format: wgpu::TextureFormat,
    window: Rc<dyn HasWindowAndDisplayHandle>,
}

impl Surface {
    pub fn create(
        instance: &wgpu::Instance,
        window_descriptor: &Window,
        window: Rc<dyn HasWindowAndDisplayHandle>,
    ) -> Result<Self> {
        fn map_handle_error(e: raw_window_handle::HandleError) -> Error {
            use raw_window_handle::HandleError;
            match e {
                HandleError::Unavailable => Error::WindowNotAvailable,
                _ => Error::UnsupportedWindowSystem,
            }
        }
        let raw_display_handle = window.display_handle().map_err(map_handle_error)?.as_raw();
        let raw_window_handle = window.window_handle().map_err(map_handle_error)?.as_raw();
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle,
                raw_window_handle,
            })?
        };
        Ok(Self {
            surface,
            size: window_descriptor.size,
            vsync: window_descriptor.vsync,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            window,
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
