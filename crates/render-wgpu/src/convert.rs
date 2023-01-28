use std::ops::Deref;

use pulz_render::{
    buffer::{BufferDescriptor, BufferUsage},
    color::Srgba,
    math::USize3,
    pipeline::{
        BindGroupLayoutDescriptor, BindGroupLayoutEntry, BlendComponent, BlendFactor,
        BlendOperation, BlendState, ColorTargetState, ColorWrite, CompareFunction,
        ComputePipelineDescriptor, DepthStencilState, Face, FragmentState, FrontFace,
        GraphicsPipelineDescriptor, IndexFormat, PipelineLayout, PipelineLayoutDescriptor,
        PrimitiveState, PrimitiveTopology, StencilFaceState, StencilOperation, VertexAttribute,
        VertexFormat, VertexState,
    },
    shader::{ShaderModule, ShaderModuleDescriptor, ShaderSource},
    texture::{
        ImageDataLayout, Texture, TextureDescriptor, TextureDimensions, TextureFormat, TextureUsage,
    },
};
use thiserror::Error;

use crate::resources::WgpuResources;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ConversionError {
    #[error("the texture format {0:?} is not supported!")]
    UnsupportedTextureFormat(TextureFormat),

    #[error("the shader-module {0:?} is not available!")]
    ShaderModuleNotAvailable(ShaderModule),

    #[error("the texture {0:?} is not available!")]
    TextureNotAvailable(Texture),

    #[error("the pipeline layout {0:?} is not available!")]
    PipelineLayoutNotAvailable(PipelineLayout),
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
        sample_count: val.sample_count as u32,
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
    if val.contains(TextureUsage::SAMPLED) {
        result |= wgpu::TextureUsages::TEXTURE_BINDING;
    }
    if val.contains(TextureUsage::STORAGE) {
        result |= wgpu::TextureUsages::STORAGE_BINDING;
    }
    if val.contains(TextureUsage::COLOR_ATTACHMENT)
        || val.contains(TextureUsage::DEPTH_STENCIL_ATTACHMENT)
    {
        result |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    }
    result
}

fn convert_bind_group_layout_entry(_val: BindGroupLayoutEntry) -> wgpu::BindGroupLayoutEntry {
    todo!() // TODO
}

fn convert_vertex_attribute(index: usize, attr: VertexAttribute) -> Result<wgpu::VertexAttribute> {
    Ok(wgpu::VertexAttribute {
        format: convert_vertex_format(attr.format)?,
        offset: attr.offset as u64,
        shader_location: index as u32,
    })
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

pub fn convert_bind_group_layout_descriptor<'l>(
    desc: &BindGroupLayoutDescriptor<'l>,
    entries_tmp: &'l mut Vec<wgpu::BindGroupLayoutEntry>,
) -> wgpu::BindGroupLayoutDescriptor<'l> {
    entries_tmp.reserve_exact(desc.entries.len());
    for entry in desc.entries.iter().copied() {
        entries_tmp.push(convert_bind_group_layout_entry(entry));
    }
    wgpu::BindGroupLayoutDescriptor {
        label: desc.label,
        entries: entries_tmp,
    }
}

pub fn convert_pipeline_layout_descriptor<'l>(
    _res: &WgpuResources,
    desc: &PipelineLayoutDescriptor<'l>,
    layouts_tmp: &'l mut Vec<&'l wgpu::BindGroupLayout>,
) -> wgpu::PipelineLayoutDescriptor<'l> {
    wgpu::PipelineLayoutDescriptor {
        label: desc.label,
        bind_group_layouts: layouts_tmp,
        push_constant_ranges: &[], // TODO
    }
}

pub fn convert_compute_pipeline_descriptor<'l, 'r: 'l>(
    res: &'r WgpuResources,
    desc: &ComputePipelineDescriptor<'l>,
) -> Result<wgpu::ComputePipelineDescriptor<'l>> {
    let layout = if let Some(layout) = desc.layout {
        Some(
            res.pipeline_layouts
                .get(layout)
                .ok_or(ConversionError::PipelineLayoutNotAvailable(layout))?,
        )
    } else {
        None
    };

    let module = res
        .shader_modules
        .get(desc.module)
        .ok_or(ConversionError::ShaderModuleNotAvailable(desc.module))?;

    Ok(wgpu::ComputePipelineDescriptor::<'l> {
        label: desc.label,
        layout,
        module,
        entry_point: desc.entry_point,
    })
}

