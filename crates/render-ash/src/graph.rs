use ash::vk::{self};
use pulz_render::{
    draw::DrawPhases,
    graph::{
        pass::PipelineBindPoint, resources::GraphBackend, PassIndex, RenderGraph,
        RenderGraphAssignments,
    },
    pipeline::{GraphicsPass, GraphicsPassDescriptorWithTextures},
    texture::{TextureDimensions, TextureFormat},
};
use pulz_window::WindowsMirror;

use crate::{
    drop_guard::Guard,
    encoder::{AshCommandPool, SubmissionGroup},
    resources::AshResources,
    swapchain::SurfaceSwapchain,
    Result,
};

pub struct AshRenderGraph {
    topo: Vec<TopoGroup>,
    barriers: Vec<Barrier>,
    assignments: RenderGraphAssignments,
}

#[derive(Default)]
pub struct TopoGroup {
    render_passes: Vec<(PassIndex, vk::RenderPass, vk::Framebuffer)>, // pass-index
    compute_passes: Vec<usize>,                                       // sub-pass-index
    ray_tracing_passes: Vec<usize>,                                   // sub-pass-index
}

pub struct Barrier {
    image: Vec<vk::ImageMemoryBarrier>,
    buffer: Vec<vk::BufferMemoryBarrier>,
}

// implement Send+Sync manually, because vk::*MemoryBarrier have unused p_next pointers
// SAFETY: p_next pointers are not used
unsafe impl Send for Barrier {}
unsafe impl Sync for Barrier {}

impl AshRenderGraph {
    #[inline]
    pub const fn new() -> Self {
        Self {
            topo: Vec::new(),
            barriers: Vec::new(),
            assignments: RenderGraphAssignments::new(),
        }
    }

    pub fn update(
        &mut self,
        src_graph: &RenderGraph,
        res: &mut AshResources,
        surfaces: &WindowsMirror<SurfaceSwapchain>,
    ) -> Result<bool> {
        // TODO: update render-pass, if resource-formats changed
        // TODO: update framebuffer if render-pass or dimensions changed
        if self
            .assignments
            .update(src_graph, &mut AshGraphBackend { res, surfaces })
        {
            self.do_update(src_graph, res)?;
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
                            &self.assignments,
                            pass,
                        )
                        .unwrap();
                        let graphics_pass =
                            res.create::<GraphicsPass>(&pass_descr.graphics_pass)?;
                        let render_pass = res[graphics_pass];
                        let framebuf = Self::create_framebuffer(res, &pass_descr, render_pass)?;
                        topo_group
                            .render_passes
                            .push((pass.index(), render_pass, framebuf.take()));
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
        for (topo_index, topo) in self.topo.iter().enumerate() {
            // render-passes
            for &(pass_index, render_pass, fb) in &topo.render_passes {
                let pass = src_graph.get_pass(pass_index).unwrap();
                let has_multiple_subpass = pass.sub_pass_range().len() > 1;
                if has_multiple_subpass {
                    encoder.begin_debug_label(pass.name());
                }
                unsafe {
                    // TODO: caching of render-pass & framebuffer
                    // TODO: clear-values, ...
                    encoder.begin_render_pass(
                        &vk::RenderPassBeginInfo::builder()
                            .render_pass(render_pass)
                            .framebuffer(fb)
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

            if let Some(_barrier) = self.barriers.get(topo_index) {
                // TODO: add barriers
            }
        }

        encoder.submit(submission_group)?;

        Ok(())
    }
}

struct AshGraphBackend<'a> {
    res: &'a mut AshResources,
    surfaces: &'a WindowsMirror<SurfaceSwapchain>,
}

impl GraphBackend for AshGraphBackend<'_> {
    fn get_surface(
        &mut self,
        window_id: pulz_window::WindowId,
    ) -> (
        pulz_render::texture::Texture,
        TextureFormat,
        TextureDimensions,
    ) {
        let swapchain = self.surfaces.get(window_id).expect("swapchain not initialized");
        (swapchain.texture_id(), swapchain.texture_format(), TextureDimensions::D2(swapchain.size()))
    }
}
