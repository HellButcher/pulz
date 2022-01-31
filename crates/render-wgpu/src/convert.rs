use std::ops::Deref;

use render::{
    buffer::{BufferDescriptor, BufferUsage},
    math::USize3,
    pass::{ColorAttachment, DepthStencilAttachment, GraphicsPassDescriptor, LoadOp, Operations},
    pipeline::{
        BindGroupLayoutDescriptor, BindGroupLayoutEntry, BlendComponent, BlendFactor,
        BlendOperation, BlendState, ColorWrite, CompareFunction, ComputePipelineDescriptor,
        DepthStencilState, Face, FragmentState, FrontFace, GraphicsPipelineDescriptor, IndexFormat,
        PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology, StencilFaceState,
        StencilOperation, VertexFormat, VertexState,
    },
    render_resource::RenderBackendResources,
    shader::{ShaderModuleDescriptor, ShaderSource},
    texture::{TextureDescriptor, TextureDimensions, TextureFormat, TextureUsage},
};
use wgpu::MultisampleState;

use crate::WgpuRendererBackend;

pub trait WgpuFrom<T> {
    fn from(val: &T) -> Self;
}

pub trait WgpuInto<U> {
    fn wgpu_into(&self) -> U;
}

impl<T, U> WgpuInto<U> for T
where
    U: WgpuFrom<T>,
{
    #[inline]
    fn wgpu_into(&self) -> U {
        U::from(self)
    }
}

impl<T: WgpuFrom<V>, V> WgpuFrom<Option<V>> for Option<T> {
    #[inline]
    fn from(val: &Option<V>) -> Self {
        val.as_ref().map(T::from)
    }
}

impl WgpuFrom<BufferDescriptor> for wgpu::BufferDescriptor<'_> {
    #[inline]
    fn from(val: &BufferDescriptor) -> Self {
        Self {
            label: None,
            size: val.size as u64,
            usage: val.usage.wgpu_into(),
            mapped_at_creation: true,
        }
    }
}

impl WgpuFrom<BufferUsage> for wgpu::BufferUsages {
    #[inline]
    fn from(val: &BufferUsage) -> Self {
        let mut result = Self::empty();
        if val.contains(BufferUsage::TRANSFER_SRC) {
            result |= Self::COPY_SRC;
        }
        if val.contains(BufferUsage::TRANSFER_DST) {
            result |= Self::COPY_DST;
        }
        if val.contains(BufferUsage::INDEX) {
            result |= Self::INDEX;
        }
        if val.contains(BufferUsage::UNIFORM) {
            result |= Self::UNIFORM;
        }
        if val.contains(BufferUsage::STORAGE) {
            result |= Self::STORAGE;
        }
        if val.contains(BufferUsage::INDIRECT) {
            result |= Self::INDIRECT;
        }
        result
    }
}

impl WgpuFrom<TextureDescriptor> for wgpu::TextureDescriptor<'_> {
    #[inline]
    fn from(val: &TextureDescriptor) -> Self {
        Self {
            label: None,
            size: val.dimensions.wgpu_into(),
            mip_level_count: val.mip_level_count,
            sample_count: val.sample_count,
            dimension: val.dimensions.wgpu_into(),
            format: val.format.wgpu_into(),
            usage: val.usage.wgpu_into(),
        }
    }
}

impl WgpuFrom<TextureDescriptor> for wgpu::TextureViewDescriptor<'_> {
    #[inline]
    fn from(val: &TextureDescriptor) -> Self {
        Self {
            dimension: Some(val.dimensions.wgpu_into()),
            ..Default::default()
        }
    }
}

impl WgpuFrom<TextureDescriptor> for wgpu::ImageDataLayout {
    #[inline]
    fn from(val: &TextureDescriptor) -> Self {
        let extend: wgpu::Extent3d = val.dimensions.wgpu_into();
        Self {
            offset: 0,
            bytes_per_row: std::num::NonZeroU32::new(
                extend.width * val.format.bytes_per_pixel() as u32,
            ),
            rows_per_image: if extend.depth_or_array_layers > 1 {
                std::num::NonZeroU32::new(extend.height)
            } else {
                None
            },
        }
    }
}

