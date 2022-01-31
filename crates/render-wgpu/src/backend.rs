use pulz_render::backend::RenderBackend;
use pulz_window::WindowId;

use crate::{Error, WgpuRendererBackend};

pub enum BackendTexture {
    Texture {
        texture: wgpu::Texture,
        view: wgpu::TextureView,
    },
    Surface {
        window: WindowId,
        view: wgpu::TextureView,
    },
}

impl BackendTexture {
    #[inline]
    pub fn view(&self) -> &wgpu::TextureView {
        match self {
            Self::Texture { view, .. } => view,
            Self::Surface { view, .. } => view,
        }
    }
}

impl RenderBackend for WgpuRendererBackend {
    type Error = Error;
    type Buffer = wgpu::Buffer;
    type Texture = BackendTexture;
    type ShaderModule = wgpu::ShaderModule;
}
