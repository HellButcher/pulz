use std::{
    ops::{Deref, DerefMut},
    rc::Rc,
};

use render::texture::TextureFormat;
use tracing::info;
use window::{HasRawWindowHandle, Size2, Window};

use crate::{convert::WgpuInto, WgpuRendererBackend};

pub struct Surface {
    surface: wgpu::Surface,
    window_handle: Rc<dyn HasRawWindowHandle>, // holds reference to window to ensure sufface is still valid until destruction
    size: Size2,
    vsync: bool,
    format: TextureFormat,
}

impl Surface {
    pub fn create(
        instance: &wgpu::Instance,
        window: &Window,
        window_handle: Rc<dyn HasRawWindowHandle>,
    ) -> Self {
        let surface = unsafe { instance.create_surface(&window_handle) };
        Self {
            surface,
            window_handle,
            size: window.size,
            vsync: window.vsync,
            format: TextureFormat::DEFAULT,
        }
    }

    #[inline]
    pub fn size(&self) -> Size2 {
        self.size
    }

    #[inline]
    pub fn format(&self) -> TextureFormat {
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

    pub fn configure(&mut self, backend: &WgpuRendererBackend) {
        // TODO: also reconfigure on resize, and when presenting results in `Outdated/Lost`
        // TODO: pass swapchain format to graph
        let format = self
            .surface
            .get_preferred_format(&backend.adapter)
            .expect("surface not compatible");
        self.format = format.wgpu_into();
        let present_mode = if self.vsync {
            wgpu::PresentMode::Fifo
        } else {
            wgpu::PresentMode::Immediate
        };
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: self.size.x,
            height: self.size.y,
            present_mode,
        };
        self.surface.configure(&backend.device, &surface_config);
    }
}

impl Deref for Surface {
    type Target = wgpu::Surface;
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