impl WgpuFrom<TextureDescriptor> for wgpu::Extent3d {
    #[inline]
    fn from(val: &TextureDescriptor) -> Self {
        val.dimensions.wgpu_into()
    }
}

impl WgpuFrom<TextureDimensions> for wgpu::Extent3d {
    #[inline]
    fn from(val: &TextureDimensions) -> Self {
        match *val {
            TextureDimensions::D1(len) => Self {
                width: len as u32,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimensions::D2(size) => Self {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            TextureDimensions::D2Array { size, array_len } => Self {
                width: size.x,
                height: size.y,
                depth_or_array_layers: array_len as u32,
            },
            TextureDimensions::Cube(size) => Self {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 6,
            },
            TextureDimensions::CubeArray { size, array_len } => Self {
                width: size.x,
                height: size.y,
                depth_or_array_layers: array_len as u32 * 6,
            },
            TextureDimensions::D3(size) => Self {
                width: size.x,
                height: size.y,
                depth_or_array_layers: size.z,
            },
        }
    }
}

impl WgpuFrom<TextureDimensions> for wgpu::TextureViewDimension {
    #[inline]
    fn from(val: &TextureDimensions) -> Self {
        match val {
            TextureDimensions::D1(_) => Self::D1,
            TextureDimensions::D2 { .. } => Self::D2,
            TextureDimensions::D2Array { .. } => Self::D2Array,
            TextureDimensions::Cube { .. } => Self::Cube,
            TextureDimensions::CubeArray { .. } => Self::CubeArray,
            TextureDimensions::D3 { .. } => Self::D3,
        }
    }
}

impl WgpuFrom<TextureDimensions> for wgpu::TextureDimension {
    #[inline]
    fn from(val: &TextureDimensions) -> Self {
        match val {
            TextureDimensions::D1(_) => Self::D1,
            TextureDimensions::D2 { .. }
            | TextureDimensions::D2Array { .. }
            | TextureDimensions::Cube { .. }
            | TextureDimensions::CubeArray { .. } => Self::D2,
            TextureDimensions::D3 { .. } => Self::D3,
        }
    }
}

impl WgpuFrom<USize3> for wgpu::Extent3d {
    #[inline]
    fn from(val: &USize3) -> Self {
        Self {
            width: val.x,
            height: val.y,
            depth_or_array_layers: val.z,
        }
    }
}

impl WgpuFrom<[u32; 3]> for wgpu::Extent3d {
    #[inline]
    fn from(val: &[u32; 3]) -> Self {
        Self {
            width: val[0],
            height: val[1],
            depth_or_array_layers: val[2],
        }
    }
}

impl WgpuFrom<TextureFormat> for wgpu::TextureFormat {
    #[inline]
    fn from(val: &TextureFormat) -> Self {
        match val {
            // 8-bit formats
            TextureFormat::R8Unorm => Self::R8Unorm,
            TextureFormat::R8Snorm => Self::R8Snorm,
            TextureFormat::R8Uint => Self::R8Uint,
            TextureFormat::R8Sint => Self::R8Sint,

            // 16-bit formats
            TextureFormat::R16Uint => Self::R16Uint,
            TextureFormat::R16Sint => Self::R16Sint,
            TextureFormat::R16Float => Self::R16Float,
            TextureFormat::Rg8Unorm => Self::Rg8Unorm,
            TextureFormat::Rg8Snorm => Self::Rg8Snorm,
            TextureFormat::Rg8Uint => Self::Rg8Uint,
            TextureFormat::Rg8Sint => Self::Rg8Sint,

            // 32-bit formats
            TextureFormat::R32Uint => Self::R32Uint,
            TextureFormat::R32Sint => Self::R32Sint,
            TextureFormat::R32Float => Self::R32Float,
            TextureFormat::Rg16Uint => Self::Rg16Uint,
            TextureFormat::Rg16Sint => Self::Rg16Sint,
            TextureFormat::Rg16Float => Self::Rg16Float,
            TextureFormat::Rgba8Unorm => Self::Rgba8Unorm,
            TextureFormat::Rgba8Srgb => Self::Rgba8UnormSrgb,
            TextureFormat::Rgba8Snorm => Self::Rgba8Snorm,
            TextureFormat::Rgba8Uint => Self::Rgba8Uint,
            TextureFormat::Rgba8Sint => Self::Rgba8Sint,
            TextureFormat::Bgra8Unorm => Self::Bgra8Unorm,
            TextureFormat::Bgra8Srgb => Self::Bgra8UnormSrgb,

            // Packed 32-bit formats
            TextureFormat::Rgb9E5Ufloat => Self::Rgb9e5Ufloat,
            TextureFormat::Rgb10A2Unorm => Self::Rgb10a2Unorm,
            TextureFormat::Rg11B10Float => Self::Rg11b10Float,

            // 64-bit formats
            TextureFormat::Rg32Uint => Self::Rg32Uint,
            TextureFormat::Rg32Sint => Self::Rg32Sint,
            TextureFormat::Rg32Float => Self::Rg32Float,
            TextureFormat::Rgba16Uint => Self::Rgba16Uint,
            TextureFormat::Rgba16Sint => Self::Rgba16Sint,
            TextureFormat::Rgba16Float => Self::Rgba16Float,

            // 128-bit formats
            TextureFormat::Rgba32Uint => Self::Rgba32Uint,
            TextureFormat::Rgba32Sint => Self::Rgba32Sint,
            TextureFormat::Rgba32Float => Self::Rgba32Float,

            // Depth and stencil formats
            // TODO: uncomment, when implemented in wgpu
            // TextureFormat::Stencil8 => Self::Stencil8,
            // TextureFormat::Depth16Unorm => Self::Depth16Unorm,
            TextureFormat::Depth24Plus => Self::Depth24Plus,
            TextureFormat::Depth24PlusStencil8 => Self::Depth24PlusStencil8,
            TextureFormat::Depth32Float => Self::Depth32Float,
        }
    }
}

impl WgpuFrom<wgpu::TextureFormat> for TextureFormat {
    #[inline]
    fn from(val: &wgpu::TextureFormat) -> Self {
        match val {
            // 8-bit formats
            wgpu::TextureFormat::R8Unorm => Self::R8Unorm,
            wgpu::TextureFormat::R8Snorm => Self::R8Snorm,
            wgpu::TextureFormat::R8Uint => Self::R8Uint,
            wgpu::TextureFormat::R8Sint => Self::R8Sint,

            // 16-bit formats
            wgpu::TextureFormat::R16Uint => Self::R16Uint,
            wgpu::TextureFormat::R16Sint => Self::R16Sint,
            wgpu::TextureFormat::R16Float => Self::R16Float,
            wgpu::TextureFormat::Rg8Unorm => Self::Rg8Unorm,
            wgpu::TextureFormat::Rg8Snorm => Self::Rg8Snorm,
            wgpu::TextureFormat::Rg8Uint => Self::Rg8Uint,
            wgpu::TextureFormat::Rg8Sint => Self::Rg8Sint,

            // 32-bit formats
            wgpu::TextureFormat::R32Uint => Self::R32Uint,
            wgpu::TextureFormat::R32Sint => Self::R32Sint,
            wgpu::TextureFormat::R32Float => Self::R32Float,
            wgpu::TextureFormat::Rg16Uint => Self::Rg16Uint,
            wgpu::TextureFormat::Rg16Sint => Self::Rg16Sint,
            wgpu::TextureFormat::Rg16Float => Self::Rg16Float,
            wgpu::TextureFormat::Rgba8Unorm => Self::Rgba8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb => Self::Rgba8Srgb,
            wgpu::TextureFormat::Rgba8Snorm => Self::Rgba8Snorm,
            wgpu::TextureFormat::Rgba8Uint => Self::Rgba8Uint,
            wgpu::TextureFormat::Rgba8Sint => Self::Rgba8Sint,
            wgpu::TextureFormat::Bgra8Unorm => Self::Bgra8Unorm,
            wgpu::TextureFormat::Bgra8UnormSrgb => Self::Bgra8Srgb,

            // Packed 32-bit formats
            wgpu::TextureFormat::Rgb9e5Ufloat => Self::Rgb9E5Ufloat,
            wgpu::TextureFormat::Rgb10a2Unorm => Self::Rgb10A2Unorm,
            wgpu::TextureFormat::Rg11b10Float => Self::Rg11B10Float,

            // 64-bit formats
            wgpu::TextureFormat::Rg32Uint => Self::Rg32Uint,
            wgpu::TextureFormat::Rg32Sint => Self::Rg32Sint,
            wgpu::TextureFormat::Rg32Float => Self::Rg32Float,
            wgpu::TextureFormat::Rgba16Uint => Self::Rgba16Uint,
            wgpu::TextureFormat::Rgba16Sint => Self::Rgba16Sint,
            wgpu::TextureFormat::Rgba16Float => Self::Rgba16Float,

            // 128-bit formats
            wgpu::TextureFormat::Rgba32Uint => Self::Rgba32Uint,
            wgpu::TextureFormat::Rgba32Sint => Self::Rgba32Sint,
            wgpu::TextureFormat::Rgba32Float => Self::Rgba32Float,

            // Depth and stencil formats
            // TODO: uncomment, when implemented in wgpu
            //wgpu::TextureFormat::Stencil8 => Self::Stencil8,
            //TextureFormat::Depth16Unorm => wgpu::TextureFormat::Depth16Unorm,
            wgpu::TextureFormat::Depth24Plus => Self::Depth24Plus,
            wgpu::TextureFormat::Depth24PlusStencil8 => Self::Depth24PlusStencil8,
            wgpu::TextureFormat::Depth32Float => Self::Depth32Float,

            _ => panic!("unsupported texture format"),
        }
    }
}

impl WgpuFrom<VertexFormat> for wgpu::VertexFormat {
    #[inline]
    fn from(val: &VertexFormat) -> Self {
        match val {
            VertexFormat::Uint8x2 => Self::Uint8x2,
            VertexFormat::Uint8x4 => Self::Uint8x4,
            VertexFormat::Sint8x2 => Self::Sint8x2,
            VertexFormat::Sint8x4 => Self::Sint8x4,
            VertexFormat::Unorm8x2 => Self::Unorm8x2,
            VertexFormat::Unorm8x4 => Self::Unorm8x4,
            VertexFormat::Snorm8x2 => Self::Snorm8x2,
            VertexFormat::Snorm8x4 => Self::Snorm8x4,
            VertexFormat::Uint16x2 => Self::Uint16x2,
            VertexFormat::Uint16x4 => Self::Uint16x4,
            VertexFormat::Sint16x2 => Self::Sint16x2,
            VertexFormat::Sint16x4 => Self::Sint16x4,
            VertexFormat::Unorm16x2 => Self::Unorm16x2,
            VertexFormat::Unorm16x4 => Self::Unorm16x4,
            VertexFormat::Snorm16x2 => Self::Snorm16x2,
            VertexFormat::Snorm16x4 => Self::Snorm16x4,
            VertexFormat::Float16x2 => Self::Float16x2,
            VertexFormat::Float16x4 => Self::Float16x4,
            VertexFormat::Float32 => Self::Float32,
            VertexFormat::Float32x2 => Self::Float32x2,
            VertexFormat::Float32x3 => Self::Float32x3,
            VertexFormat::Float32x4 => Self::Float32x4,
            VertexFormat::Uint32 => Self::Uint32,
            VertexFormat::Uint32x2 => Self::Uint32x2,
            VertexFormat::Uint32x3 => Self::Uint32x3,
            VertexFormat::Uint32x4 => Self::Uint32x4,
            VertexFormat::Sint32 => Self::Sint32,
            VertexFormat::Sint32x2 => Self::Sint32x2,
            VertexFormat::Sint32x3 => Self::Sint32x3,
            VertexFormat::Sint32x4 => Self::Sint32x4,
        }
    }
}

impl WgpuFrom<TextureUsage> for wgpu::TextureUsages {
    fn from(val: &TextureUsage) -> Self {
        let mut result = Self::empty();
        if val.contains(TextureUsage::TRANSFER_SRC) {
            result |= Self::COPY_SRC;
        }
        if val.contains(TextureUsage::TRANSFER_DST) {
            result |= Self::COPY_DST;
        }
        if val.contains(TextureUsage::TEXTURE_BINDING) {
            result |= Self::TEXTURE_BINDING;
        }
        if val.contains(TextureUsage::STORAGE_BINDING) {
            result |= Self::STORAGE_BINDING;
        }
        if val.contains(TextureUsage::COLOR_ATTACHMENT)
            || val.contains(TextureUsage::DEPTH_STENCIL_ATTACHMENT)
        {
            result |= Self::RENDER_ATTACHMENT;
        }
        result
    }
}

impl WgpuFrom<BindGroupLayoutEntry> for wgpu::BindGroupLayoutEntry {
    fn from(_val: &BindGroupLayoutEntry) -> Self {
        todo!() // TODO:
                // Self{
                //     binding,
                //     visibility,
                //     ty,
                //     count: val.count
                // }
    }
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
    for entry in desc.entries {
        entries_tmp.push(entry.wgpu_into());
    }
    wgpu::BindGroupLayoutDescriptor {
        label: desc.label,
        entries: entries_tmp,
    }
}

pub fn convert_pipeline_layout_descriptor<'l>(
    _res: &'l RenderBackendResources<WgpuRendererBackend>,
    desc: &PipelineLayoutDescriptor<'l>,
    layouts_tmp: &'l mut Vec<&'l wgpu::BindGroupLayout>,
) -> wgpu::PipelineLayoutDescriptor<'l> {
    wgpu::PipelineLayoutDescriptor {
        label: desc.label,
        bind_group_layouts: layouts_tmp,
        push_constant_ranges: &[], // TODO
    }
}

