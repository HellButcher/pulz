use ash::vk::{self, PipelineStageFlags};
use pulz_assets::Handle;
use pulz_render::{
    backend::PhysicalResourceResolver,
    buffer::{Buffer, BufferUsage},
    camera::RenderTarget,
    draw::DrawPhases,
    graph::{
        pass::PipelineBindPoint,
        resources::{PhysicalResource, PhysicalResources},
        PassIndex, RenderGraph,
    },
    math::USize2,
    pipeline::{GraphicsPass, GraphicsPassDescriptorWithTextures},
    texture::{Texture, TextureDescriptor, TextureDimensions, TextureFormat, TextureUsage},
};
use pulz_window::WindowsMirror;
use tracing::debug;

use crate::{
    convert::VkInto,
    drop_guard::Guard,
    encoder::{AshCommandPool, SubmissionGroup},
    resources::AshResources,
    swapchain::AshSurfaceSwapchain,
    Result,
};

pub struct AshRenderGraph {
    physical_resources: PhysicalResources,
    topo: Vec<TopoGroup>,
    barriers: Vec<Barrier>,
    hash: u64,
    physical_resources_hash: u64,
}

#[derive(Default, Debug)]
pub struct TopoGroup {
    render_passes: Vec<(PassIndex, vk::RenderPass, vk::Framebuffer, USize2)>, // pass-index
    compute_passes: Vec<usize>,                                               // sub-pass-index
    ray_tracing_passes: Vec<usize>,                                           // sub-pass-index
}

#[derive(Debug)]
pub struct Barrier {
    image: Vec<vk::ImageMemoryBarrier>,
    buffer: Vec<vk::BufferMemoryBarrier>,
}

// implement Send+Sync manually, because vk::*MemoryBarrier have unused p_next pointers
// SAFETY: p_next pointers are not used
unsafe impl Send for Barrier {}
unsafe impl Sync for Barrier {}

struct AshPhysicalResourceResolver<'a> {
    submission_group: &'a mut SubmissionGroup,
    res: &'a mut AshResources,
    command_pool: &'a mut AshCommandPool,
    surfaces: &'a mut WindowsMirror<AshSurfaceSwapchain>,
}

impl PhysicalResourceResolver for AshPhysicalResourceResolver<'_> {
    fn resolve_render_target(
        &mut self,
        render_target: &RenderTarget,
    ) -> Option<PhysicalResource<Texture>> {
        match render_target {
            RenderTarget::Image(_i) => todo!("implement resolve_render_target (image)"),
            RenderTarget::Window(w) => {
                let surface = self.surfaces.get_mut(*w).expect("resolve window");
                assert!(!surface.is_acquired());

                let sem = self
                    .command_pool
                    .request_semaphore()
                    .expect("request semaphore");
                self.submission_group
                    .wait(sem, PipelineStageFlags::TRANSFER);
                let aquired_texture = surface
                    .acquire_next_image(self.res, 0, sem)
                    .expect("aquire failed")
                    .expect("aquire failed(2)");

                Some(PhysicalResource {
                    resource: aquired_texture.texture,
                    format: surface.texture_format(),
                    // TODO: usage
                    usage: TextureUsage::ALL_ATTACHMENTS,
                    size: TextureDimensions::D2(surface.size()),
                })
            }
        }
    }

    fn resolve_buffer(&mut self, _handle: &Handle<Buffer>) -> Option<PhysicalResource<Buffer>> {
        todo!("implement resolve_buffer")
    }

    fn create_transient_texture(
        &mut self,
        format: TextureFormat,
        dimensions: TextureDimensions,
        usage: TextureUsage,
    ) -> Option<Texture> {
        let t = self
            .res
            .create::<Texture>(&TextureDescriptor {
                format,
                dimensions,
                usage,
                ..Default::default()
            })
            .ok()?;
        // TODO: destroy texture
        // TODO: reuse textures
        Some(t)
    }

    fn create_transient_buffer(&mut self, _size: usize, _usage: BufferUsage) -> Option<Buffer> {
        // TODO: reuse textures
        todo!("implement create_transient_buffer")
    }
}

impl AshRenderGraph {
    #[inline]
    pub const fn new() -> Self {
        Self {
            physical_resources: PhysicalResources::new(),
            topo: Vec::new(),
            barriers: Vec::new(),
            hash: 0,
            physical_resources_hash: 0,
        }
    }