pub fn convert_graphics_pipeline_descriptor<'l, 'r: 'l>(
    res: &'r WgpuResources,
    desc: &'r GraphicsPipelineDescriptor<'_>,
    buffers_tmp: &'l mut Vec<wgpu::VertexBufferLayout<'l>>,
    attribs_tmp: &'l mut Vec<wgpu::VertexAttribute>,
    targets_tmp: &'l mut Vec<Option<wgpu::ColorTargetState>>,
) -> Result<wgpu::RenderPipelineDescriptor<'l>> {
    let layout = if let Some(layout) = desc.layout {
        Some(
            res.pipeline_layouts
                .get(layout)
                .ok_or(ConversionError::PipelineLayoutNotAvailable(layout))?,
        )
    } else {
        None
    };

    let vertex = convert_vertex_state(res, &desc.vertex, buffers_tmp, attribs_tmp)?;

    let depth_stencil = if let Some(ref state) = desc.depth_stencil {
        Some(convert_depth_stencil_state(state)?)
    } else {
        None
    };

    let fragment = if let Some(ref fragment) = desc.fragment {
        Some(convert_fragment_state(res, fragment, targets_tmp)?)
    } else {
        None
    };

    Ok(wgpu::RenderPipelineDescriptor::<'l> {
        label: desc.label,
        layout,
        vertex,
        primitive: convert_primitive_state(&desc.primitive),
        depth_stencil,
        multisample: wgpu::MultisampleState {
            count: desc.samples,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        fragment,
        multiview: None,
    })
}

fn convert_vertex_state<'l, 'r: 'l>(
    res: &'r WgpuResources,
    state: &'r VertexState<'_>,
    buffers_tmp: &'l mut Vec<wgpu::VertexBufferLayout<'l>>,
    attributes_tmp: &'l mut Vec<wgpu::VertexAttribute>,
) -> Result<wgpu::VertexState<'l>> {
    let module = res
        .shader_modules
        .get(state.module)
        .ok_or(ConversionError::ShaderModuleNotAvailable(state.module))?;

    attributes_tmp.reserve_exact(state.buffers.iter().map(|l| l.attributes.len()).sum());
    for (i, attr) in state
        .buffers
        .iter()
        .flat_map(|l| l.attributes.as_ref())
        .copied()
        .enumerate()
    {
        attributes_tmp.push(convert_vertex_attribute(i, attr)?);
    }

    buffers_tmp.reserve_exact(state.buffers.len());
    let mut offset = 0;
    for layout in state.buffers.as_ref() {
        let next_offset = offset + layout.attributes.len();
        buffers_tmp.push(wgpu::VertexBufferLayout {
            array_stride: layout.array_stride as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes_tmp[offset..next_offset],
        });
        offset = next_offset;
    }

    Ok(wgpu::VertexState::<'l> {
        module,
        entry_point: state.entry_point,
        buffers: buffers_tmp,
    })
}

fn convert_fragment_state<'l, 'r: 'l>(
    res: &'r WgpuResources,
    state: &FragmentState<'l>,
    targets_tmp: &'l mut Vec<Option<wgpu::ColorTargetState>>,
) -> Result<wgpu::FragmentState<'l>> {
    let module = res
        .shader_modules
        .get(state.module)
        .ok_or(ConversionError::ShaderModuleNotAvailable(state.module))?;

    targets_tmp.reserve_exact(state.targets.len());
    for target in state.targets.as_ref() {
        targets_tmp.push(convert_color_target_state(&target)?);
    }

    Ok(wgpu::FragmentState {
        module,
        entry_point: state.entry_point,
        targets: targets_tmp,
    })
}

#[inline]
fn convert_color_target_state(val: &ColorTargetState) -> Result<Option<wgpu::ColorTargetState>> {
    Ok(Some(wgpu::ColorTargetState {
        format: convert_texture_format(val.format)?,
        blend: val.blend.map(convert_blent_state),
        write_mask: convert_color_write(val.write_mask),
    }))
}

#[inline]
fn convert_blent_state(val: BlendState) -> wgpu::BlendState {
    wgpu::BlendState {
        color: convert_blent_component(val.color),
        alpha: convert_blent_component(val.alpha),
    }
}

#[inline]
fn convert_blent_component(val: BlendComponent) -> wgpu::BlendComponent {
    wgpu::BlendComponent {
        operation: convert_blend_operation(val.operation),
        src_factor: convert_blend_factor(val.src_factor),
        dst_factor: convert_blend_factor(val.dst_factor),
    }
}