pub fn convert_compute_pipeline_descriptor<'l>(
    res: &'l RenderBackendResources<WgpuRendererBackend>,
    desc: &ComputePipelineDescriptor<'l>,
) -> Option<wgpu::ComputePipelineDescriptor<'l>> {
    let layout = if let Some(layout) = desc.layout {
        Some(res.pipeline_layouts.get(layout)?)
    } else {
        None
    };

    let module = res.shader_modules.get(desc.module)?;

    Some(wgpu::ComputePipelineDescriptor {
        label: desc.label,
        layout,
        module,
        entry_point: desc.entry_point,
    })
}

pub fn convert_graphics_pipeline_descriptor<'l>(
    res: &'l RenderBackendResources<WgpuRendererBackend>,
    desc: &GraphicsPipelineDescriptor<'l>,
    buffers_tmp: &'l mut Vec<wgpu::VertexBufferLayout<'l>>,
    attribs_tmp: &'l mut Vec<wgpu::VertexAttribute>,
    targets_tmp: &'l mut Vec<wgpu::ColorTargetState>,
) -> Option<wgpu::RenderPipelineDescriptor<'l>> {
    let layout = if let Some(layout) = desc.layout {
        Some(res.pipeline_layouts.get(layout)?)
    } else {
        None
    };

    let vertex = convert_vertex_state(res, &desc.vertex, buffers_tmp, attribs_tmp)?;

    let fragment = if let Some(ref fragment) = desc.fragment {
        Some(convert_fragment_state(res, fragment, targets_tmp)?)
    } else {
        None
    };

    Some(wgpu::RenderPipelineDescriptor {
        label: desc.label,
        layout,
        vertex,
        primitive: desc.primitive.wgpu_into(),
        depth_stencil: desc.depth_stencil.wgpu_into(),
        multisample: if desc.samples > 1 {
            MultisampleState {
                count: desc.samples,
                mask: !0,
                alpha_to_coverage_enabled: false,
            }
        } else {
            Default::default()
        },
        fragment,
        multiview: None,
    })
}