    pub fn update(
        &mut self,
        src_graph: &RenderGraph,
        submission_group: &mut SubmissionGroup,
        res: &mut AshResources,
        command_pool: &mut AshCommandPool,
        surfaces: &mut WindowsMirror<AshSurfaceSwapchain>,
    ) -> Result<bool> {
        let mut resolver = AshPhysicalResourceResolver {
            submission_group,
            res,
            command_pool,
            surfaces,
        };
        // TODO: update render-pass, if resource-formats changed
        // TODO: update framebuffer if render-pass or dimensions changed
        let formats_changed = self
            .physical_resources
            .assign_physical(src_graph, &mut resolver);
        if src_graph.was_updated() || src_graph.hash() != self.hash || formats_changed {
            self.do_update(src_graph, res)?;
            debug!(
                "graph updated: topo={:?}, barriers={:?}, formats_changed={:?}",
                self.topo, self.barriers, formats_changed,
            );
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn create_framebuffer<'d>(
        res: &'d mut AshResources,
        descr: &GraphicsPassDescriptorWithTextures,
        render_pass: vk::RenderPass,
    ) -> Result<Guard<'d, vk::Framebuffer>> {
        // TODO: caching
        // TODO: destroy (if not caching)
        let image_views: Vec<_> = descr.textures.iter().map(|&t| res[t].1).collect();
        let create_info = vk::FramebufferCreateInfo::builder()
            .render_pass(render_pass)
            // TODO
            .attachments(&image_views)
            .width(descr.size.x)
            .height(descr.size.y)
            .layers(1);
        unsafe {
            let fb = res.device().create(&create_info.build())?;
            Ok(fb)
        }
    }

    fn do_update(&mut self, src: &RenderGraph, res: &mut AshResources) -> Result<()> {
        self.hash = src.hash();
        self.topo.clear();
        self.barriers.clear();

        let num_topological_groups = src.get_num_topological_groups();
        self.topo
            .resize_with(num_topological_groups, Default::default);

        for topo_index in 0..num_topological_groups {
            let topo_group = &mut self.topo[topo_index];
            for pass in src.get_topological_group(topo_index) {
                match pass.bind_point() {
                    PipelineBindPoint::Graphics => {
                        // TODO: no unwrap / error handling
                        let pass_descr = GraphicsPassDescriptorWithTextures::from_graph(
                            src,
                            &self.physical_resources,
                            pass,
                        )
                        .unwrap();
                        let graphics_pass =
                            res.create::<GraphicsPass>(&pass_descr.graphics_pass)?;
                        let render_pass = res[graphics_pass];
                        let framebuf = Self::create_framebuffer(res, &pass_descr, render_pass)?;
                        topo_group.render_passes.push((
                            pass.index(),
                            render_pass,
                            framebuf.take(),
                            pass_descr.size,
                        ));
                    }
                    PipelineBindPoint::Compute => {
                        let range = pass.sub_pass_range();
                        assert_eq!(range.start + 1, range.end);
                        topo_group.compute_passes.push(range.start);
                    }
                    PipelineBindPoint::RayTracing => {
                        let range = pass.sub_pass_range();
                        assert_eq!(range.start + 1, range.end);
                        topo_group.ray_tracing_passes.push(range.start);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn execute(
        &self,
        src_graph: &RenderGraph,
        submission_group: &mut SubmissionGroup,
        command_pool: &mut AshCommandPool,
        draw_phases: &DrawPhases,
    ) -> Result<()> {
        let mut encoder = command_pool.encoder()?;
        //let mut clear_values = Vec::new();
        for (topo_index, topo) in self.topo.iter().enumerate() {
            // render-passes
            for &(pass_index, render_pass, fb, size) in &topo.render_passes {
                let pass = src_graph.get_pass(pass_index).unwrap();
                let has_multiple_subpass = pass.sub_pass_range().len() > 1;
                if has_multiple_subpass {
                    encoder.begin_debug_label(pass.name());
                }
                unsafe {
                    // TODO: caching of render-pass & framebuffer
                    // TODO: clear-values, render-area, ...
                    encoder.begin_render_pass(
                        &vk::RenderPassBeginInfo::builder()
                            .render_pass(render_pass)
                            .framebuffer(fb)
                            //.clear_values(&clear_values)
                            .render_area(
                                vk::Rect2D::builder()
                                    .offset(vk::Offset2D { x: 0, y: 0 })
                                    .extent(size.vk_into())
                                    .build(),
                            )
                            .build(),
                        vk::SubpassContents::INLINE,
                    );
                    let mut first = true;
                    for subpass_index in pass.sub_pass_range() {
                        if first {
                            first = false;
                        } else {
                            encoder.next_subpass(vk::SubpassContents::INLINE);
                        }
                        let subpass = src_graph.get_subpass(subpass_index).unwrap();
                        encoder.begin_debug_label(subpass.name());
                        src_graph.execute_sub_pass(subpass_index, &mut encoder, draw_phases);
                        encoder.end_debug_label();
                    }
                    encoder.end_render_pass();
                }
                if has_multiple_subpass {
                    encoder.end_debug_label();
                }
            }
            // TODO: compute passes, raytracing-passes

            if let Some(barrier) = self.barriers.get(topo_index) {
                // TODO: add barriers
                todo!("implement barriers {barrier:?}");
            }
        }

        encoder.submit(submission_group)?;

        Ok(())
    }
}
