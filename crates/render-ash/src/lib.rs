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

use std::{ffi::CStr, sync::Arc};

use ash::vk::{self, PipelineStageFlags};
use bitflags::bitflags;
use device::AshDevice;
use encoder::{AshCommandPool, SubmissionGroup};
use graph::AshRenderGraph;
use instance::AshInstance;
use pulz_ecs::prelude::*;
use pulz_render::{draw::DrawPhases, graph::RenderGraph, RenderModule, RenderSystemPhase};
use resources::AshResources;
use thiserror::Error;
use tracing::info;

mod convert;
mod debug_utils;
mod device;
mod drop_guard;
mod encoder;
mod graph;
mod instance;
mod resources;
mod shader;
mod swapchain;

use pulz_window::{
    RawWindow, RawWindowHandles, WindowDescriptor, WindowId, Windows, WindowsMirror,
};

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("library loading error")]
    LoadingError(#[from] ash::LoadingError),

    #[error("Vulkan driver does not support {0:?}")]
    ExtensionNotSupported(&'static CStr),

    #[error("The used Window-System is not supported")]
    UnsupportedWindowSystem,

    #[error("The window is not available, or it has no raw-window-handle")]
    WindowNotAvailable,

    #[error("No suitable GPU adapters found on the system!")]
    NoAdapter,

    #[error("Device doesn't have swapchain support")]
    NoSwapchainSupport,

    #[error("Swapchain supports {supported:?}, but {requested:?} was requested")]
    SwapchainUsageNotSupported {
        requested: vk::ImageUsageFlags,
        supported: vk::ImageUsageFlags,
    },

    #[error("The surface was lost")]
    SurfaceLost,

    #[error("A next swapchain image was already acquired without beeing presented.")]
    SwapchainImageAlreadyAcquired,

    #[error("Vulkan Error")]
    VkError(#[from] vk::Result),

    #[error("Allocation Error")]
    AllocationError(#[from] gpu_alloc::AllocationError),

    #[error("Serialization Error")]
    SerializationError(Box<dyn std::error::Error>),

    #[error("Deserialization Error")]
    DeserializationError(Box<dyn std::error::Error>),

    #[error("unknown renderer error")]
    Unknown,
}

#[derive(Debug)]
pub struct ErrorNoExtension(pub &'static CStr);

impl From<ErrorNoExtension> for Error {
    #[inline]
    fn from(e: ErrorNoExtension) -> Self {
        Self::ExtensionNotSupported(e.0)
    }
}

impl From<&vk::Result> for Error {
    #[inline]
    fn from(e: &vk::Result) -> Self {
        Self::VkError(*e)
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub struct AshRenderer {
    device: Arc<AshDevice>,
    res: AshResources,
    frames: Vec<Frame>,
    current_frame: usize,
    surfaces: WindowsMirror<swapchain::SurfaceSwapchain>,
    graph: AshRenderGraph,
}

impl Drop for AshRenderer {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
        }
        self.frames.clear();
        self.res.destroy_all(&self.device);
    }
}

bitflags!(
    /// Instance initialization flags.
    pub struct AshRendererFlags: u32 {
        /// Generate debug information in shaders and objects.
        const DEBUG = 1 << 0;
    }
);

struct Frame {
    // TODO: multi-threaded command recording: CommandPool per thread
    command_pool: AshCommandPool,
    finished_fence: vk::Fence, // signaled ad end of command-cueue, waited at beginning of frame
    finished_semaphore: vk::Semaphore, // semaphore used for presenting to the swapchain
    retired_swapchains: Vec<vk::SwapchainKHR>,
}

impl Frame {
    pub const NUM_FRAMES_IN_FLIGHT: usize = 2;
}

impl Frame {
    unsafe fn create(device: &Arc<AshDevice>) -> Result<Self> {
        let command_pool = device.new_command_pool(device.queues().graphics_family)?;
        let finished_fence = device.create(
            &vk::FenceCreateInfo::builder()
                .flags(vk::FenceCreateFlags::SIGNALED)
                .build(),
        )?;
        let finished_semaphore = device.create(&vk::SemaphoreCreateInfo::builder().build())?;
        Ok(Self {
            command_pool,
            finished_fence: finished_fence.take(),
            finished_semaphore: finished_semaphore.take(),
            retired_swapchains: Vec::new(),
        })
    }

    unsafe fn reset(&mut self, device: &AshDevice) -> Result<(), vk::Result> {
        if let Ok(ext_swapchain) = device.ext_swapchain() {
            for swapchain in self.retired_swapchains.drain(..) {
                ext_swapchain.destroy_swapchain(swapchain, None);
            }
        }

        self.command_pool.reset()?;

        Ok(())
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        unsafe {
            let device = self.command_pool.device();
            if let Ok(ext_swapchain) = device.ext_swapchain() {
                for swapchain in self.retired_swapchains.drain(..) {
                    ext_swapchain.destroy_swapchain(swapchain, None);
                }
            }
            if self.finished_fence != vk::Fence::null() {
                device.destroy_fence(self.finished_fence, None);
            }
            if self.finished_semaphore != vk::Semaphore::null() {
                device.destroy_semaphore(self.finished_semaphore, None);
            }
        }
    }
}

impl AshRenderer {
    pub fn new(flags: AshRendererFlags) -> Result<Self> {
        let instance = AshInstance::new(flags)?;
        let device = instance.new_device()?;
        Ok(Self::from_device(device))
    }

    pub fn for_window(
        flags: AshRendererFlags,
        window_id: WindowId,
        window_descriptor: &WindowDescriptor,
        // TODO: link lifetimes of HasRawWindowHandle and Surface!
        raw_window: &dyn RawWindow,
    ) -> Result<Self> {
        let instance = AshInstance::new(flags)?;
        let surface = instance.new_surface(raw_window)?;
        let device = instance.new_device_for_surface(&surface)?;
        let mut renderer = Self::from_device(device);

        let surface_swapchain = renderer.device.new_swapchain(
            surface,
            window_descriptor.size,
            //TODO: ergonomics
            if window_descriptor.vsync { 3 } else { 2 },
            if window_descriptor.vsync {
                vk::PresentModeKHR::MAILBOX
            } else {
                vk::PresentModeKHR::IMMEDIATE
            },
        )?;
        renderer.surfaces.insert(window_id, surface_swapchain);
        Ok(renderer)
    }

    fn from_device(device: Arc<AshDevice>) -> Self {
        let graph = AshRenderGraph::create(&device);
        Self {
            device,
            res: AshResources::new(),
            frames: Vec::with_capacity(Frame::NUM_FRAMES_IN_FLIGHT),
            current_frame: 0,
            surfaces: WindowsMirror::new(),
            graph,
        }
    }

    fn reconfigure_swapchains(&mut self, windows: &Windows) {
        let mut to_remove = Vec::new();
        for (window_id, surface_swapchain) in self.surfaces.iter_mut() {
            let Some(window) = windows.get(window_id) else {
                to_remove.push(window_id);
                continue;
            };
            //TODO: re-create also the surface, when SURFACE_LOST was returned in earlier calls.
            //TODO: better resize check (don't compare size, but use a 'dirty'-flag)
            if window.size != surface_swapchain.size() {
                info!(
                    "surface sized changed: {} => {}",
                    surface_swapchain.size(),
                    window.size
                );
                surface_swapchain
                    .configure_with(
                        window.size,
                        if window.vsync {
                            vk::PresentModeKHR::MAILBOX
                        } else {
                            vk::PresentModeKHR::IMMEDIATE
                        },
                    )
                    .unwrap();
            }
        }
        for window_id in to_remove {
            self.surfaces.remove(window_id);
        }
    }

    fn begin_frame(&mut self) -> Result<SubmissionGroup> {
        let _span = tracing::trace_span!("BeginFrame").entered();

        if self.frames.is_empty() {
            self.frames.reserve_exact(Frame::NUM_FRAMES_IN_FLIGHT);
            for _ in 0..Frame::NUM_FRAMES_IN_FLIGHT {
                self.frames.push(unsafe { Frame::create(&self.device)? });
            }
        }

        let frame = &mut self.frames[self.current_frame];
        unsafe {
            self.device
                .wait_for_fences(&[frame.finished_fence], true, !0)?;
        }

        // cleanup old frame
        unsafe {
            frame.reset(&self.device)?;
        }

        Ok(SubmissionGroup::new())
    }

    fn render_frame(
        &mut self,
        submission_group: &mut SubmissionGroup,
        src_graph: &RenderGraph,
        phases: &DrawPhases,
    ) -> Result<()> {
        let _span = tracing::trace_span!("RunGraph").entered();
        let frame = &mut self.frames[self.current_frame];

        self.graph
            .execute(src_graph, submission_group, &mut frame.command_pool, phases)?;

        Ok(())
    }

    // TODO: remove this!
    fn clear_unacquired_swapchain_images(
        &mut self,
        submission_group: &mut SubmissionGroup,
    ) -> Result<()> {
        let count = self.get_num_unacquired_swapchains();
        if count == 0 {
            return Ok(());
        }

        // TODO: try to clear with empty render-pass
        let _span = tracing::trace_span!("ClearImages").entered();
        let frame = &mut self.frames[self.current_frame];
        let mut encoder = frame.command_pool.encoder()?;
        encoder.begin_debug_label("ClearImages");

        let mut images = Vec::with_capacity(count);
        for (_window_id, surface_swapchain) in &mut self.surfaces {
            if !surface_swapchain.is_acquired() {
                let sem = encoder.request_semaphore()?;
                submission_group.wait(sem, PipelineStageFlags::TRANSFER);
                if let Some(img) = surface_swapchain.acquire_next_image(0, sem)? {
                    images.push((img.image, surface_swapchain.clear_value()));
                }
            }
        }

        let subrange = vk::ImageSubresourceRange::builder()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .layer_count(vk::REMAINING_ARRAY_LAYERS)
            .level_count(vk::REMAINING_MIP_LEVELS)
            .build();

        let barriers = images
            .iter()
            .map(|(image, _)| {
                vk::ImageMemoryBarrier::builder()
                    .src_access_mask(vk::AccessFlags::empty())
                    .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .subresource_range(subrange)
                    .image(*image)
                    .build()
            })
            .collect::<Vec<_>>();
        unsafe {
            encoder.pipeline_barrier(
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
                &[],
                &[],
                &barriers,
            );
        }

        for (image, clear_color) in &images {
            unsafe {
                encoder.clear_color_image(
                    *image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    clear_color,
                    &[subrange],
                )
            }
        }

        let barriers = images
            .iter()
            .map(|(image, _)| {
                vk::ImageMemoryBarrier::builder()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::empty())
                    .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                    .subresource_range(subrange)
                    .image(*image)
                    .build()
            })
            .collect::<Vec<_>>();
        unsafe {
            encoder.pipeline_barrier(
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                &[],
                &[],
                &barriers,
            );
        }

        encoder.submit(submission_group)?;
        Ok(())
    }

    fn end_frame(&mut self, mut submission_group: SubmissionGroup) -> Result<()> {
        let _span = tracing::trace_span!("EndFrame").entered();

        self.clear_unacquired_swapchain_images(&mut submission_group)?;

        let acquired_swapchains = self.get_num_acquired_swapchains();
        let frame = &self.frames[self.current_frame];

        unsafe {
            self.device
                .reset_fences(&[self.frames[self.current_frame].finished_fence])?;
        }

        submission_group.flush_queue();
        if acquired_swapchains == 0 {
            submission_group.submit(&self.device, frame.finished_fence)?;
        } else {
            submission_group
                .signal(frame.finished_semaphore)
                .submit(&self.device, frame.finished_fence)?;

            self.present_acquired_swapchain_images(&[frame.finished_semaphore])?;
        }

        let next_frame = self.current_frame;
        self.current_frame = next_frame + 1;
        if self.current_frame >= self.frames.len() {
            self.current_frame = 0;
        }
        Ok(())
    }

    fn run_render_system(
        mut renderer: ResMut<'_, Self>,
        mut windows: ResMut<'_, Windows>,
        src_graph: Res<'_, RenderGraph>,
        draw_phases: Res<'_, DrawPhases>,
    ) {
        renderer.reconfigure_swapchains(&mut windows);
        // TODO: maybe graph needs to consider updated swapchain format & dimensions?

        renderer.graph.update(&src_graph);

        let mut submission_group = renderer.begin_frame().unwrap();
        renderer
            .render_frame(&mut submission_group, &src_graph, &draw_phases)
            .unwrap();
        renderer.end_frame(submission_group).unwrap();
    }
}

impl ModuleWithOutput for AshRenderer {
    type Output<'l> = &'l mut Self;

    fn install_modules(&self, res: &mut Resources) {
        res.install(RenderModule);
    }

    fn install_resources(self, res: &mut Resources) -> &mut Self {
        let resource_id = res.insert(self);
        res.get_mut_id(resource_id).unwrap()
    }

    fn install_systems(schedule: &mut Schedule) {
        schedule
            .add_system(Self::run_render_system)
            .into_phase(RenderSystemPhase::Render);
    }
}

pub struct AshRendererBuilder {
    flags: AshRendererFlags,
    window: Option<WindowId>,
}

impl AshRendererBuilder {
    #[inline]
    pub const fn new() -> Self {
        Self {
            flags: AshRendererFlags::DEBUG,
            window: None,
        }
    }

    #[inline]
    pub const fn with_flags(mut self, flags: AshRendererFlags) -> Self {
        self.flags = flags;
        self
    }

    /// # Unsafe
    /// Raw Window Handle must be a valid object to create a surface
    /// upon and must remain valid for the lifetime of the surface.
    #[inline]
    pub unsafe fn with_window(mut self, window_id: WindowId) -> Self {
        self.window = Some(window_id);
        self
    }

    pub fn install(self, res: &mut Resources) -> Result<&mut AshRenderer> {
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
            AshRenderer::for_window(self.flags, window_id, descriptor, handle.as_ref())?
        } else {
            AshRenderer::new(self.flags)?
        };
        Ok(res.install(renderer))
    }
}
