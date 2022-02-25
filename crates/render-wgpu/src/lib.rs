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

use std::{
    ops::{Deref, DerefMut},
    rc::Rc,
};

use backend::BackendTexture;

use ecs::{module::Module, resource::Resources, schedule::Schedule, system::IntoSystem};
use raw_window_handle::HasRawWindowHandle;
use render::{
    backend::RenderBackendTypes,
    cache::{Cache, Cacheable},
    render_graph::graph::RenderGraph,
    render_resource::RenderBackendResources,
    texture::{TextureCache, TextureDescriptor, TextureDimensions, TextureUsage},
    view::surface::{Msaa, SurfaceTarget, SurfaceTargets},
    RenderSystemLabel,
};
use surface::Surface;
use thiserror::Error;
use tracing::info;

mod backend;
mod convert;
mod surface;

use window::{RawWindowHandles, Window, WindowId, Windows, WindowsMirror};

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("No suitable GPU adapters found on the system!")]
    NoAdapter,

    #[error("Unable to request a suitable device!")]
    NoDevice,

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
    backend: WgpuRendererBackend,
    surfaces: WindowsMirror<Surface>,
    tmp_surface_targets: SurfaceTargets,
    tmp_surface_textures: Vec<wgpu::SurfaceTexture>,
}

pub struct WgpuRendererBackend {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    resources: RenderBackendResources<Self>,
}

impl Deref for WgpuRenderer {
    type Target = WgpuRendererBackend;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.backend
    }
}

impl DerefMut for WgpuRenderer {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.backend
    }
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
        window_handle: Rc<dyn HasRawWindowHandle>,
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
            backend: WgpuRendererBackend {
                adapter,
                device,
                queue,
                resources: RenderBackendResources::new(),
            },
            surfaces: WindowsMirror::new(),
            tmp_surface_targets: SurfaceTargets::new(),
            tmp_surface_textures: Vec::new(),
        })
    }

    #[inline]
    pub fn backend(&self) -> &WgpuRendererBackend {
        &self.backend
    }

    #[inline]
    pub fn backend_mut(&mut self) -> &mut WgpuRendererBackend {
        &mut self.backend
    }

    fn reconfigure_surfaces(&mut self, windows: &Windows) {
        for (window_id, surface) in self.surfaces.iter_mut() {
            if let Some(window) = windows.get(window_id) {
                if surface.update(window) {
                    surface.configure(&self.backend);
                }
            }
        }
    }

    fn aquire_swapchain_images(&mut self, res: &mut Resources) {
        let _ = tracing::trace_span!("Aquire Images").entered();
        assert_eq!(0, self.tmp_surface_targets.len());

        self.tmp_surface_targets
            .set_capacity(self.surfaces.capacity());

        let mut texture_cache = res.borrow_res_mut::<TextureCache>().unwrap();
        let samples = res.borrow_res::<Msaa>().map_or(1, |msaa| msaa.samples);

        for (window, surface) in self.surfaces.iter_mut() {
            // TODO: only affected/updated surfaces/windows
            let tex = match surface.get_current_texture() {
                Ok(t) => t,
                Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                    info!("reconfigure surface (outdated)");
                    surface.configure(&self.backend);
                    surface
                        .get_current_texture()
                        .expect("Failed to acquire next surface texture!")
                }
                Err(e) => panic!("unable to aquire next frame: {}", e), // TODO: better error handling
            };
            let view = tex
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let tex_id = self
                .backend
                .resources
                .textures
                .insert(BackendTexture::Surface { window, view });
            let sampled = if samples > 1 {
                Some(texture_cache.get(
                    &mut self.backend,
                    TextureDescriptor {
                        dimensions: TextureDimensions::D2(surface.size()),
                        sample_count: samples,
                        format: surface.format(),
                        usage: TextureUsage::COLOR_ATTACHMENT,
                        ..Default::default()
                    },
                ))
            } else {
                None
            };
            self.tmp_surface_targets.insert(
                window,
                SurfaceTarget {
                    texture: tex_id,
                    sampled,
                },
            );
            self.tmp_surface_textures.push(tex);
        }
    }

    fn present_swapchain_images(&mut self) {
        let _ = tracing::trace_span!("Present").entered();
        // release texture views
        for (_window_id, target) in self.tmp_surface_targets.drain() {
            self.backend.resources.textures.remove(target.texture); // for swapchain images: only the view is destroyed
                                                                    // sampled texture is managed by texture-cache
        }

        for surface_texture in self.tmp_surface_textures.drain(..) {
            surface_texture.present();
        }
    }

    fn run_graph(&mut self, graph: &mut RenderGraph, res: &mut Resources) {
        if self.tmp_surface_textures.is_empty() {
            // skip
            return;
        }
        let _ = tracing::trace_span!("Run Graph").entered();
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let mut encoder = backend::CommandEncoder(encoder, self.resources());
        graph.run(res, &mut encoder, &self.tmp_surface_targets);
        self.queue.submit([encoder.finish()]);
    }

    fn update_cache<C: Cacheable>(&mut self, res: &mut Resources) {
        let mut cache = res.borrow_res_mut::<Cache<C>>().unwrap();
        cache.update(self.backend_mut());
    }

    fn run_graph_system(res: &mut Resources) {
        let mut renderer = res.remove::<Self>().unwrap();

        let mut windows = res.remove::<Windows>().unwrap();
        renderer.reconfigure_surfaces(&mut windows);
        res.insert_again(windows);

        renderer.aquire_swapchain_images(res);

        let mut graph = res.remove::<RenderGraph>().unwrap();
        renderer.run_graph(&mut graph, res);
        res.insert_again(graph);

        renderer.present_swapchain_images();

        // update caches
        renderer.update_cache::<TextureDescriptor>(res);

        res.insert_again(renderer);
    }

    pub fn install_into<'r>(self, res: &'r mut Resources, schedule: &mut Schedule) -> &'r mut Self {
        let is_first_init = res.get_id::<Self>().is_none();
        render::install_into(res, schedule);
        let resource_id = res.insert_unsend(self);
        if is_first_init {
            schedule.add_system(
                Self::run_graph_system
                    .with_label(RenderSystemLabel::RunGraph)
                    .after(RenderSystemLabel::UpdateGraph),
            );
        }

        res.get_mut_id(resource_id).unwrap()
    }
}

impl Module for WgpuRenderer {
    type Output = ();
    #[inline]
    fn install(self, res: &mut Resources, schedule: &mut Schedule) {
        self.install_into(res, schedule);
    }
}

/// # Unsafe
/// Raw Window Handle must be a valid object to create a surface
/// upon and must remain valid for the lifetime of the surface.
pub async fn install_wgpu_renderer_with_window<'w>(
    res: &'w mut Resources,
    schedule: &mut Schedule,
    window_id: WindowId,
) -> Result<&'w mut WgpuRenderer> {
    let renderer = {
        let windows = res.borrow_res::<Windows>().unwrap();
        let window = &windows[window_id];
        let raw_window_handles = res.borrow_res::<RawWindowHandles>().unwrap();
        let handle = raw_window_handles
            .get(window_id)
            .unwrap()
            .upgrade()
            .unwrap();
        WgpuRenderer::for_window(window_id, window, handle).await?
    };
    Ok(renderer.install_into(res, schedule))
}
