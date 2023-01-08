use std::sync::Arc;

use ash::vk::{self, SubpassDependency};
use pulz_render::{
    buffer::BufferUsage,
    draw::DrawPhases,
    graph::{
        pass::PipelineBindPoint, resources::ResourceDep, PassDescription, PassGroupDescription,
        RenderGraph,
    },
    texture::TextureUsage,
};

use crate::{
    convert::{
        into_buffer_usage_read_access, into_buffer_usage_write_access,
        into_texture_usage_read_access, into_texture_usage_write_access, VkInto,
    },
    device::AshDevice,
    encoder::{AshCommandPool, SubmissionGroup},
    drop_guard::Guard,
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
    render_passes: Vec<(usize, vk::RenderPass, vk::Framebuffer)>, // group-index
    compute_passes: Vec<usize>,                                   // pass-index
    ray_tracing_passes: Vec<usize>,                               // pass-index
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
    pub fn create(device: &Arc<AshDevice>) -> Self {
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

    pub fn update(&mut self, src_graph: &RenderGraph) {
        if src_graph.was_updated() || self.hash != src_graph.hash() {
            self.force_update(src_graph);
        }
    }

    fn create_render_pass<'d>(
        device: &'d AshDevice,
        src_graph: &RenderGraph,
        group: &PassGroupDescription,
    ) -> Result<Guard<'d, vk::RenderPass>> {
        let range = group.range();
        let attachments = Vec::new();
        let mut subpasses = Vec::with_capacity(range.len());
        let mut dependencies = Vec::new();

        fn map_pass_index_to_subpass_index(group: &PassGroupDescription, pass_index: usize) -> u32 {
            let range = group.range();
            if range.contains(&pass_index) {
                (pass_index - range.start) as u32
            } else {
                vk::SUBPASS_EXTERNAL
            }
        }

        fn get_subpass_dep<'l>(
            deps: &'l mut Vec<SubpassDependency>,
            group: &PassGroupDescription,
            src_pass: usize,
            dst_pass: usize,
        ) -> &'l mut SubpassDependency {
            let src = map_pass_index_to_subpass_index(group, src_pass);
            let dst = map_pass_index_to_subpass_index(group, dst_pass);
            match deps.binary_search_by_key(&(src, dst), |d| (d.src_subpass, d.dst_subpass)) {
                Ok(i) => &mut deps[i],
                Err(i) => {
                    deps.insert(
                        i,
                        SubpassDependency::builder()
                            .src_subpass(src)
                            .dst_subpass(dst)
                            // use BY-REGION by default
                            .dependency_flags(vk::DependencyFlags::BY_REGION)
                            .build(),
                    );
                    &mut deps[i]
                }
            }
        }

        fn get_texture_access(dep: &ResourceDep<TextureUsage>, dst: bool) -> vk::AccessFlags {
            let reads = dep.src_pass() != !0; // resource was written in a different pass
            let writes = dep.write_access();
            let usage = dep.usage();
            let mut result = vk::AccessFlags::empty();
            if reads && (dst || !writes) {
                result |= into_texture_usage_read_access(usage);
            }
            if writes {
                result |= into_texture_usage_write_access(usage);
            }
            result
        }

        fn get_buffer_access(dep: &ResourceDep<BufferUsage>, _dst: bool) -> vk::AccessFlags {
            let reads = dep.src_pass() != !0; // resource was written in a different pass
            let writes = dep.write_access();
            let usage = dep.usage();
            let mut result = vk::AccessFlags::empty();
            if reads {
                result |= into_buffer_usage_read_access(usage);
            }
            if writes {
                result |= into_buffer_usage_write_access(usage);
            }
            result
        }

        fn add_subpass_deps(
            src_graph: &RenderGraph,
            deps: &mut Vec<SubpassDependency>,
            group: &PassGroupDescription,
            pass: &PassDescription,
        ) {
            let dst_pass = pass.index();
            for tex_dep in pass.textures().deps() {
                if tex_dep.src_pass() != !0 {
                    let usage = tex_dep.usage();
                    let dst_dep = get_subpass_dep(deps, group, tex_dep.src_pass(), dst_pass);
                    if !usage.contains(TextureUsage::BY_REGION) {
                        // remove by-region
                        dst_dep.dependency_flags &= !vk::DependencyFlags::BY_REGION;
                    }
                    dst_dep.dst_stage_mask |= tex_dep.stages().vk_into();
                    dst_dep.dst_access_mask |= get_texture_access(tex_dep, true);

                    let tex_src = src_graph
                        .get_pass(tex_dep.src_pass())
                        .unwrap()
                        .textures()
                        .find_by_resource_index(tex_dep.resource_index())
                        .unwrap();
                    dst_dep.src_stage_mask |= tex_src.stages().vk_into();
                    dst_dep.src_access_mask |= get_texture_access(tex_src, false);
                }
            }
            for buf_dep in pass.buffers().deps() {
                if buf_dep.src_pass() != !0 {
                    let dst_dep = get_subpass_dep(deps, group, buf_dep.src_pass(), dst_pass);
                    dst_dep.dst_stage_mask |= buf_dep.stages().vk_into();
                    dst_dep.dst_access_mask |= get_buffer_access(buf_dep, true);

                    let buf_src = src_graph
                        .get_pass(buf_dep.src_pass())
                        .unwrap()
                        .buffers()
                        .find_by_resource_index(buf_dep.resource_index())
                        .unwrap();
                    dst_dep.src_stage_mask |= buf_src.stages().vk_into();
                    dst_dep.src_access_mask |= get_buffer_access(buf_src, false);
                }
            }
        }

        for pass_index in range {
            let pass = src_graph.get_pass(pass_index).unwrap();
            subpasses.push(
                vk::SubpassDescription::builder()
                    .pipeline_bind_point(pass.bind_point().vk_into())
                    // TODO: attachments
                    .build(),
            );
            add_subpass_deps(src_graph, &mut dependencies, group, pass);
        }

        let create_info = vk::RenderPassCreateInfo::builder()
            .attachments(&attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies);

        unsafe {
            let pass = device.create(&create_info.build())?;
            if let Ok(debug_utils) = device.instance().ext_debug_utils() {
                debug_utils.object_name(device.handle(), pass.raw(), group.name());
            }
            Ok(pass)
        }
    }

    fn create_framebuffer<'d>(
        device: &'d AshDevice,
        _src_graph: &RenderGraph,
        group: &PassGroupDescription,
        render_pass: vk::RenderPass,
    ) -> Result<Guard<'d, vk::Framebuffer>> {
        let create_info = vk::FramebufferCreateInfo::builder()
            .render_pass(render_pass)
            // TODO
            // .attachments()
            .width(800)
            .height(600)
            .layers(1);

        unsafe {
            let fb = device.create(&create_info.build())?;
            if let Ok(debug_utils) = device.instance().ext_debug_utils() {
                debug_utils.object_name(device.handle(), fb.raw(), group.name());
            }
            Ok(fb)
        }
    }

    pub fn force_update(&mut self, src: &RenderGraph) -> Result<()> {
        self.reset();
        self.hash = src.hash();

        let num_topological_groups = src.get_num_topological_groups();
        self.topo
            .resize_with(num_topological_groups, Default::default);

        for topo_index in 0..num_topological_groups {
            let topo = &mut self.topo[topo_index];
            for group in src.get_topological_group(topo_index) {
                match group.bind_point() {
                    PipelineBindPoint::Graphics => {
                        let pass = Self::create_render_pass(&self.device, src, group)?;
                        let fb = Self::create_framebuffer(&self.device, src, group, pass.raw())?;
                        topo.render_passes
                            .push((group.group_index(), pass.take(), fb.take()));
                    }
                    PipelineBindPoint::Compute => {
                        let range = group.range();
                        assert_eq!(range.start + 1, range.end);
                        topo.compute_passes.push(range.start);
                    }
                    PipelineBindPoint::RayTracing => {
                        let range = group.range();
                        assert_eq!(range.start + 1, range.end);
                        topo.ray_tracing_passes.push(range.start);
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
            for &(group_index, render_pass, fb) in &topo.render_passes {
                let group = src_graph.get_pass_group(group_index).unwrap();
                let multi_pass = group.range().len() > 1;
                if multi_pass {
                    encoder.begin_debug_label(group.name());
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
                    for pass_index in group.range() {
                        if first {
                            first = false;
                        } else {
                            encoder.next_subpass(vk::SubpassContents::INLINE);
                        }
                        let pass = src_graph.get_pass(pass_index).unwrap();
                        encoder.begin_debug_label(pass.name());
                        src_graph.execute_pass(pass.index(), &mut encoder, draw_phases);
                        encoder.end_debug_label();
                    }
                    encoder.end_render_pass();
                }
                if multi_pass {
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
