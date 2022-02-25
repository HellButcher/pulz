use crate::render_resource::GraphicsPipelineId;
use std::ops::Range;

pub trait DrawCommandEncoder {
    fn set_pipeline(&mut self, pipeline: GraphicsPipelineId);
    fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>);
    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>);
}

#[derive(Debug, Clone)]
pub enum DrawCommand {
    SetPipeline(GraphicsPipelineId),
    SetVertexBuffer,
    SetIndexBuffer,
    SetBindGroup,
    DrawIndexed {
        indices: Range<u32>,
        base_vertex: i32,
        instances: Range<u32>,
    },
    Draw {
        vertices: Range<u32>,
        instances: Range<u32>,
    },
}

/// A component that indicates how to draw an entity.
#[derive(Debug, Clone)]
pub struct Draw {
    pub draw_commands: Vec<DrawCommand>,
}

impl Default for Draw {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Draw {
    #[inline]
    pub const fn new() -> Self {
        Self {
            draw_commands: Vec::new(),
        }
    }

    pub fn clear_draw_commands(&mut self) {
        self.draw_commands.clear();
    }

    #[inline]
    pub fn draw_command(&mut self, render_command: DrawCommand) {
        self.draw_commands.push(render_command);
    }
}

impl DrawCommandEncoder for Draw {
    fn set_pipeline(&mut self, pipeline: GraphicsPipelineId) {
        self.draw_command(DrawCommand::SetPipeline(pipeline));
    }

    #[inline]
    fn draw_indexed(&mut self, indices: Range<u32>, base_vertex: i32, instances: Range<u32>) {
        self.draw_command(DrawCommand::DrawIndexed {
            base_vertex,
            indices,
            instances,
        });
    }

    #[inline]
    fn draw(&mut self, vertices: Range<u32>, instances: Range<u32>) {
        self.draw_command(DrawCommand::Draw {
            vertices,
            instances,
        });
    }
}