fn convert_vertex_state<'l>(
    res: &'l RenderBackendResources<WgpuRendererBackend>,
    state: &VertexState<'l>,
    buffers_tmp: &'l mut Vec<wgpu::VertexBufferLayout<'l>>,
    attributes_tmp: &'l mut Vec<wgpu::VertexAttribute>,
) -> Option<wgpu::VertexState<'l>> {
    let module = res.shader_modules.get(state.module)?;

    attributes_tmp.reserve_exact(state.buffers.iter().map(|l| l.attributes.len()).sum());
    for (i, attr) in state.buffers.iter().flat_map(|l| l.attributes).enumerate() {
        attributes_tmp.push(wgpu::VertexAttribute {
            format: attr.format.wgpu_into(),
            offset: attr.offset as u64,
            shader_location: i as u32,
        });
    }

    buffers_tmp.reserve_exact(state.buffers.len());
    let mut offset = 0;
    for layout in state.buffers {
        let next_offset = offset + layout.attributes.len();
        buffers_tmp.push(wgpu::VertexBufferLayout {
            array_stride: layout.array_stride as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes_tmp[offset..next_offset],
        });
        offset = next_offset;
    }

    Some(wgpu::VertexState {
        module,
        entry_point: state.entry_point,
        buffers: buffers_tmp,
    })
}

