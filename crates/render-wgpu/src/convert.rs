use std::ops::Deref;

use pulz_render::{
    buffer::{BufferDescriptor, BufferUsage},
    color::Srgba,
    math::USize3,
    pipeline::{Face, FrontFace, IndexFormat, PrimitiveTopology, VertexFormat},
    shader::{ShaderModule, ShaderModuleDescriptor, ShaderSource},
    texture::{
        ImageDataLayout, Texture, TextureDescriptor, TextureDimensions, TextureFormat, TextureUsage,
    },
};
use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ConversionError {
    #[error("the texture format {0:?} is not supported!")]
    UnsupportedTextureFormat(TextureFormat),

    #[error("the shader-module {0:?} is not available!")]
    ShaderModuleNotAvailable(ShaderModule),

    #[error("the texture {0:?} is not available!")]
    TextureNotAvailable(Texture),
}

pub type Result<T, E = ConversionError> = std::result::Result<T, E>;

#[inline]
pub fn convert_buffer_descriptor(val: &BufferDescriptor) -> wgpu::BufferDescriptor<'_> {
    wgpu::BufferDescriptor {
        label: None,
        size: val.size as u64,
        usage: convert_buffer_usage(val.usage),
        mapped_at_creation: true,
    }
}

#[inline]
fn convert_buffer_usage(val: BufferUsage) -> wgpu::BufferUsages {
    let mut result = wgpu::BufferUsages::empty();
    if val.contains(BufferUsage::TRANSFER_SRC) {
        result |= wgpu::BufferUsages::COPY_SRC;
    }
    if val.contains(BufferUsage::TRANSFER_DST) {
        result |= wgpu::BufferUsages::COPY_DST;
    }
    if val.contains(BufferUsage::INDEX) {
        result |= wgpu::BufferUsages::INDEX;
    }
    if val.contains(BufferUsage::UNIFORM) {
        result |= wgpu::BufferUsages::UNIFORM;
    }
    if val.contains(BufferUsage::STORAGE) {
        result |= wgpu::BufferUsages::STORAGE;
    }
    if val.contains(BufferUsage::INDIRECT) {
        result |= wgpu::BufferUsages::INDIRECT;
    }
    result
}

#[inline]
pub fn convert_texture_descriptor(val: &TextureDescriptor) -> Result<wgpu::TextureDescriptor<'_>> {
    Ok(wgpu::TextureDescriptor {
        label: None,
        size: convert_extents(val.dimensions.extents()),
        mip_level_count: val.mip_level_count,
        sample_count: val.sample_count,
        dimension: convert_texture_dimensions(&val.dimensions),
        format: convert_texture_format(val.format)?,
        usage: convert_texture_usages(val.usage),
    })
}

#[inline]
pub fn convert_texture_view_descriptor(val: &TextureDescriptor) -> wgpu::TextureViewDescriptor<'_> {
    wgpu::TextureViewDescriptor {
        dimension: Some(convert_texture_view_dimensions(&val.dimensions)),
        ..Default::default()
    }
}

#[inline]
pub fn convert_image_data_layout(image: &ImageDataLayout) -> wgpu::ImageDataLayout {
    wgpu::ImageDataLayout {
        offset: 0,
        bytes_per_row: std::num::NonZeroU32::new(image.bytes_per_row),
        rows_per_image: std::num::NonZeroU32::new(image.rows_per_image),
    }
}

#[inline]
fn convert_texture_view_dimensions(val: &TextureDimensions) -> wgpu::TextureViewDimension {
    match val {
        TextureDimensions::D1(_) => wgpu::TextureViewDimension::D1,
        TextureDimensions::D2 { .. } => wgpu::TextureViewDimension::D2,
        TextureDimensions::D2Array { .. } => wgpu::TextureViewDimension::D2Array,
        TextureDimensions::Cube { .. } => wgpu::TextureViewDimension::Cube,
        TextureDimensions::CubeArray { .. } => wgpu::TextureViewDimension::CubeArray,
        TextureDimensions::D3 { .. } => wgpu::TextureViewDimension::D3,
    }
}

#[inline]
fn convert_texture_dimensions(val: &TextureDimensions) -> wgpu::TextureDimension {
    match val {
        TextureDimensions::D1(_) => wgpu::TextureDimension::D1,
        TextureDimensions::D2 { .. }
        | TextureDimensions::D2Array { .. }
        | TextureDimensions::Cube { .. }
        | TextureDimensions::CubeArray { .. } => wgpu::TextureDimension::D2,
        TextureDimensions::D3 { .. } => wgpu::TextureDimension::D3,
    }
}

#[inline]
fn convert_extents(val: USize3) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: val.x,
        height: val.y,
        depth_or_array_layers: val.z,
    }
}

