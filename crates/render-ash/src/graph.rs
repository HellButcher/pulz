use ash::vk::{self, PipelineStageFlags};
use pulz_assets::Handle;
use pulz_render::{
    backend::PhysicalResourceResolver,
    buffer::Buffer,
    camera::RenderTarget,
    draw::DrawPhases,
    graph::{
        access::Access,
        pass::PipelineBindPoint,
        resources::{PhysicalResource, PhysicalResourceAccessTracker, PhysicalResources},
        PassDescription, PassIndex, RenderGraph,
    },
    math::USize2,
    pipeline::{ExtendedGraphicsPassDescriptor, GraphicsPass},
    texture::{Texture, TextureDescriptor, TextureDimensions, TextureFormat},
};
use pulz_window::WindowsMirror;
use tracing::debug;

use crate::{
    convert::{default_clear_value_for_format, VkInto},
    encoder::{AshCommandPool, SubmissionGroup},
    resources::AshResources,
    swapchain::AshSurfaceSwapchain,
    Result,
};

pub struct AshRenderGraph {
    physical_resources: PhysicalResources,
    physical_resource_access: PhysicalResourceAccessTracker,
    topo: Vec<TopoGroup>,
    barriers: Vec<Barrier>,
    hash: u64,
}

#[derive(Default, Debug)]
pub struct TopoGroup {
    render_passes: Vec<TopoRenderPass>, // pass-index
    compute_passes: Vec<usize>,         // sub-pass-index
    ray_tracing_passes: Vec<usize>,     // sub-pass-index
}

#[derive(Debug)]
struct TopoRenderPass {
    index: PassIndex,
    render_pass: vk::RenderPass,
    framebuffer: vk::Framebuffer,
    attachment_resource_indices: Vec<u16>,
    //framebuffers_cache: U64HashMap<vk::Framebuffer>,
    size: USize2,
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
                    .wait(sem, PipelineStageFlags::TOP_OF_PIPE); // TODO: better sync
                let aquired_texture = surface
                    .acquire_next_image(self.res, sem)
                    .expect("aquire failed")
                    .expect("aquire failed(2)");

                Some(PhysicalResource {
                    resource: aquired_texture.texture,
                    format: surface.texture_format(),
                    access: Access::PRESENT,
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
        access: Access,
    ) -> Option<Texture> {
        let t = self
            .res
            .create::<Texture>(&TextureDescriptor {
                format,
                dimensions,
                usage: access.as_texture_usage(),
                ..Default::default()
            })
            .ok()?;
        // TODO: destroy texture
        self.res.current_frame_garbage_mut().texture_handles.push(t);
        // TODO: reuse textures
        Some(t)
    }

    fn create_transient_buffer(&mut self, _size: usize, _access: Access) -> Option<Buffer> {
        // TODO: destroy buffers
        // TODO: reuse buffers
        todo!("implement create_transient_buffer")
    }
}

impl TopoRenderPass {
    fn from_graph(
        res: &mut AshResources,
        src: &RenderGraph,
        phys: &PhysicalResources,
        current_access: &mut PhysicalResourceAccessTracker,
        pass: &PassDescription,
    ) -> Result<Self> {
        let pass_descr =
            ExtendedGraphicsPassDescriptor::from_graph(src, phys, current_access, pass).unwrap();
        let graphics_pass = res.create::<GraphicsPass>(&pass_descr.graphics_pass)?;
        let render_pass = res[graphics_pass];
        Ok(Self {
            index: pass.index(),
            render_pass,
            framebuffer: vk::Framebuffer::null(),
            attachment_resource_indices: pass_descr.resource_indices,
            //framebuffers_cache: U64HashMap::default(),
            size: pass_descr.size,
        })
    }

    fn cleanup(&mut self, res: &mut AshResources) {
        if self.framebuffer != vk::Framebuffer::null() {
            res.current_frame_garbage_mut()
                .framebuffers
                .push(self.framebuffer);
            self.framebuffer = vk::Framebuffer::null();
        }
        /*
        for (_, fb) in self.framebuffers_cache.drain() {
            unsafe {
                res.device().destroy_framebuffer(fb, None);
            }
        }
        */
    }