fn convert_fragment_state<'l>(
    res: &'l RenderBackendResources<WgpuRendererBackend>,
    state: &FragmentState<'l>,
    targets_tmp: &'l mut Vec<wgpu::ColorTargetState>,
) -> Option<wgpu::FragmentState<'l>> {
    let module = res.shader_modules.get(state.module)?;

    targets_tmp.reserve_exact(state.targets.len());
    for target in state.targets {
        targets_tmp.push(wgpu::ColorTargetState {
            format: target.format.wgpu_into(),
            blend: target.blend.wgpu_into(),
            write_mask: target.write_mask.wgpu_into(),
        })
    }

    Some(wgpu::FragmentState {
        module,
        entry_point: state.entry_point,
        targets: targets_tmp,
    })
}

impl WgpuFrom<BlendState> for wgpu::BlendState {
    #[inline]
    fn from(val: &BlendState) -> Self {
        Self {
            color: val.color.wgpu_into(),
            alpha: val.alpha.wgpu_into(),
        }
    }
}

impl WgpuFrom<BlendComponent> for wgpu::BlendComponent {
    #[inline]
    fn from(val: &BlendComponent) -> Self {
        Self {
            operation: val.operation.wgpu_into(),
            src_factor: val.src_factor.wgpu_into(),
            dst_factor: val.dst_factor.wgpu_into(),
        }
    }
}