#[inline]
fn convert_texture_format(val: TextureFormat) -> Result<wgpu::TextureFormat> {
    Ok(match val {
        // 8-bit formats
        TextureFormat::R8Unorm => wgpu::TextureFormat::R8Unorm,
        TextureFormat::R8Snorm => wgpu::TextureFormat::R8Snorm,
        TextureFormat::R8Uint => wgpu::TextureFormat::R8Uint,
        TextureFormat::R8Sint => wgpu::TextureFormat::R8Sint,

        // 16-bit formats
        TextureFormat::R16Uint => wgpu::TextureFormat::R16Uint,
        TextureFormat::R16Sint => wgpu::TextureFormat::R16Sint,
        TextureFormat::R16Float => wgpu::TextureFormat::R16Float,
        TextureFormat::Rg8Unorm => wgpu::TextureFormat::Rg8Unorm,
        TextureFormat::Rg8Snorm => wgpu::TextureFormat::Rg8Snorm,
        TextureFormat::Rg8Uint => wgpu::TextureFormat::Rg8Uint,
        TextureFormat::Rg8Sint => wgpu::TextureFormat::Rg8Sint,

        // 32-bit formats
        TextureFormat::R32Uint => wgpu::TextureFormat::R32Uint,
        TextureFormat::R32Sint => wgpu::TextureFormat::R32Sint,
        TextureFormat::R32Float => wgpu::TextureFormat::R32Float,
        TextureFormat::Rg16Uint => wgpu::TextureFormat::Rg16Uint,
        TextureFormat::Rg16Sint => wgpu::TextureFormat::Rg16Sint,
        TextureFormat::Rg16Float => wgpu::TextureFormat::Rg16Float,
        TextureFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
        TextureFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Rgba8Snorm => wgpu::TextureFormat::Rgba8Snorm,
        TextureFormat::Rgba8Uint => wgpu::TextureFormat::Rgba8Uint,
        TextureFormat::Rgba8Sint => wgpu::TextureFormat::Rgba8Sint,
        TextureFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,

        // Packed 32-bit formats
        TextureFormat::Rgb9E5Ufloat => wgpu::TextureFormat::Rgb9e5Ufloat,
        TextureFormat::Rgb10A2Unorm => wgpu::TextureFormat::Rgb10a2Unorm,
        TextureFormat::Rg11B10Float => wgpu::TextureFormat::Rg11b10Float,

        // 64-bit formats
        TextureFormat::Rg32Uint => wgpu::TextureFormat::Rg32Uint,
        TextureFormat::Rg32Sint => wgpu::TextureFormat::Rg32Sint,
        TextureFormat::Rg32Float => wgpu::TextureFormat::Rg32Float,
        TextureFormat::Rgba16Uint => wgpu::TextureFormat::Rgba16Uint,
        TextureFormat::Rgba16Sint => wgpu::TextureFormat::Rgba16Sint,
        TextureFormat::Rgba16Float => wgpu::TextureFormat::Rgba16Float,

        // 128-bit formats
        TextureFormat::Rgba32Uint => wgpu::TextureFormat::Rgba32Uint,
        TextureFormat::Rgba32Sint => wgpu::TextureFormat::Rgba32Sint,
        TextureFormat::Rgba32Float => wgpu::TextureFormat::Rgba32Float,

        // Depth and stencil formats
        // TextureFormat::Stencil8 => wgpu::TextureFormat::Stencil8,
        // TextureFormat::Depth16Unorm => wgpu::TextureFormat::Depth16Unorm,
        TextureFormat::Depth24Plus => wgpu::TextureFormat::Depth24Plus,
        TextureFormat::Depth24PlusStencil8 => wgpu::TextureFormat::Depth24PlusStencil8,
        TextureFormat::Depth32Float => wgpu::TextureFormat::Depth32Float,

        _ => return Err(ConversionError::UnsupportedTextureFormat(val)),
    })
}