#[inline]
fn convert_primitive_state(val: &PrimitiveState) -> wgpu::PrimitiveState {
    wgpu::PrimitiveState {
        topology: convert_primitive_topology(val.topology),
        strip_index_format: None,
        front_face: convert_front_face(val.front_face),
        cull_mode: val.cull_mode.map(convert_face),

        polygon_mode: wgpu::PolygonMode::Fill, // TODO:
        unclipped_depth: false,                // TODO,
        conservative: false,                   // TODO
    }
}

#[inline]
fn convert_depth_stencil_state(val: &DepthStencilState) -> Result<wgpu::DepthStencilState> {
    Ok(wgpu::DepthStencilState {
        format: convert_texture_format(val.format)?,
        depth_write_enabled: val.depth.write_enabled,
        depth_compare: convert_compare_function(val.depth.compare),
        stencil: wgpu::StencilState {
            front: convert_stencil_face_state(&val.stencil.front),
            back: convert_stencil_face_state(&val.stencil.back),
            read_mask: val.stencil.read_mask,
            write_mask: val.stencil.write_mask,
        },
        bias: wgpu::DepthBiasState {
            constant: val.depth.bias,
            slope_scale: val.depth.bias_slope_scale,
            clamp: val.depth.bias_clamp,
        },
    })
}

#[inline]
fn convert_stencil_face_state(val: &StencilFaceState) -> wgpu::StencilFaceState {
    wgpu::StencilFaceState {
        compare: convert_compare_function(val.compare),
        fail_op: convert_stencil_operation(val.fail_op),
        depth_fail_op: convert_stencil_operation(val.depth_fail_op),
        pass_op: convert_stencil_operation(val.pass_op),
    }
}