    fn update_framebuffer(
        &mut self,
        res: &mut AshResources,
        phys: &PhysicalResources,
    ) -> Result<()> {
        self.cleanup(res);

        let mut image_views = Vec::with_capacity(self.attachment_resource_indices.len());
        for &i in &self.attachment_resource_indices {
            let physical_resource = phys.get_texture(i).expect("unassigned resource");
            let dim = physical_resource.size.subimage_extents();
            if dim != self.size {
                // TODO: handle size changed!
                // TODO: error handling
                panic!("all framebuffer textures need to have the same dimensions");
            }
            image_views.push(res[physical_resource.resource].1);
        }

        /*
        let mut hasher = DefaultHasher::new();
        image_views.hash(&mut hasher);
        let key = hasher.finish();
        if let Some(fb) = self.framebuffers_cache.get(&key).copied() {
            self.framebuffer = fb;
            return Ok(());
        }
        */

        let create_info = vk::FramebufferCreateInfo::builder()
            .render_pass(self.render_pass)
            // TODO
            .attachments(&image_views)
            .width(self.size.x)
            .height(self.size.y)
            .layers(1);
        let fb = unsafe { res.device().create(&create_info.build())? };
        self.framebuffer = fb.take();
        //self.framebuffers_cache.insert(key, self.framebuffer);
        Ok(())
    }
}

impl AshRenderGraph {
    #[inline]
    pub const fn new() -> Self {
        Self {
            physical_resources: PhysicalResources::new(),
            physical_resource_access: PhysicalResourceAccessTracker::new(),
            topo: Vec::new(),
            barriers: Vec::new(),
            hash: 0,
        }
    }

    pub fn cleanup(&mut self, res: &mut AshResources) {
        self.hash = 0;
        for mut topo in self.topo.drain(..) {
            for mut topo_render_pass in &mut topo.render_passes.drain(..) {
                topo_render_pass.cleanup(res);
            }
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
            self.do_update_framebuffers(res)?;
            Ok(true)
        } else {
            self.do_update_framebuffers(res)?;
            Ok(false)
        }
    }

    fn do_update(&mut self, src: &RenderGraph, res: &mut AshResources) -> Result<()> {
        self.cleanup(res);
        self.hash = src.hash();
        self.barriers.clear();

        let num_topological_groups = src.get_num_topological_groups();
        self.topo
            .resize_with(num_topological_groups, Default::default);

        self.physical_resource_access
            .reset(&self.physical_resources);
        // TODO: get initial layout of external textures

        for topo_index in 0..num_topological_groups {
            let topo_group = &mut self.topo[topo_index];
            for pass in src.get_topological_group(topo_index) {
                match pass.bind_point() {
                    PipelineBindPoint::Graphics => {
                        topo_group.render_passes.push(TopoRenderPass::from_graph(
                            res,
                            src,
                            &self.physical_resources,
                            &mut self.physical_resource_access,
                            pass,
                        )?);
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

    fn do_update_framebuffers(&mut self, res: &mut AshResources) -> Result<()> {
        for topo in &mut self.topo {
            for topo_render_pass in &mut topo.render_passes {
                topo_render_pass.update_framebuffer(res, &self.physical_resources)?;
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
        let mut clear_values = Vec::new();
        for (topo_index, topo) in self.topo.iter().enumerate() {
            // render-passes
            for topo_render_pass in &topo.render_passes {
                let pass = src_graph.get_pass(topo_render_pass.index).unwrap();
                let has_multiple_subpass = pass.sub_pass_range().len() > 1;
                if has_multiple_subpass {
                    encoder.begin_debug_label(pass.name());
                }
                clear_values.clear();
                for &i in &topo_render_pass.attachment_resource_indices {
                    let physical_resource = self.physical_resources.get_texture(i).unwrap();
                    let format: vk::Format = physical_resource.format.vk_into();
                    let clear_value = default_clear_value_for_format(format);
                    clear_values.push(clear_value);
                }
                unsafe {
                    // TODO: caching of framebuffer
                    // TODO: clear-values, render-area, ...
                    encoder.begin_render_pass(
                        &vk::RenderPassBeginInfo::builder()
                            .render_pass(topo_render_pass.render_pass)
                            .framebuffer(topo_render_pass.framebuffer)
                            .clear_values(&clear_values)
                            .render_area(
                                vk::Rect2D::builder()
                                    .offset(vk::Offset2D { x: 0, y: 0 })
                                    .extent(topo_render_pass.size.vk_into())
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
