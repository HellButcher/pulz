use std::ops::{Deref, DerefMut};

use pulz_window::{RawWindow, Size2, Window};
use tracing::info;

pub struct Surface {
    surface: wgpu::Surface,
    size: Size2,
    vsync: bool,
    format: wgpu::TextureFormat,
}

impl Surface {
    pub fn create(
        instance: &wgpu::Instance,
        window: &Window,
        window_handle: &dyn RawWindow,
    ) -> Result<Self, wgpu::CreateSurfaceError> {
        let surface = unsafe { instance.create_surface(&window_handle)? };
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
        let capabilities = self.surface.get_capabilities(adapter);
        self.format = capabilities
            .formats
            .first()
            .copied()
            .expect("surface not compatible");
        let present_mode = if self.vsync {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        };
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.format,
            width: self.size.x,
            height: self.size.y,
            present_mode,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        self.surface.configure(device, &surface_config);
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