impl WgpuFrom<PrimitiveState> for wgpu::PrimitiveState {
    #[inline]
    fn from(val: &PrimitiveState) -> Self {
        Self {
            topology: val.topology.wgpu_into(),
            strip_index_format: None,
            front_face: val.front_face.wgpu_into(),
            cull_mode: val.cull_mode.wgpu_into(),

            polygon_mode: wgpu::PolygonMode::Fill, // TODO:
            unclipped_depth: false,                // TODO,
            conservative: false,                   // TODO
        }
    }
}

impl WgpuFrom<DepthStencilState> for wgpu::DepthStencilState {
    #[inline]
    fn from(val: &DepthStencilState) -> Self {
        Self {
            format: val.format.wgpu_into(),
            depth_write_enabled: val.depth.write_enabled,
            depth_compare: val.depth.compare.wgpu_into(),
            stencil: wgpu::StencilState {
                front: val.stencil.front.wgpu_into(),
                back: val.stencil.back.wgpu_into(),
                read_mask: val.stencil.read_mask,
                write_mask: val.stencil.write_mask,
            },
            bias: wgpu::DepthBiasState {
                constant: val.depth.bias,
                slope_scale: val.depth.bias_slope_scale,
                clamp: val.depth.bias_clamp,
            },
        }
    }
}

