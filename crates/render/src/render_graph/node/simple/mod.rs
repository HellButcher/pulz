use std::borrow::Cow;

use palette::Srgba;

use crate::{
    backend::RenderBackend,
    include_wgsl,
    pass::{ColorAttachment, GraphicsPassDescriptor, LoadOp, Operations},
    pipeline::{
        FragmentState, GraphicsPipelineDescriptor, PipelineLayoutDescriptor, PrimitiveState,
        VertexState,
    },
    render_graph::slot::SlotAccess,
    render_resource::GraphicsPipelineId,
    texture::TextureFormat,
};

use crate::render_graph::{
    context::RenderGraphContext,
    node::Node,
    slot::{SlotDescriptor, SlotType},
    GraphError,
};

pub struct SimpleTriangleRenderNode {
    graphics_pipeline: GraphicsPipelineId,
}

impl SimpleTriangleRenderNode {
    pub fn new(backend: &mut impl RenderBackend) -> Self {
        let module = backend.create_shader_module(include_wgsl!("simple.wgsl"));
        let pipeline_layout = backend.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("SimpleRenderNode"),
            bind_group_layouts: &[],
        });

        // TODO: get format & samples of surface
        let swapchain_format = TextureFormat::Bgra8Srgb;
        let samples = 1;

        let graphics_pipeline = backend.create_graphics_pipeline(&GraphicsPipelineDescriptor {
            label: None,
            layout: Some(pipeline_layout),
            vertex: VertexState {
                module,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(FragmentState {
                module,
                entry_point: "fs_main",
                targets: &[swapchain_format.into()],
            }),
            samples,
            primitive: PrimitiveState::default(),
            depth_stencil: None,
        });

        Self { graphics_pipeline }
    }
}

impl Node for SimpleTriangleRenderNode {
    fn slots(&self) -> Cow<'static, [SlotDescriptor]> {
        Cow::Borrowed(&[SlotDescriptor {
            name: Cow::Borrowed("color"),
            access: SlotAccess::Both,
            slot_type: SlotType::Texture,
            optional: false,
        }])
    }

    fn run<'c>(&self, graph: &'c mut RenderGraphContext<'_>) -> Result<(), GraphError<'c>> {
        let color_texture = graph.input_texture(0)?;
        let color_sampled_texture = None; //graph.input_texture(1).ok();

        graph.graphics_pass(
            &GraphicsPassDescriptor {
                label: None,
                color_attachments: &[ColorAttachment {
                    texture: if let Some(sampled) = color_sampled_texture {
                        sampled
                    } else {
                        color_texture
                    },
                    resolve_target: if color_sampled_texture.is_some() {
                        Some(color_texture)
                    } else {
                        None
                    },
                    ops: Operations {
                        load: LoadOp::Clear(Srgba::new(0.0, 1.0, 0.0, 1.0)),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            },
            &mut |pass| {
                pass.set_pipeline(self.graphics_pipeline);
                pass.draw(0..3, 0..1);
            },
        );

        Ok(())
    }
}
