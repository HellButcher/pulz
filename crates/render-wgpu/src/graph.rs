use pulz_render::graph::RenderGraph;

use crate::backend::WgpuCommandEncoder;

pub struct WgpuRenderGraph;

impl WgpuRenderGraph {
    pub fn new() -> Self {
        Self
    }

    pub fn update(&mut self, _src_graph: &RenderGraph) {
        todo!()
    }

    pub fn execute(
        &self,
        _src_graph: &RenderGraph,
        encoder: wgpu::CommandEncoder,
    ) -> [wgpu::CommandBuffer; 1] {
        let _encoder = WgpuCommandEncoder(encoder);
        todo!();
        // TODO
        [encoder.finish()]
    }
}
