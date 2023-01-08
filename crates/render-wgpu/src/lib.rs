#![warn(
    // missing_docs,
    // rustdoc::missing_doc_code_examples,
    future_incompatible,
    rust_2018_idioms,
    unused,
    trivial_casts,
    trivial_numeric_casts,
    unused_lifetimes,
    unused_qualifications,
    unused_crate_dependencies,
    clippy::cargo,
    clippy::multiple_crate_versions,
    clippy::empty_line_after_outer_attr,
    clippy::fallible_impl_from,
    clippy::redundant_pub_crate,
    clippy::use_self,
    clippy::suspicious_operation_groupings,
    clippy::useless_let_if_seq,
    // clippy::missing_errors_doc,
    // clippy::missing_panics_doc,
    clippy::wildcard_imports
)]
#![doc(html_logo_url = "https://raw.githubusercontent.com/HellButcher/pulz/master/docs/logo.png")]
#![doc(html_no_source)]
#![doc = include_str!("../README.md")]

use std::rc::Rc;

use convert::ConversionError;
use graph::WgpuRenderGraph;
use pulz_ecs::prelude::*;
use pulz_render::{graph::RenderGraph, RenderModule, RenderSystemPhase};
use pulz_window::{RawWindow, RawWindowHandles, Window, WindowId, Windows, WindowsMirror};
use resources::WgpuResources;
use surface::Surface;
use thiserror::Error;
use tracing::info;

mod backend;
mod convert;
mod graph;
mod resources;
mod surface;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("No suitable GPU adapters found on the system!")]
    NoAdapter,

    #[error("Unable to request a suitable device!")]
    NoDevice,

    #[error("The window is not available, or it has no raw-window-handle")]
    WindowNotAvailable,

    #[error("Unable to convert objects")]
    ConversionError(#[from] ConversionError),

    #[error("unknown renderer error")]
    Unknown,
}

impl From<wgpu::RequestDeviceError> for Error {
    fn from(_: wgpu::RequestDeviceError) -> Self {
        Self::NoDevice
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub struct WgpuRenderer {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    resources: WgpuResources,
    surfaces: WindowsMirror<Surface>,
    graph: WgpuRenderGraph,
    tmp_surface_textures: Vec<wgpu::SurfaceTexture>,
}

fn backend_bits_from_env_or_default() -> wgpu::Backends {
    wgpu::util::backend_bits_from_env().unwrap_or(wgpu::Backends::PRIMARY | wgpu::Backends::GL)
}

async fn initialize_adapter_from_env_or_default(
    instance: &wgpu::Instance,
    backend_bits: wgpu::Backends,
    compatible_surface: Option<&wgpu::Surface>,
) -> Option<wgpu::Adapter> {
    match wgpu::util::initialize_adapter_from_env(instance, backend_bits) {
        Some(a) => Some(a),
        None => {
            let power_preference = wgpu::util::power_preference_from_env().unwrap_or_default();
            instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference,
                    force_fallback_adapter: false,
                    compatible_surface,
                })
                .await
        }
    }
}

impl WgpuRenderer {
    pub async fn new() -> Result<Self> {
        let backends = backend_bits_from_env_or_default();
        let instance = wgpu::Instance::new(backends);
        let adapter = initialize_adapter_from_env_or_default(&instance, backends, None)
            .await
            .ok_or(Error::NoAdapter)?;
        Self::for_adapter(instance, adapter).await
    }

    /// # Unsafe
    /// Raw Window Handle must be a valid object to create a surface
    /// upon and must remain valid for the lifetime of the surface.
    pub async fn for_window(
        window_id: WindowId,
        window: &Window,
        window_handle: Rc<dyn RawWindow>,
    ) -> Result<Self> {
        let backends = backend_bits_from_env_or_default();
        let instance = wgpu::Instance::new(backends);
        let surface = Surface::create(&instance, window, window_handle);
        let adapter = initialize_adapter_from_env_or_default(&instance, backends, Some(&surface))
            .await
            .ok_or(Error::NoAdapter)?;
        let mut renderer = Self::for_adapter(instance, adapter).await?;
        renderer.surfaces.insert(window_id, surface);
        Ok(renderer)
    }