impl WgpuFrom<StencilFaceState> for wgpu::StencilFaceState {
    #[inline]
    fn from(val: &StencilFaceState) -> Self {
        Self {
            compare: val.compare.wgpu_into(),
            fail_op: val.fail_op.wgpu_into(),
            depth_fail_op: val.depth_fail_op.wgpu_into(),
            pass_op: val.pass_op.wgpu_into(),
        }
    }
}

impl WgpuFrom<ColorWrite> for wgpu::ColorWrites {
    #[inline]
    fn from(val: &ColorWrite) -> Self {
        // SAFETY: all bitflags of both types are identical
        unsafe { Self::from_bits_unchecked(val.bits()) }
    }
}

impl WgpuFrom<PrimitiveTopology> for wgpu::PrimitiveTopology {
    #[inline]
    fn from(val: &PrimitiveTopology) -> Self {
        match val {
            PrimitiveTopology::PointList => Self::PointList,
            PrimitiveTopology::LineList => Self::LineList,
            PrimitiveTopology::LineStrip => Self::LineStrip,
            PrimitiveTopology::TriangleList => Self::TriangleList,
            PrimitiveTopology::TriangleStrip => Self::TriangleStrip,
        }
    }
}

impl WgpuFrom<FrontFace> for wgpu::FrontFace {
    #[inline]
    fn from(val: &FrontFace) -> Self {
        match val {
            FrontFace::CounterClockwise => Self::Ccw,
            FrontFace::Clockwise => Self::Cw,
        }
    }
}

impl WgpuFrom<Face> for wgpu::Face {
    #[inline]
    fn from(val: &Face) -> Self {
        match val {
            Face::Front => Self::Front,
            Face::Back => Self::Back,
        }
    }
}

impl WgpuFrom<BlendOperation> for wgpu::BlendOperation {
    #[inline]
    fn from(val: &BlendOperation) -> Self {
        match val {
            BlendOperation::Add => Self::Add,
            BlendOperation::Subtract => Self::Subtract,
            BlendOperation::ReverseSubtract => Self::ReverseSubtract,
            BlendOperation::Min => Self::Min,
            BlendOperation::Max => Self::Max,
        }
    }
}

impl WgpuFrom<BlendFactor> for wgpu::BlendFactor {
    #[inline]
    fn from(val: &BlendFactor) -> Self {
        match val {
            BlendFactor::Zero => Self::Zero,
            BlendFactor::One => Self::One,
            BlendFactor::Src => Self::Src,
            BlendFactor::OneMinusSrc => Self::OneMinusSrc,
            BlendFactor::SrcAlpha => Self::SrcAlpha,
            BlendFactor::OneMinusSrcAlpha => Self::OneMinusSrcAlpha,
            BlendFactor::Dst => Self::Dst,
            BlendFactor::OneMinusDst => Self::OneMinusDst,
            BlendFactor::DstAlpha => Self::DstAlpha,
            BlendFactor::OneMinusDstAlpha => Self::OneMinusDstAlpha,
            BlendFactor::SrcAlphaSaturated => Self::SrcAlphaSaturated,
            BlendFactor::Constant => Self::Constant,
            BlendFactor::OneMinusConstant => Self::OneMinusConstant,
        }
    }
}

impl WgpuFrom<IndexFormat> for wgpu::IndexFormat {
    #[inline]
    fn from(val: &IndexFormat) -> Self {
        match val {
            IndexFormat::Uint16 => Self::Uint16,
            IndexFormat::Uint32 => Self::Uint32,
        }
    }
}

impl WgpuFrom<CompareFunction> for wgpu::CompareFunction {
    #[inline]
    fn from(val: &CompareFunction) -> Self {
        match val {
            CompareFunction::Never => Self::Never,
            CompareFunction::Less => Self::Less,
            CompareFunction::Equal => Self::Equal,
            CompareFunction::LessEqual => Self::LessEqual,
            CompareFunction::Greater => Self::Greater,
            CompareFunction::NotEqual => Self::NotEqual,
            CompareFunction::GreaterEqual => Self::GreaterEqual,
            CompareFunction::Always => Self::Always,
        }
    }
}