#[inline]
fn convert_vertex_format(val: VertexFormat) -> Result<wgpu::VertexFormat> {
    Ok(match val {
        VertexFormat::Uint8x2 => wgpu::VertexFormat::Uint8x2,
        VertexFormat::Uint8x4 => wgpu::VertexFormat::Uint8x4,
        VertexFormat::Sint8x2 => wgpu::VertexFormat::Sint8x2,
        VertexFormat::Sint8x4 => wgpu::VertexFormat::Sint8x4,
        VertexFormat::Unorm8x2 => wgpu::VertexFormat::Unorm8x2,
        VertexFormat::Unorm8x4 => wgpu::VertexFormat::Unorm8x4,
        VertexFormat::Snorm8x2 => wgpu::VertexFormat::Snorm8x2,
        VertexFormat::Snorm8x4 => wgpu::VertexFormat::Snorm8x4,
        VertexFormat::Uint16x2 => wgpu::VertexFormat::Uint16x2,
        VertexFormat::Uint16x4 => wgpu::VertexFormat::Uint16x4,
        VertexFormat::Sint16x2 => wgpu::VertexFormat::Sint16x2,
        VertexFormat::Sint16x4 => wgpu::VertexFormat::Sint16x4,
        VertexFormat::Unorm16x2 => wgpu::VertexFormat::Unorm16x2,
        VertexFormat::Unorm16x4 => wgpu::VertexFormat::Unorm16x4,
        VertexFormat::Snorm16x2 => wgpu::VertexFormat::Snorm16x2,
        VertexFormat::Snorm16x4 => wgpu::VertexFormat::Snorm16x4,
        VertexFormat::Uint32 => wgpu::VertexFormat::Uint32,
        VertexFormat::Uint32x2 => wgpu::VertexFormat::Uint32x2,
        VertexFormat::Uint32x3 => wgpu::VertexFormat::Uint32x3,
        VertexFormat::Uint32x4 => wgpu::VertexFormat::Uint32x4,
        VertexFormat::Sint32 => wgpu::VertexFormat::Sint32,
        VertexFormat::Sint32x2 => wgpu::VertexFormat::Sint32x2,
        VertexFormat::Sint32x3 => wgpu::VertexFormat::Sint32x3,
        VertexFormat::Sint32x4 => wgpu::VertexFormat::Sint32x4,
        //VertexFormat::Float16 => wgpu::VertexFormat::Float16,
        VertexFormat::Float16x2 => wgpu::VertexFormat::Float16x2,
        VertexFormat::Float16x4 => wgpu::VertexFormat::Float16x4,
        VertexFormat::Float32 => wgpu::VertexFormat::Float32,
        VertexFormat::Float32x2 => wgpu::VertexFormat::Float32x2,
        VertexFormat::Float32x3 => wgpu::VertexFormat::Float32x3,
        VertexFormat::Float32x4 => wgpu::VertexFormat::Float32x4,
        VertexFormat::Float64 => wgpu::VertexFormat::Float64,
        VertexFormat::Float64x2 => wgpu::VertexFormat::Float64x2,
        VertexFormat::Float64x3 => wgpu::VertexFormat::Float64x3,
        VertexFormat::Float64x4 => wgpu::VertexFormat::Float64x4,

        _ => panic!("unsupported vertex format: {:?}", val),
    })
}

#[inline]
fn convert_texture_usages(val: TextureUsage) -> wgpu::TextureUsages {
    let mut result = wgpu::TextureUsages::empty();
    if val.contains(TextureUsage::TRANSFER_SRC) {
        result |= wgpu::TextureUsages::COPY_SRC;
    }
    if val.contains(TextureUsage::TRANSFER_DST) {
        result |= wgpu::TextureUsages::COPY_DST;
    }
    if val.contains(TextureUsage::TEXTURE_BINDING) {
        result |= wgpu::TextureUsages::TEXTURE_BINDING;
    }
    if val.contains(TextureUsage::STORAGE_BINDING) {
        result |= wgpu::TextureUsages::STORAGE_BINDING;
    }
    if val.contains(TextureUsage::COLOR_ATTACHMENT)
        || val.contains(TextureUsage::DEPTH_STENCIL_ATTACHMENT)
    {
        result |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    }
    result
}

pub fn convert_shader_module_descriptor<'a>(
    val: &'a ShaderModuleDescriptor<'a>,
) -> wgpu::ShaderModuleDescriptor<'a> {
    let source = match &val.source {
        ShaderSource::Wgsl(s) => wgpu::ShaderSource::Wgsl(s.deref().into()),
        // ShaderSource::Glsl(s) => wgpu::ShaderSource::Glsl(s.deref().into()),
        // ShaderSource::SpirV(s) => wgpu::ShaderSource::SpirV(s.deref().into()),
        #[allow(unreachable_patterns)]
        _ => panic!("unsupported shader type in shader {:?}", val.label),
    };
    wgpu::ShaderModuleDescriptor {
        label: val.label,
        source,
    }
}

#[inline]
fn convert_primitive_topology(val: PrimitiveTopology) -> wgpu::PrimitiveTopology {
    match val {
        PrimitiveTopology::PointList => wgpu::PrimitiveTopology::PointList,
        PrimitiveTopology::LineList => wgpu::PrimitiveTopology::LineList,
        PrimitiveTopology::LineStrip => wgpu::PrimitiveTopology::LineStrip,
        PrimitiveTopology::TriangleList => wgpu::PrimitiveTopology::TriangleList,
        PrimitiveTopology::TriangleStrip => wgpu::PrimitiveTopology::TriangleStrip,
    }
}

#[inline]
fn convert_front_face(val: FrontFace) -> wgpu::FrontFace {
    match val {
        FrontFace::CounterClockwise => wgpu::FrontFace::Ccw,
        FrontFace::Clockwise => wgpu::FrontFace::Cw,
    }
}

#[inline]
fn convert_face(val: Face) -> wgpu::Face {
    match val {
        Face::Front => wgpu::Face::Front,
        Face::Back => wgpu::Face::Back,
    }
}

#[inline]
fn convert_index_format(val: IndexFormat) -> wgpu::IndexFormat {
    match val {
        IndexFormat::Uint16 => wgpu::IndexFormat::Uint16,
        IndexFormat::Uint32 => wgpu::IndexFormat::Uint32,
    }
}

#[inline]
fn convert_color(color: Srgba) -> wgpu::Color {
    wgpu::Color {
        r: color.red as f64,
        g: color.green as f64,
        b: color.blue as f64,
        a: color.alpha as f64,
    }
}
