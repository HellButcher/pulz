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
use pulz_render::{draw::DrawPhases, graph::RenderGraph, RenderModule, RenderSystemPhase};
use pulz_window::{
    listener::{WindowSystemListener, WindowSystemListeners},
    RawWindow, Window, WindowId, Windows, WindowsMirror,
};
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

    #[error("Unable to create surface")]
    CreateSurfaceError(#[from] wgpu::CreateSurfaceError),

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

struct WgpuRendererFull {
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

impl WgpuRendererFull {
    async fn for_adapter(instance: wgpu::Instance, adapter: wgpu::Adapter) -> Result<Self> {
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

    fn run(&mut self, windows: &Windows, src_graph: &RenderGraph, _draw_phases: &DrawPhases) {
        self.reconfigure_surfaces(&windows);
        self.graph.update(&src_graph);
        self.aquire_swapchain_images();
        self.run_graph(&src_graph);
        self.present_swapchain_images();
    }
}

#[allow(clippy::large_enum_variant)]
enum WgpuRendererInner {
    #[cfg(not(target_arch = "wasm32"))]
    Early {
        instance: wgpu::Instance,
    },
    #[cfg(not(target_arch = "wasm32"))]
    Tmp,
    Full(WgpuRendererFull),
}

pub struct WgpuRenderer(WgpuRendererInner);

impl WgpuRenderer {
    pub async fn new() -> Result<Self> {
        let backends = backend_bits_from_env_or_default();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });
        if let Some(adapter) = wgpu::util::initialize_adapter_from_env(&instance, backends) {
            let renderer = WgpuRendererFull::for_adapter(instance, adapter).await?;
            return Ok(Self(WgpuRendererInner::Full(renderer)));
        }
        #[cfg(target_arch = "wasm32")]
        {
            let adapter = Self::default_adapter(&instance, None).await?;
            let renderer = Self::for_adapter(instance, adapter).await?;
            return Ok(Self(WgpuRendererInner::Full(renderer)));
        }
        #[cfg(not(target_arch = "wasm32"))]
        Ok(Self(WgpuRendererInner::Early { instance }))
    }

    async fn default_adapter(
        instance: &wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface>,
    ) -> Result<wgpu::Adapter> {
        let power_preference = wgpu::util::power_preference_from_env().unwrap_or_default();
        instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference,
                force_fallback_adapter: false,
                compatible_surface,
            })
            .await
            .ok_or(Error::NoAdapter)
    }

    fn init_window(
        &mut self,
        window_id: WindowId,
        window_descriptor: &Window,
        window_raw: &dyn RawWindow,
    ) -> Result<&mut WgpuRendererFull> {
        if let WgpuRendererInner::Full(renderer) = &mut self.0 {
            renderer.surfaces.remove(window_id); // replaces old surface
            let surface = Surface::create(&renderer.instance, window_descriptor, window_raw)?;
            renderer.surfaces.insert(window_id, surface);
        } else {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let WgpuRendererInner::Early { instance } = std::mem::replace(&mut self.0, WgpuRendererInner::Tmp) else {
                    panic!("unexpected state");
                };
                let surface = Surface::create(&instance, window_descriptor, window_raw)?;
                let mut renderer = pollster::block_on(async {
                    let adapter = Self::default_adapter(&instance, Some(&surface)).await?;
                    WgpuRendererFull::for_adapter(instance, adapter).await
                })?;
                renderer.surfaces.insert(window_id, surface);
                self.0 = WgpuRendererInner::Full(renderer);
            }
        }
        let WgpuRendererInner::Full(renderer) = &mut self.0 else {
            unreachable!()
        };
        Ok(renderer)
    }

    fn init(&mut self) -> Result<&mut WgpuRendererFull> {
        #[cfg(not(target_arch = "wasm32"))]
        if !matches!(self.0, WgpuRendererInner::Full { .. }) {
            let WgpuRendererInner::Early { instance } = std::mem::replace(&mut self.0, WgpuRendererInner::Tmp) else {
                panic!("unexpected state");
            };
            let renderer = pollster::block_on(async {
                let adapter = Self::default_adapter(&instance, None).await?;
                WgpuRendererFull::for_adapter(instance, adapter).await
            })?;
            self.0 = WgpuRendererInner::Full(renderer);
        }
        let WgpuRendererInner::Full(renderer) = &mut self.0 else {
            unreachable!()
        };
        Ok(renderer)
    }

    fn run(&mut self, windows: &mut Windows, src_graph: &RenderGraph, draw_phases: &DrawPhases) {
        if let WgpuRendererInner::Full(renderer) = &mut self.0 {
            renderer.run(windows, src_graph, draw_phases);
        } else {
            panic!("renderer uninitialized");
        }
    }
}

struct WgpuRendererInitWindowSystemListener(ResourceId<WgpuRenderer>);

impl WindowSystemListener for WgpuRendererInitWindowSystemListener {
    fn on_created(
        &self,
        res: &Resources,
        window_id: WindowId,
        window_desc: &Window,
        window_raw: &dyn RawWindow,
    ) {
        let Some(mut renderer) = res.borrow_res_mut_id(self.0) else { return };
        renderer
            .init_window(window_id, window_desc, window_raw)
            .unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn on_resumed(&self, res: &Resources) {
        let Some(mut renderer) = res.borrow_res_mut_id(self.0) else { return };
        renderer.init().unwrap();
    }
    fn on_closed(&self, res: &Resources, window_id: WindowId) {
        let Some(mut renderer) = res.borrow_res_mut_id(self.0) else { return };
        let WgpuRendererInner::Full(renderer) = &mut renderer.0 else { return };
        renderer.surfaces.remove(window_id);
    }
    fn on_suspended(&self, res: &Resources) {
        let Some(mut renderer) = res.borrow_res_mut_id(self.0) else { return };
        let WgpuRendererInner::Full(renderer) = &mut renderer.0 else { return };
        renderer.surfaces.clear();
    }
}

impl ModuleWithOutput for WgpuRenderer {
    type Output<'l> = &'l mut Self;

    fn install_modules(&self, res: &mut Resources) {
        res.install(RenderModule);
    }

    fn install_resources(self, res: &mut Resources) -> &mut Self {
        let listeners_id = res.init_unsend::<WindowSystemListeners>();
        let resource_id = res.insert_unsend(self);
        res.get_mut_id(listeners_id)
            .unwrap()
            .insert(WgpuRendererInitWindowSystemListener(resource_id));
        res.get_mut_id(resource_id).unwrap()
    }

    fn install_systems(schedule: &mut Schedule) {
        schedule
            .add_system(Self::run)
            .into_phase(RenderSystemPhase::Render);
    }
}