    pub async fn for_adapter(instance: wgpu::Instance, adapter: wgpu::Adapter) -> Result<Self> {
        let trace_dir = std::env::var("WGPU_TRACE");
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                    limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
                },
                trace_dir.ok().as_ref().map(std::path::Path::new),
            )
            .await?;

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            resources: WgpuResources::new(),
            surfaces: WindowsMirror::new(),
            tmp_surface_textures: Vec::new(),
            graph: WgpuRenderGraph::new(),
        })
    }

    fn reconfigure_surfaces(&mut self, windows: &Windows) {
        for (window_id, surface) in self.surfaces.iter_mut() {
            if let Some(window) = windows.get(window_id) {
                if surface.update(window) {
                    surface.configure(&self.adapter, &self.device);
                }
            }
        }
    }

    fn aquire_swapchain_images(&mut self) {
        let _ = tracing::trace_span!("AquireImages").entered();
        assert_eq!(0, self.tmp_surface_textures.len());

        self.tmp_surface_textures
            .reserve_exact(self.surfaces.capacity());

        for (_window, surface) in self.surfaces.iter_mut() {
            // TODO: only affected/updated surfaces/windows
            let tex = match surface.get_current_texture() {
                Ok(t) => t,
                Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                    info!("reconfigure surface (outdated)");
                    surface.configure(&self.adapter, &self.device);
                    surface
                        .get_current_texture()
                        .expect("Failed to acquire next surface texture!")
                }
                Err(e) => panic!("unable to aquire next frame: {}", e), // TODO: better error handling
            };
            self.tmp_surface_textures.push(tex);
        }
    }

    fn present_swapchain_images(&mut self) {
        let _ = tracing::trace_span!("Present").entered();

        for surface_texture in self.tmp_surface_textures.drain(..) {
            surface_texture.present();
        }
    }

    fn run_graph(&mut self, src_graph: &RenderGraph) {
        if self.tmp_surface_textures.is_empty() {
            // skip
            return;
        }
        let _ = tracing::trace_span!("RunGraph").entered();
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let cmds = self.graph.execute(src_graph, encoder);
        self.queue.submit(cmds);
    }

    fn run_render_system(
        mut renderer: ResMut<'_, Self>,
        windows: Res<'_, Windows>,
        src_graph: Res<'_, RenderGraph>,
    ) {
        renderer.reconfigure_surfaces(&windows);
        renderer.graph.update(&src_graph);
        renderer.aquire_swapchain_images();
        renderer.run_graph(&src_graph);
        renderer.present_swapchain_images();
    }
}

impl ModuleWithOutput for WgpuRenderer {
    type Output<'l> = &'l mut Self;

    fn install_modules(&self, res: &mut Resources) {
        res.install(RenderModule);
    }

    fn install_resources(self, res: &mut Resources) -> &mut Self {
        let resource_id = res.insert_unsend(self);
        res.get_mut_id(resource_id).unwrap()
    }

    fn install_systems(schedule: &mut Schedule) {
        schedule
            .add_system(Self::run_render_system)
            .into_phase(RenderSystemPhase::Render);
    }
}

pub struct WgpuRendererBuilder {
    window: Option<WindowId>,
}

impl WgpuRendererBuilder {
    #[inline]
    pub const fn new() -> Self {
        Self { window: None }
    }

    /// # Unsafe
    /// Raw Window Handle must be a valid object to create a surface
    /// upon and must remain valid for the lifetime of the surface.
    #[inline]
    pub unsafe fn with_window(mut self, window_id: WindowId) -> Self {
        self.window = Some(window_id);
        self
    }

    pub async fn install(self, res: &mut Resources) -> Result<&mut WgpuRenderer> {
        let renderer = if let Some(window_id) = self.window {
            let windows = res.borrow_res::<Windows>().unwrap();
            // TODO: make not dependent on descriptor.
            // add size-method to RawWindow
            let descriptor = &windows[window_id];
            let raw_window_handles = res.borrow_res::<RawWindowHandles>().unwrap();
            let handle = raw_window_handles
                .get(window_id)
                .and_then(|h| h.upgrade())
                .ok_or(Error::WindowNotAvailable)?;
            WgpuRenderer::for_window(window_id, descriptor, handle).await?
        } else {
            WgpuRenderer::new().await?
        };
        Ok(res.install(renderer))
    }
}
