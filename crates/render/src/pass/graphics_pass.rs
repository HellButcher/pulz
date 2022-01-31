use crate::render_resource::TextureId;

pub struct GraphicsPassDescriptor<'a> {
    pub label: Option<&'a str>,
    pub color_attachments: &'a [ColorAttachment],
    pub depth_stencil_attachment: Option<DepthStencilAttachment>,
}

pub struct ColorAttachment {
    pub texture: TextureId,
    pub resolve_target: Option<TextureId>,
    pub ops: Operations<crate::color::Srgba>,
}

pub struct DepthStencilAttachment {
    pub texture: TextureId,
    pub depth_ops: Option<Operations<f32>>,
    pub stencil_ops: Option<Operations<u32>>,
}

pub struct Operations<V> {
    pub load: LoadOp<V>,
    pub store: bool,
}

pub enum LoadOp<V> {
    /// Clear with a specified value.
    Clear(V),
    /// Load from memory.
    Load,
}