#[inline]
fn convert_color_write(val: ColorWrite) -> wgpu::ColorWrites {
    let mut result = wgpu::ColorWrites::empty();
    if val.contains(ColorWrite::RED) {
        result |= wgpu::ColorWrites::RED;
    }
    if val.contains(ColorWrite::GREEN) {
        result |= wgpu::ColorWrites::GREEN;
    }
    if val.contains(ColorWrite::BLUE) {
        result |= wgpu::ColorWrites::BLUE;
    }
    if val.contains(ColorWrite::ALPHA) {
        result |= wgpu::ColorWrites::ALPHA;
    }
    result
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
fn convert_blend_operation(val: BlendOperation) -> wgpu::BlendOperation {
    match val {
        BlendOperation::Add => wgpu::BlendOperation::Add,
        BlendOperation::Subtract => wgpu::BlendOperation::Subtract,
        BlendOperation::ReverseSubtract => wgpu::BlendOperation::ReverseSubtract,
        BlendOperation::Min => wgpu::BlendOperation::Min,
        BlendOperation::Max => wgpu::BlendOperation::Max,
    }
}

#[inline]
fn convert_blend_factor(val: BlendFactor) -> wgpu::BlendFactor {
    match val {
        BlendFactor::Zero => wgpu::BlendFactor::Zero,
        BlendFactor::One => wgpu::BlendFactor::One,
        BlendFactor::Src => wgpu::BlendFactor::Src,
        BlendFactor::OneMinusSrc => wgpu::BlendFactor::OneMinusSrc,
        BlendFactor::SrcAlpha => wgpu::BlendFactor::SrcAlpha,
        BlendFactor::OneMinusSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
        BlendFactor::Dst => wgpu::BlendFactor::Dst,
        BlendFactor::OneMinusDst => wgpu::BlendFactor::OneMinusDst,
        BlendFactor::DstAlpha => wgpu::BlendFactor::DstAlpha,
        BlendFactor::OneMinusDstAlpha => wgpu::BlendFactor::OneMinusDstAlpha,
        BlendFactor::SrcAlphaSaturated => wgpu::BlendFactor::SrcAlphaSaturated,
        BlendFactor::Constant => wgpu::BlendFactor::Constant,
        BlendFactor::OneMinusConstant => wgpu::BlendFactor::OneMinusConstant,
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
fn convert_compare_function(val: CompareFunction) -> wgpu::CompareFunction {
    match val {
        CompareFunction::Never => wgpu::CompareFunction::Never,
        CompareFunction::Less => wgpu::CompareFunction::Less,
        CompareFunction::Equal => wgpu::CompareFunction::Equal,
        CompareFunction::LessEqual => wgpu::CompareFunction::LessEqual,
        CompareFunction::Greater => wgpu::CompareFunction::Greater,
        CompareFunction::NotEqual => wgpu::CompareFunction::NotEqual,
        CompareFunction::GreaterEqual => wgpu::CompareFunction::GreaterEqual,
        CompareFunction::Always => wgpu::CompareFunction::Always,
    }
}

#[inline]
fn convert_stencil_operation(val: StencilOperation) -> wgpu::StencilOperation {
    match val {
        StencilOperation::Keep => wgpu::StencilOperation::Keep,
        StencilOperation::Zero => wgpu::StencilOperation::Zero,
        StencilOperation::Replace => wgpu::StencilOperation::Replace,
        StencilOperation::Invert => wgpu::StencilOperation::Invert,
        StencilOperation::IncrementClamp => wgpu::StencilOperation::IncrementClamp,
        StencilOperation::DecrementClamp => wgpu::StencilOperation::DecrementClamp,
        StencilOperation::IncrementWrap => wgpu::StencilOperation::IncrementWrap,
        StencilOperation::DecrementWrap => wgpu::StencilOperation::DecrementWrap,
    }
}

// #[inline]
// fn convert_color_operations(val: Operations<Srgba>) -> wgpu::Operations<wgpu::Color> {
//     wgpu::Operations {
//         load: match val.load {
//             LoadOp::Clear(clear) => wgpu::LoadOp::Clear(convert_color(clear)),
//             LoadOp::Load => wgpu::LoadOp::Load,
//         },
//         store: val.store,
//     }
// }

// #[inline]
// fn convert_operations<T>(val: Operations<T>) -> wgpu::Operations<T> {
//     wgpu::Operations {
//         load: match val.load {
//             LoadOp::Clear(clear) => wgpu::LoadOp::Clear(clear),
//             LoadOp::Load => wgpu::LoadOp::Load,
//         },
//         store: val.store,
//     }
// }

#[inline]
fn convert_color(color: Srgba) -> wgpu::Color {
    wgpu::Color {
        r: color.red as f64,
        g: color.green as f64,
        b: color.blue as f64,
        a: color.alpha as f64,
    }
}

// pub fn convert_render_pass<'l, 'r: 'l>(
//     res: &'r RenderBackendResources<WgpuRendererBackend>,
//     desc: &GraphicsPassDescriptor<'l>,
//     tmp_color: &'l mut Vec<wgpu::RenderPassColorAttachment<'l>>,
// ) -> Result<wgpu::RenderPassDescriptor<'l, 'l>> {
//     tmp_color.reserve_exact(desc.color_attachments.len());
//     for a in desc.color_attachments {
//         tmp_color.push(convert_color_attachment(res, a)?);
//     }

//     let depth_stencil_attachment = if let Some(a) = &desc.depth_stencil_attachment {
//         Some(convert_depth_stencil_attachment(res, a)?)
//     } else {
//         None
//     };
//     Ok(wgpu::RenderPassDescriptor {
//         label: desc.label,
//         color_attachments: tmp_color,
//         depth_stencil_attachment,
//     })
// }

// pub fn convert_color_attachment<'r>(
//     res: &'r RenderBackendResources<WgpuRendererBackend>,
//     desc: &ColorAttachment,
// ) -> Result<wgpu::RenderPassColorAttachment<'l>> {
//     let view = res
//         .textures
//         .get(desc.texture)
//         .ok_or(ConversionError::TextureNotAvailable(desc.texture))?
//         .view();
//     let resolve_target = if let Some(resolve) = desc.resolve_target {
//         Some(
//             res.textures
//                 .get(resolve)
//                 .ok_or(ConversionError::TextureNotAvailable(resolve))?
//                 .view(),
//         )
//     } else {
//         None
//     };
//     Ok(wgpu::RenderPassColorAttachment {
//         view,
//         resolve_target,
//         ops: convert_color_operations(desc.ops),
//     })
// }

// pub fn convert_depth_stencil_attachment<'r>(
//     res: &'r RenderBackendResources<WgpuRendererBackend>,
//     desc: &DepthStencilAttachment,
// ) -> Result<wgpu::RenderPassDepthStencilAttachment<'l>> {
//     let view = res
//         .textures
//         .get(desc.texture)
//         .ok_or(ConversionError::TextureNotAvailable(desc.texture))?
//         .view();
//     Ok(wgpu::RenderPassDepthStencilAttachment {
//         view,
//         depth_ops: desc.depth_ops.map(convert_operations),
//         stencil_ops: desc.stencil_ops.map(convert_operations),
//     })
// }