impl WgpuFrom<StencilOperation> for wgpu::StencilOperation {
    #[inline]
    fn from(val: &StencilOperation) -> Self {
        match val {
            StencilOperation::Keep => Self::Keep,
            StencilOperation::Zero => Self::Zero,
            StencilOperation::Replace => Self::Replace,
            StencilOperation::Invert => Self::Invert,
            StencilOperation::IncrementClamp => Self::IncrementClamp,
            StencilOperation::DecrementClamp => Self::DecrementClamp,
            StencilOperation::IncrementWrap => Self::IncrementWrap,
            StencilOperation::DecrementWrap => Self::DecrementWrap,
        }
    }
}

impl<A, B> WgpuFrom<Operations<A>> for wgpu::Operations<B>
where
    wgpu::LoadOp<B>: WgpuFrom<LoadOp<A>>,
{
    #[inline]
    fn from(val: &Operations<A>) -> Self {
        Self {
            load: val.load.wgpu_into(),
            store: val.store,
        }
    }
}

impl<C: Copy + Clone> WgpuFrom<LoadOp<C>> for wgpu::LoadOp<C> {
    #[inline]
    fn from(val: &LoadOp<C>) -> Self {
        match val {
            LoadOp::Clear(color) => Self::Clear(*color),
            LoadOp::Load => Self::Load,
        }
    }
}

impl WgpuFrom<LoadOp<render::color::Srgba>> for wgpu::LoadOp<wgpu::Color> {
    #[inline]
    fn from(val: &LoadOp<render::color::Srgba>) -> Self {
        match val {
            LoadOp::Clear(color) => Self::Clear(wgpu::Color {
                r: color.red as f64,
                g: color.green as f64,
                b: color.blue as f64,
                a: color.alpha as f64,
            }),
            LoadOp::Load => Self::Load,
        }
    }
}

pub fn convert_render_pass<'l>(
    res: &'l RenderBackendResources<WgpuRendererBackend>,
    desc: &GraphicsPassDescriptor<'l>,
    tmp_color: &'l mut Vec<wgpu::RenderPassColorAttachment<'l>>,
) -> Option<wgpu::RenderPassDescriptor<'l, 'l>> {
    tmp_color.reserve_exact(desc.color_attachments.len());
    for a in desc.color_attachments {
        tmp_color.push(convert_color_attachment(res, a)?);
    }

    let depth_stencil_attachment = if let Some(a) = &desc.depth_stencil_attachment {
        Some(convert_depth_stencil_attachment(res, a)?)
    } else {
        None
    };
    Some(wgpu::RenderPassDescriptor {
        label: desc.label,
        color_attachments: tmp_color,
        depth_stencil_attachment,
    })
}

pub fn convert_color_attachment<'l>(
    res: &'l RenderBackendResources<WgpuRendererBackend>,
    desc: &ColorAttachment,
) -> Option<wgpu::RenderPassColorAttachment<'l>> {
    let view = res.textures.get(desc.texture)?.view();
    let resolve_target = if let Some(resolve) = desc.resolve_target {
        Some(res.textures.get(resolve)?.view())
    } else {
        None
    };
    Some(wgpu::RenderPassColorAttachment {
        view,
        resolve_target,
        ops: desc.ops.wgpu_into(),
    })
}

pub fn convert_depth_stencil_attachment<'l>(
    res: &'l RenderBackendResources<WgpuRendererBackend>,
    desc: &DepthStencilAttachment,
) -> Option<wgpu::RenderPassDepthStencilAttachment<'l>> {
    let view = res.textures.get(desc.texture)?.view();
    Some(wgpu::RenderPassDepthStencilAttachment {
        view,
        depth_ops: desc.depth_ops.wgpu_into(),
        stencil_ops: desc.stencil_ops.wgpu_into(),
    })
}
