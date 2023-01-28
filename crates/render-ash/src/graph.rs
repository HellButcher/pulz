use std::sync::Arc;

use ash::vk::{self};
use pulz_render::{
    draw::DrawPhases,
    graph::{pass::PipelineBindPoint, resources::ResourceAssignments, PassIndex, RenderGraph},
    pipeline::{GraphicsPass, GraphicsPassDescriptorWithTextures},
};

use crate::{
    device::AshDevice,
    drop_guard::Guard,
    encoder::{AshCommandPool, SubmissionGroup},
    resources::AshResources,
    Result,
};

pub struct AshRenderGraph {
    device: Arc<AshDevice>,
    hash: u64,
    topo: Vec<TopoGroup>,
    barriers: Vec<Barrier>,
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
    pub fn new(device: &Arc<AshDevice>) -> Self {
        Self {
            device: device.clone(),
            hash: 0,
            topo: Vec::new(),
            barriers: Vec::new(),
        }
    }

    fn reset(&mut self) {
        // TODO: caching of render-passes: don't destroy & recreate on every update!
        for topo in &mut self.topo {
            for (_, pass, fb) in topo.render_passes.drain(..) {
                unsafe {
                    self.device.destroy_framebuffer(fb, None);
                    self.device.destroy_render_pass(pass, None);
                }
            }
        }

        self.topo.clear();
        self.barriers.clear();
    }

    pub fn update(&mut self, src_graph: &RenderGraph, res: &mut AshResources) -> Result<bool> {
        // TODO: update render-pass, if resource-formats changed
        // TODO: update framebuffer if render-pass or dimensions changed
        if src_graph.was_updated() || self.hash != src_graph.hash() {
            self.force_update(src_graph, res)?;
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
        // TODO: caching?
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

    pub fn force_update(&mut self, src: &RenderGraph, res: &mut AshResources) -> Result<()> {
        self.reset();
        self.hash = src.hash();

        let num_topological_groups = src.get_num_topological_groups();
        self.topo
            .resize_with(num_topological_groups, Default::default);

        let texture_assignments = ResourceAssignments::new();
        for topo_index in 0..num_topological_groups {
            let topo_group = &mut self.topo[topo_index];
            for pass in src.get_topological_group(topo_index) {
                match pass.bind_point() {
                    PipelineBindPoint::Graphics => {
                        // TODO: no unwrap / error handling
                        let pass_descr = GraphicsPassDescriptorWithTextures::from_graph(
                            src,
                            pass,
                            &texture_assignments,
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

impl Drop for AshRenderGraph {
    fn drop(&mut self) {
        self.reset()
    }
}
