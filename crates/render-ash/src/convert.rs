use std::marker::PhantomData;

use ash::vk;
use pulz_render::{
    buffer::{BufferDescriptor, BufferUsage},
    graph::{access::Stage, pass::PipelineBindPoint},
    math::{USize2, USize3},
    pipeline::{
        BindGroupLayoutDescriptor, BlendFactor, BlendOperation, CompareFunction,
        ComputePipelineDescriptor, DepthStencilState, Face, FrontFace, GraphicsPipelineDescriptor,
        IndexFormat, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology,
        RayTracingPipelineDescriptor, StencilFaceState, StencilOperation, VertexFormat,
    },
    texture::{TextureAspects, TextureDescriptor, TextureDimensions, TextureFormat, TextureUsage},
};

use crate::resources::AshResources;

pub trait VkFrom<T> {
    fn from(val: &T) -> Self;
}

pub trait VkInto<U> {
    fn vk_into(&self) -> U;
}

impl<T, U> VkInto<U> for T
where
    U: VkFrom<T>,
{
    #[inline]
    fn vk_into(&self) -> U {
        U::from(self)
    }
}

impl<T: VkFrom<V>, V> VkFrom<Option<V>> for Option<T> {
    #[inline]
    fn from(val: &Option<V>) -> Self {
        val.as_ref().map(T::from)
    }
}

impl VkFrom<BufferDescriptor> for vk::BufferCreateInfo {
    #[inline]
    fn from(val: &BufferDescriptor) -> Self {
        Self::builder()
            .size(val.size as u64)
            .usage(val.usage.vk_into())
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .build()
    }
}

impl VkFrom<BufferUsage> for vk::BufferUsageFlags {
    #[inline]
    fn from(val: &BufferUsage) -> Self {
        let mut result = Self::empty();
        if val.contains(BufferUsage::TRANSFER_SRC) {
            result |= Self::TRANSFER_SRC;
        }
        if val.contains(BufferUsage::TRANSFER_DST) {
            result |= Self::TRANSFER_DST;
        }
        if val.contains(BufferUsage::INDEX) {
            result |= Self::INDEX_BUFFER;
        }
        if val.contains(BufferUsage::UNIFORM) {
            result |= Self::UNIFORM_BUFFER;
        }
        if val.contains(BufferUsage::STORAGE) {
            result |= Self::STORAGE_BUFFER;
        }
        if val.contains(BufferUsage::INDIRECT) {
            result |= Self::INDIRECT_BUFFER;
        }
        // if val.contains(BufferUsage::UNIFORM_TEXEL) {
        //     result |= Self::UNIFORM_TEXEL_BUFFER;
        // }
        // if val.contains(BufferUsage::STORAGE_TEXEL) {
        //     result |= Self::STORAGE_TEXEL_BUFFER;
        // }
        result
    }
}

fn get_array_layers(dimensions: &TextureDimensions) -> u32 {
    match dimensions {
        TextureDimensions::Cube(_) => 6,
        TextureDimensions::D2Array { array_len, .. } => *array_len,
        TextureDimensions::CubeArray { array_len, .. } => *array_len * 6,
        _ => 1,
    }
}

impl VkFrom<TextureDescriptor> for vk::ImageCreateInfo {
    fn from(val: &TextureDescriptor) -> Self {
        Self::builder()
            .image_type(val.dimensions.vk_into())
            .format(val.format.vk_into())
            .extent(val.dimensions.vk_into())
            .array_layers(get_array_layers(&val.dimensions))
            .mip_levels(val.mip_level_count)
            .samples(vk::SampleCountFlags::from_raw(val.sample_count))
            .usage(val.usage.vk_into())
            .tiling(vk::ImageTiling::OPTIMAL)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .build()
    }
}

impl VkFrom<TextureDescriptor> for vk::ImageViewCreateInfo {
    fn from(val: &TextureDescriptor) -> Self {
        Self::builder()
            .view_type(val.dimensions.vk_into())
            .format(val.format.vk_into())
            .subresource_range(
                vk::ImageSubresourceRange::builder()
                    .aspect_mask(val.aspects().vk_into())
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(get_array_layers(&val.dimensions))
                    .build(),
            )
            .build()
    }
}

impl VkFrom<TextureDescriptor> for vk::Extent3D {
    #[inline]
    fn from(val: &TextureDescriptor) -> Self {
        val.dimensions.vk_into()
    }
}

impl VkFrom<TextureDimensions> for vk::Extent3D {
    #[inline]
    fn from(val: &TextureDimensions) -> Self {
        match *val {
            TextureDimensions::D1(len) => Self {
                width: len,
                height: 1,
                depth: 1,
            },
            TextureDimensions::D2(size) => Self {
                width: size.x,
                height: size.y,
                depth: 1,
            },
            TextureDimensions::D2Array { size, array_len: _ } => Self {
                width: size.x,
                height: size.y,
                depth: 1,
            },
            TextureDimensions::Cube(size) => Self {
                width: size.x,
                height: size.y,
                depth: 1,
            },
            TextureDimensions::CubeArray { size, array_len: _ } => Self {
                width: size.x,
                height: size.y,
                depth: 1,
            },
            TextureDimensions::D3(size) => Self {
                width: size.x,
                height: size.y,
                depth: size.z,
            },
        }
    }
}

impl VkFrom<TextureAspects> for vk::ImageAspectFlags {
    #[inline]
    fn from(val: &TextureAspects) -> Self {
        let mut result = Self::empty();
        if val.contains(TextureAspects::COLOR) {
            result |= Self::COLOR;
        }
        if val.contains(TextureAspects::DEPTH) {
            result |= Self::DEPTH;
        }
        if val.contains(TextureAspects::STENCIL) {
            result |= Self::STENCIL;
        }
        result
    }
}

impl VkFrom<TextureDimensions> for vk::ImageViewType {
    #[inline]
    fn from(val: &TextureDimensions) -> Self {
        match val {
            TextureDimensions::D1(_) => Self::TYPE_1D,
            TextureDimensions::D2 { .. } => Self::TYPE_2D,
            TextureDimensions::D2Array { .. } => Self::TYPE_2D_ARRAY,
            TextureDimensions::Cube { .. } => Self::CUBE,
            TextureDimensions::CubeArray { .. } => Self::CUBE_ARRAY,
            TextureDimensions::D3 { .. } => Self::TYPE_3D,
        }
    }
}

impl VkFrom<TextureDimensions> for vk::ImageType {
    #[inline]
    fn from(val: &TextureDimensions) -> Self {
        match val {
            TextureDimensions::D1(_) => Self::TYPE_1D,
            TextureDimensions::D2 { .. }
            | TextureDimensions::D2Array { .. }
            | TextureDimensions::Cube { .. }
            | TextureDimensions::CubeArray { .. } => Self::TYPE_2D,
            TextureDimensions::D3 { .. } => Self::TYPE_3D,
        }
    }
}

impl VkFrom<USize3> for vk::Extent3D {
    #[inline]
    fn from(val: &USize3) -> Self {
        Self {
            width: val.x,
            height: val.y,
            depth: val.z,
        }
    }
}

impl VkFrom<[u32; 3]> for vk::Extent3D {
    #[inline]
    fn from(val: &[u32; 3]) -> Self {
        Self {
            width: val[0],
            height: val[1],
            depth: val[2],
        }
    }
}

impl VkFrom<USize2> for vk::Extent2D {
    #[inline]
    fn from(val: &USize2) -> Self {
        Self {
            width: val.x,
            height: val.y,
        }
    }
}

impl VkFrom<[u32; 2]> for vk::Extent2D {
    #[inline]
    fn from(val: &[u32; 2]) -> Self {
        Self {
            width: val[0],
            height: val[1],
        }
    }
}

impl VkFrom<vk::Extent2D> for USize2 {
    #[inline]
    fn from(val: &vk::Extent2D) -> Self {
        Self::new(val.width, val.height)
    }
}

impl VkFrom<TextureFormat> for vk::Format {
    #[inline]
    fn from(val: &TextureFormat) -> Self {
        match val {
            // 8-bit formats
            TextureFormat::R8Unorm => Self::R8_UNORM,
            TextureFormat::R8Snorm => Self::R8_SNORM,
            TextureFormat::R8Uint => Self::R8_UINT,
            TextureFormat::R8Sint => Self::R8_SINT,

            // 16-bit formats
            TextureFormat::R16Uint => Self::R16_UINT,
            TextureFormat::R16Sint => Self::R16_SINT,
            TextureFormat::R16Float => Self::R16_SFLOAT,
            TextureFormat::Rg8Unorm => Self::R8G8_UNORM,
            TextureFormat::Rg8Snorm => Self::R8G8_SNORM,
            TextureFormat::Rg8Uint => Self::R8G8_UINT,
            TextureFormat::Rg8Sint => Self::R8G8_SINT,

            // 32-bit formats
            TextureFormat::R32Uint => Self::R32_UINT,
            TextureFormat::R32Sint => Self::R32_SINT,
            TextureFormat::R32Float => Self::R32_SFLOAT,
            TextureFormat::Rg16Uint => Self::R16G16_UINT,
            TextureFormat::Rg16Sint => Self::R16G16_SINT,
            TextureFormat::Rg16Float => Self::R16G16_SFLOAT,
            TextureFormat::Rgba8Unorm => Self::R8G8B8A8_UNORM,
            TextureFormat::Rgba8UnormSrgb => Self::R8G8B8A8_SRGB,
            TextureFormat::Rgba8Snorm => Self::R8G8B8A8_SNORM,
            TextureFormat::Rgba8Uint => Self::R8G8B8A8_UINT,
            TextureFormat::Rgba8Sint => Self::R8G8B8A8_SINT,
            TextureFormat::Bgra8Unorm => Self::B8G8R8A8_UNORM,
            TextureFormat::Bgra8UnormSrgb => Self::B8G8R8A8_SRGB,

            // Packed 32-bit formats
            TextureFormat::Rgb9E5Ufloat => Self::E5B9G9R9_UFLOAT_PACK32,
            TextureFormat::Rgb10A2Unorm => Self::A2R10G10B10_UNORM_PACK32,
            TextureFormat::Rg11B10Float => Self::B10G11R11_UFLOAT_PACK32,

            // 64-bit formats
            TextureFormat::Rg32Uint => Self::R32G32_UINT,
            TextureFormat::Rg32Sint => Self::R32G32_SINT,
            TextureFormat::Rg32Float => Self::R32G32_SFLOAT,
            TextureFormat::Rgba16Uint => Self::R16G16B16A16_UINT,
            TextureFormat::Rgba16Sint => Self::R16G16B16A16_SINT,
            TextureFormat::Rgba16Float => Self::R16G16B16A16_SFLOAT,

            // 128-bit formats
            TextureFormat::Rgba32Uint => Self::R32G32B32A32_UINT,
            TextureFormat::Rgba32Sint => Self::R32G32B32A32_SINT,
            TextureFormat::Rgba32Float => Self::R32G32B32A32_SFLOAT,

            // Depth and stencil formats
            TextureFormat::Stencil8 => Self::S8_UINT,
            TextureFormat::Depth16Unorm => Self::D16_UNORM,
            TextureFormat::Depth24Plus => Self::X8_D24_UNORM_PACK32,
            TextureFormat::Depth24PlusStencil8 => Self::D24_UNORM_S8_UINT,
            TextureFormat::Depth32Float => Self::D32_SFLOAT,

            _ => panic!("unsupported texture format: {val:?}"),
        }
    }
}

impl VkFrom<vk::Format> for TextureFormat {
    #[inline]
    fn from(val: &vk::Format) -> Self {
        use vk::Format;
        match *val {
            // 8-bit formats
            Format::R8_UNORM => Self::R8Unorm,
            Format::R8_SNORM => Self::R8Snorm,
            Format::R8_UINT => Self::R8Uint,
            Format::R8_SINT => Self::R8Sint,

            // 16-bit formats
            Format::R16_UINT => Self::R16Uint,
            Format::R16_SINT => Self::R16Sint,
            Format::R16_SFLOAT => Self::R16Float,
            Format::R8G8_UNORM => Self::Rg8Unorm,
            Format::R8G8_SNORM => Self::Rg8Snorm,
            Format::R8G8_UINT => Self::Rg8Uint,
            Format::R8G8_SINT => Self::Rg8Sint,

            // 32-bit formats
            Format::R32_UINT => Self::R32Uint,
            Format::R32_SINT => Self::R32Sint,
            Format::R32_SFLOAT => Self::R32Float,
            Format::R16G16_UINT => Self::Rg16Uint,
            Format::R16G16_SINT => Self::Rg16Sint,
            Format::R16G16_SFLOAT => Self::Rg16Float,
            Format::R8G8B8A8_UNORM => Self::Rgba8Unorm,
            Format::R8G8B8A8_SRGB => Self::Rgba8UnormSrgb,
            Format::R8G8B8A8_SNORM => Self::Rgba8Snorm,
            Format::R8G8B8A8_UINT => Self::Rgba8Uint,
            Format::R8G8B8A8_SINT => Self::Rgba8Sint,
            Format::B8G8R8A8_UNORM => Self::Bgra8Unorm,
            Format::B8G8R8A8_SRGB => Self::Bgra8UnormSrgb,

            // Packed 32-bit formats
            Format::E5B9G9R9_UFLOAT_PACK32 => Self::Rgb9E5Ufloat,
            Format::A2R10G10B10_UNORM_PACK32 => Self::Rgb10A2Unorm,
            Format::B10G11R11_UFLOAT_PACK32 => Self::Rg11B10Float,

            // 64-bit formats
            Format::R32G32_UINT => Self::Rg32Uint,
            Format::R32G32_SINT => Self::Rg32Sint,
            Format::R32G32_SFLOAT => Self::Rg32Float,
            Format::R16G16B16A16_UINT => Self::Rgba16Uint,
            Format::R16G16B16A16_SINT => Self::Rgba16Sint,
            Format::R16G16B16A16_SFLOAT => Self::Rgba16Float,

            // 128-bit formats
            Format::R32G32B32A32_UINT => Self::Rgba32Uint,
            Format::R32G32B32A32_SINT => Self::Rgba32Sint,
            Format::R32G32B32A32_SFLOAT => Self::Rgba32Float,

            // Depth and stencil formats
            Format::S8_UINT => Self::Stencil8,
            Format::D16_UNORM => Self::Depth16Unorm,
            Format::X8_D24_UNORM_PACK32 => Self::Depth24Plus,
            Format::D24_UNORM_S8_UINT => Self::Depth24PlusStencil8,
            Format::D32_SFLOAT => Self::Depth32Float,

            _ => panic!("unsupported texture format: {:x}", val.as_raw()),
        }
    }
}

impl VkFrom<VertexFormat> for vk::Format {
    #[inline]
    fn from(val: &VertexFormat) -> Self {
        match val {
            VertexFormat::Uint8x2 => Self::R8G8_UINT,
            VertexFormat::Uint8x4 => Self::R8G8B8A8_UINT,
            VertexFormat::Sint8x2 => Self::R8G8_SINT,
            VertexFormat::Sint8x4 => Self::R8G8B8A8_SINT,
            VertexFormat::Unorm8x2 => Self::R8G8_UNORM,
            VertexFormat::Unorm8x4 => Self::R8G8B8A8_UNORM,
            VertexFormat::Snorm8x2 => Self::R8G8_SNORM,
            VertexFormat::Snorm8x4 => Self::R8G8B8A8_SNORM,
            VertexFormat::Uint16x2 => Self::R16G16_UINT,
            VertexFormat::Uint16x4 => Self::R16G16B16A16_UINT,
            VertexFormat::Sint16x2 => Self::R16G16_SINT,
            VertexFormat::Sint16x4 => Self::R16G16B16A16_SINT,
            VertexFormat::Unorm16x2 => Self::R16G16_UNORM,
            VertexFormat::Unorm16x4 => Self::R16G16B16A16_UNORM,
            VertexFormat::Snorm16x2 => Self::R16G16_SNORM,
            VertexFormat::Snorm16x4 => Self::R16G16B16A16_SNORM,
            VertexFormat::Uint32 => Self::R32_UINT,
            VertexFormat::Uint32x2 => Self::R32G32_UINT,
            VertexFormat::Uint32x3 => Self::R32G32B32_UINT,
            VertexFormat::Uint32x4 => Self::R32G32B32A32_UINT,
            VertexFormat::Sint32 => Self::R32_SINT,
            VertexFormat::Sint32x2 => Self::R32G32_SINT,
            VertexFormat::Sint32x3 => Self::R32G32B32_SINT,
            VertexFormat::Sint32x4 => Self::R32G32B32A32_SINT,
            VertexFormat::Float16 => Self::R16_SFLOAT,
            VertexFormat::Float16x2 => Self::R16G16_SFLOAT,
            VertexFormat::Float16x4 => Self::R16G16B16A16_SFLOAT,
            VertexFormat::Float32 => Self::R32_SFLOAT,
            VertexFormat::Float32x2 => Self::R32G32_SFLOAT,
            VertexFormat::Float32x3 => Self::R32G32B32_SFLOAT,
            VertexFormat::Float32x4 => Self::R32G32B32A32_SFLOAT,
            VertexFormat::Float64 => Self::R64_SFLOAT,
            VertexFormat::Float64x2 => Self::R64G64_SFLOAT,
            VertexFormat::Float64x3 => Self::R64G64B64_SFLOAT,
            VertexFormat::Float64x4 => Self::R64G64B64A64_SFLOAT,

            _ => panic!("unsupported vertex format: {val:?}"),
        }
    }
}

impl VkFrom<TextureUsage> for vk::ImageUsageFlags {
    fn from(val: &TextureUsage) -> Self {
        let mut result = Self::empty();
        if val.contains(TextureUsage::TRANSFER_SRC) {
            result |= Self::TRANSFER_SRC;
        }
        if val.contains(TextureUsage::TRANSFER_DST) {
            result |= Self::TRANSFER_DST;
        }
        if val.contains(TextureUsage::TEXTURE_BINDING) {
            result |= Self::SAMPLED;
        }
        if val.contains(TextureUsage::STORAGE_BINDING) {
            result |= Self::STORAGE;
        }
        if val.contains(TextureUsage::COLOR_ATTACHMENT) {
            result |= Self::COLOR_ATTACHMENT;
        }
        if val.contains(TextureUsage::DEPTH_STENCIL_ATTACHMENT) {
            result |= Self::DEPTH_STENCIL_ATTACHMENT;
        }
        if val.contains(TextureUsage::INPUT_ATTACHMENT) {
            result |= Self::INPUT_ATTACHMENT;
        }
        result
    }
}

pub fn into_texture_usage_read_access(usage: TextureUsage) -> vk::AccessFlags {
    let mut result = vk::AccessFlags::empty();
    if usage.contains(TextureUsage::TRANSFER_SRC) | usage.contains(TextureUsage::TRANSFER_DST) {
        result |= vk::AccessFlags::TRANSFER_READ;
    }
    if usage.contains(TextureUsage::TEXTURE_BINDING)
        || usage.contains(TextureUsage::STORAGE_BINDING)
    {
        result |= vk::AccessFlags::SHADER_READ;
    }
    if usage.contains(TextureUsage::COLOR_ATTACHMENT) {
        result |= vk::AccessFlags::COLOR_ATTACHMENT_READ;
    }
    if usage.contains(TextureUsage::DEPTH_STENCIL_ATTACHMENT) {
        result |= vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ;
    }
    if usage.contains(TextureUsage::INPUT_ATTACHMENT) {
        result |= vk::AccessFlags::INPUT_ATTACHMENT_READ;
    }
    result
}

pub fn into_texture_usage_write_access(usage: TextureUsage) -> vk::AccessFlags {
    let mut result = vk::AccessFlags::empty();
    if usage.contains(TextureUsage::TRANSFER_SRC) | usage.contains(TextureUsage::TRANSFER_DST) {
        result |= vk::AccessFlags::TRANSFER_WRITE;
    }
    if usage.contains(TextureUsage::TEXTURE_BINDING)
        || usage.contains(TextureUsage::STORAGE_BINDING)
    {
        result |= vk::AccessFlags::SHADER_WRITE;
    }
    if usage.contains(TextureUsage::COLOR_ATTACHMENT) {
        result |= vk::AccessFlags::COLOR_ATTACHMENT_WRITE;
    }
    if usage.contains(TextureUsage::DEPTH_STENCIL_ATTACHMENT) {
        result |= vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE;
    }
    result
}

pub fn into_buffer_usage_read_access(usage: BufferUsage) -> vk::AccessFlags {
    let mut result = vk::AccessFlags::empty();
    if usage.contains(BufferUsage::MAP_READ) | usage.contains(BufferUsage::MAP_WRITE) {
        result |= vk::AccessFlags::HOST_READ;
    }
    if usage.contains(BufferUsage::TRANSFER_SRC) | usage.contains(BufferUsage::TRANSFER_DST) {
        result |= vk::AccessFlags::TRANSFER_READ;
    }
    if usage.contains(BufferUsage::INDEX) {
        result |= vk::AccessFlags::INDEX_READ;
    }
    if usage.contains(BufferUsage::VERTEX) {
        result |= vk::AccessFlags::VERTEX_ATTRIBUTE_READ;
    }
    if usage.contains(BufferUsage::UNIFORM) {
        result |= vk::AccessFlags::UNIFORM_READ;
    }
    if usage.contains(BufferUsage::STORAGE) {
        result |= vk::AccessFlags::SHADER_READ;
    }
    if usage.contains(BufferUsage::INDIRECT) {
        result |= vk::AccessFlags::INDIRECT_COMMAND_READ;
    }
    result
}

pub fn into_buffer_usage_write_access(usage: BufferUsage) -> vk::AccessFlags {
    let mut result = vk::AccessFlags::empty();
    if usage.contains(BufferUsage::MAP_READ) | usage.contains(BufferUsage::MAP_WRITE) {
        result |= vk::AccessFlags::HOST_WRITE;
    }
    if usage.contains(BufferUsage::TRANSFER_SRC) | usage.contains(BufferUsage::TRANSFER_DST) {
        result |= vk::AccessFlags::TRANSFER_WRITE;
    }
    if usage.contains(BufferUsage::STORAGE) {
        result |= vk::AccessFlags::SHADER_WRITE;
    }
    result
}

impl VkFrom<PipelineBindPoint> for vk::PipelineBindPoint {
    #[inline]
    fn from(val: &PipelineBindPoint) -> Self {
        match val {
            PipelineBindPoint::Graphics => Self::GRAPHICS,
            PipelineBindPoint::Compute => Self::COMPUTE,
            PipelineBindPoint::RayTracing => Self::RAY_TRACING_KHR,
        }
    }
}

impl VkFrom<Stage> for vk::PipelineStageFlags {
    #[inline]
    fn from(val: &Stage) -> Self {
        let mut result = Self::empty();
        if val.contains(Stage::DRAW_INDIRECT) {
            result |= Self::DRAW_INDIRECT;
        }
        if val.contains(Stage::VERTEX_INPUT) {
            result |= Self::VERTEX_INPUT;
        }
        if val.contains(Stage::VERTEX_SHADER) {
            result |= Self::VERTEX_SHADER;
        }
        if val.contains(Stage::TESSELLATION_CONTROL_SHADER) {
            result |= Self::TESSELLATION_CONTROL_SHADER;
        }
        if val.contains(Stage::TESSELLATION_EVALUATION_SHADER) {
            result |= Self::TESSELLATION_EVALUATION_SHADER;
        }
        if val.contains(Stage::GEOMETRY_SHADER) {
            result |= Self::GEOMETRY_SHADER;
        }
        if val.contains(Stage::FRAGMENT_SHADER) {
            result |= Self::FRAGMENT_SHADER;
        }
        if val.contains(Stage::EARLY_FRAGMENT_TESTS) {
            result |= Self::EARLY_FRAGMENT_TESTS;
        }
        if val.contains(Stage::LATE_FRAGMENT_TESTS) {
            result |= Self::LATE_FRAGMENT_TESTS;
        }
        if val.contains(Stage::COLOR_ATTACHMENT_OUTPUT) {
            result |= Self::COLOR_ATTACHMENT_OUTPUT;
        }
        if val.contains(Stage::COMPUTE_SHADER) {
            result |= Self::COMPUTE_SHADER;
        }
        if val.contains(Stage::ACCELERATION_STRUCTURE_BUILD) {
            result |= Self::ACCELERATION_STRUCTURE_BUILD_KHR;
        }
        if val.contains(Stage::RAY_TRACING_SHADER) {
            result |= Self::RAY_TRACING_SHADER_KHR;
        }
        if val.contains(Stage::TRANSFER) {
            result |= Self::TRANSFER;
        }
        if val.contains(Stage::HOST) {
            result |= Self::HOST;
        }
        result
    }
}

impl VkFrom<PrimitiveState> for vk::PipelineInputAssemblyStateCreateInfo {
    #[inline]
    fn from(val: &PrimitiveState) -> Self {
        Self::builder().topology(val.topology.vk_into()).build()
    }
}

impl VkFrom<PrimitiveState> for vk::PipelineRasterizationStateCreateInfo {
    #[inline]
    fn from(val: &PrimitiveState) -> Self {
        let mut builder = Self::builder()
            .polygon_mode(vk::PolygonMode::FILL)
            .front_face(val.front_face.vk_into())
            .line_width(1.0);
        if let Some(cull_mode) = val.cull_mode {
            builder = builder.cull_mode(cull_mode.vk_into())
        }
        builder.build()
    }
}

impl VkFrom<DepthStencilState> for vk::PipelineDepthStencilStateCreateInfo {
    #[inline]
    fn from(val: &DepthStencilState) -> Self {
        let mut builder = Self::builder();
        if val.is_depth_enabled() {
            builder = builder
                .depth_test_enable(true)
                .depth_write_enable(val.depth.write_enabled)
                .depth_compare_op(val.depth.compare.vk_into());
        }
        if val.stencil.is_enabled() {
            builder = builder
                .stencil_test_enable(true)
                .front(val.stencil.front.vk_into())
                .back(val.stencil.back.vk_into());
        }
        builder.build()
    }
}

impl VkFrom<StencilFaceState> for vk::StencilOpState {
    #[inline]
    fn from(val: &StencilFaceState) -> Self {
        Self {
            compare_op: val.compare.vk_into(),
            fail_op: val.fail_op.vk_into(),
            depth_fail_op: val.depth_fail_op.vk_into(),
            pass_op: val.pass_op.vk_into(),
            compare_mask: !0,
            write_mask: !0,
            reference: !0,
        }
    }
}

impl VkFrom<PrimitiveTopology> for vk::PrimitiveTopology {
    #[inline]
    fn from(val: &PrimitiveTopology) -> Self {
        match val {
            PrimitiveTopology::PointList => Self::POINT_LIST,
            PrimitiveTopology::LineList => Self::LINE_LIST,
            PrimitiveTopology::LineStrip => Self::LINE_STRIP,
            PrimitiveTopology::TriangleList => Self::TRIANGLE_LIST,
            PrimitiveTopology::TriangleStrip => Self::TRIANGLE_STRIP,
        }
    }
}

impl VkFrom<FrontFace> for vk::FrontFace {
    #[inline]
    fn from(val: &FrontFace) -> Self {
        match val {
            FrontFace::CounterClockwise => Self::COUNTER_CLOCKWISE,
            FrontFace::Clockwise => Self::CLOCKWISE,
        }
    }
}

impl VkFrom<Face> for vk::CullModeFlags {
    #[inline]
    fn from(val: &Face) -> Self {
        match val {
            Face::Front => Self::FRONT,
            Face::Back => Self::BACK,
        }
    }
}

impl VkFrom<BlendOperation> for vk::BlendOp {
    #[inline]
    fn from(val: &BlendOperation) -> Self {
        match val {
            BlendOperation::Add => Self::ADD,
            BlendOperation::Subtract => Self::SUBTRACT,
            BlendOperation::ReverseSubtract => Self::REVERSE_SUBTRACT,
            BlendOperation::Min => Self::MIN,
            BlendOperation::Max => Self::MAX,
        }
    }
}

impl VkFrom<BlendFactor> for vk::BlendFactor {
    #[inline]
    fn from(val: &BlendFactor) -> Self {
        match val {
            BlendFactor::Zero => Self::ZERO,
            BlendFactor::One => Self::ONE,
            BlendFactor::Src => Self::SRC_COLOR,
            BlendFactor::OneMinusSrc => Self::ONE_MINUS_SRC_COLOR,
            BlendFactor::SrcAlpha => Self::SRC_ALPHA,
            BlendFactor::OneMinusSrcAlpha => Self::ONE_MINUS_SRC_ALPHA,
            BlendFactor::Dst => Self::DST_COLOR,
            BlendFactor::OneMinusDst => Self::ONE_MINUS_DST_COLOR,
            BlendFactor::DstAlpha => Self::DST_ALPHA,
            BlendFactor::OneMinusDstAlpha => Self::ONE_MINUS_DST_ALPHA,
            BlendFactor::SrcAlphaSaturated => Self::SRC_ALPHA_SATURATE,
            BlendFactor::Constant => Self::CONSTANT_COLOR,
            BlendFactor::OneMinusConstant => Self::ONE_MINUS_CONSTANT_COLOR,
        }
    }
}

impl VkFrom<IndexFormat> for vk::IndexType {
    #[inline]
    fn from(val: &IndexFormat) -> Self {
        match val {
            IndexFormat::Uint16 => Self::UINT16,
            IndexFormat::Uint32 => Self::UINT32,
        }
    }
}

impl VkFrom<CompareFunction> for vk::CompareOp {
    #[inline]
    fn from(val: &CompareFunction) -> Self {
        match val {
            CompareFunction::Never => Self::NEVER,
            CompareFunction::Less => Self::LESS,
            CompareFunction::Equal => Self::EQUAL,
            CompareFunction::LessEqual => Self::LESS_OR_EQUAL,
            CompareFunction::Greater => Self::GREATER,
            CompareFunction::NotEqual => Self::NOT_EQUAL,
            CompareFunction::GreaterEqual => Self::GREATER_OR_EQUAL,
            CompareFunction::Always => Self::ALWAYS,
        }
    }
}

impl VkFrom<StencilOperation> for vk::StencilOp {
    #[inline]
    fn from(val: &StencilOperation) -> Self {
        match val {
            StencilOperation::Keep => Self::KEEP,
            StencilOperation::Zero => Self::ZERO,
            StencilOperation::Replace => Self::REPLACE,
            StencilOperation::Invert => Self::INVERT,
            StencilOperation::IncrementClamp => Self::INCREMENT_AND_CLAMP,
            StencilOperation::DecrementClamp => Self::DECREMENT_AND_CLAMP,
            StencilOperation::IncrementWrap => Self::INCREMENT_AND_WRAP,
            StencilOperation::DecrementWrap => Self::DECREMENT_AND_WRAP,
        }
    }
}

pub struct CreateInfoBuffer<T = u8> {
    buf: *mut u8,
    len_bytes: usize,
    capacity: usize,
    _phantom: PhantomData<T>,
}

impl CreateInfoBuffer {
    #[inline]
    pub const fn new() -> Self {
        Self {
            buf: std::ptr::null_mut(),
            len_bytes: 0,
            capacity: 0,
            _phantom: PhantomData,
        }
    }

    fn reset_as<T: Copy>(&mut self) -> &mut CreateInfoBuffer<T> {
        assert!(std::mem::align_of::<T>() <= std::mem::align_of::<usize>());
        assert_ne!(0, std::mem::size_of::<T>());
        self.len_bytes = 0;
        unsafe { std::mem::transmute(self) }
    }
}

impl<T: Copy> CreateInfoBuffer<T> {
    #[inline]
    pub fn len(&self) -> usize {
        self.len_bytes / std::mem::size_of::<T>()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len_bytes == 0
    }

    pub fn reserve(&mut self, _num_items: usize) {
        let value_len = std::mem::size_of::<T>();
        let new_len = self.len_bytes + value_len * 2;
        if new_len > self.capacity {
            let new_capacity = new_len.next_power_of_two();
            unsafe {
                if self.buf.is_null() {
                    let layout = std::alloc::Layout::from_size_align(
                        new_capacity,
                        std::mem::align_of::<usize>(),
                    )
                    .unwrap();
                    self.buf = std::alloc::alloc(layout);
                } else {
                    let layout = std::alloc::Layout::from_size_align(
                        self.capacity,
                        std::mem::align_of::<usize>(),
                    )
                    .unwrap();
                    self.buf = std::alloc::realloc(self.buf, layout, new_capacity)
                }
            }
            self.capacity = new_capacity;
        }
    }

    pub fn push(&mut self, value: T) -> &mut T {
        self.reserve(1);
        let value_len = std::mem::size_of::<T>();
        unsafe {
            let ptr = self.buf.add(self.len_bytes) as *mut T;
            std::ptr::write(ptr, value);
            self.len_bytes += value_len;
            &mut *ptr
        }
    }

    pub fn as_slice(&self) -> &[T] {
        let len = self.len();
        if len == 0 {
            return &[];
        }
        unsafe {
            let ptr = self.buf as *const T;
            std::slice::from_raw_parts(ptr, len)
        }
    }
    pub fn as_slice_mut(&mut self) -> &mut [T] {
        let len = self.len();
        if len == 0 {
            return &mut [];
        }
        unsafe {
            let ptr = self.buf as *mut T;
            std::slice::from_raw_parts_mut(ptr, len)
        }
    }
}

impl<T> Drop for CreateInfoBuffer<T> {
    fn drop(&mut self) {
        if !self.buf.is_null() {
            let layout =
                std::alloc::Layout::from_size_align(self.capacity, std::mem::align_of::<usize>())
                    .unwrap();
            unsafe {
                std::alloc::dealloc(self.buf, layout);
            }
            self.buf = std::ptr::null_mut();
        }
    }
}

pub struct CreateInfoConverter(CreateInfoBuffer, CreateInfoBuffer);

impl CreateInfoConverter {
    #[inline]
    pub const fn new() -> Self {
        Self(CreateInfoBuffer::new(), CreateInfoBuffer::new())
    }

    pub fn bind_group_layout(
        &mut self,
        desc: &BindGroupLayoutDescriptor<'_>,
    ) -> &vk::DescriptorSetLayoutCreateInfo {
        let buf0 = self.0.reset_as::<vk::DescriptorSetLayoutBinding>();
        buf0.reserve(desc.entries.len());
        for e in desc.entries {
            buf0.push(
                vk::DescriptorSetLayoutBinding::builder()
                    .binding(e.binding)
                    .descriptor_count(e.count)
                    .build(),
            );
            // TODO: descriptor_type, stage_flags, immutable_samplers
            todo!();
        }

        let buf = self.1.reset_as::<vk::DescriptorSetLayoutCreateInfo>();
        buf.reserve(1);
        buf.push(
            vk::DescriptorSetLayoutCreateInfo::builder()
                .bindings(buf0.as_slice())
                .build(),
        );
        &buf.as_slice()[0]
    }

    pub fn pipeline_layout(
        &mut self,
        res: &AshResources,
        desc: &PipelineLayoutDescriptor<'_>,
    ) -> &vk::PipelineLayoutCreateInfo {
        let buf0 = self.0.reset_as::<vk::DescriptorSetLayout>();
        buf0.reserve(desc.bind_group_layouts.len());
        for bgl in desc.bind_group_layouts {
            buf0.push(res.bind_group_layouts[*bgl]);
        }

        let buf = self.1.reset_as::<vk::PipelineLayoutCreateInfo>();
        buf.reserve(1);
        buf.push(
            vk::PipelineLayoutCreateInfo::builder()
                .set_layouts(buf0.as_slice())
                .build(),
        );
        &buf.as_slice()[0]
    }

    pub fn graphics_pipeline_descriptor(
        &mut self,
        res: &AshResources,
        descs: &[GraphicsPipelineDescriptor<'_>],
    ) -> &[vk::GraphicsPipelineCreateInfo] {
        let buf = self.0.reset_as::<vk::GraphicsPipelineCreateInfo>();
        buf.reserve(descs.len());
        for desc in descs {
            let layout = desc
                .layout
                .map_or(vk::PipelineLayout::null(), |l| res.pipeline_layouts[l]);
            buf.push(
                vk::GraphicsPipelineCreateInfo::builder()
                    .layout(layout)
                    .build(),
            );
            // TODO: vertex, primitive, depth_stencil, fragment, samples, specialization
            todo!(" implement graphics_pipeline_descriptor");
        }
        buf.as_slice()
    }

    pub fn compute_pipeline_descriptor(
        &mut self,
        res: &AshResources,
        descs: &[ComputePipelineDescriptor<'_>],
    ) -> &[vk::ComputePipelineCreateInfo] {
        let buf = self.0.reset_as::<vk::ComputePipelineCreateInfo>();
        buf.reserve(descs.len());
        for desc in descs {
            let layout = desc
                .layout
                .map_or(vk::PipelineLayout::null(), |l| res.pipeline_layouts[l]);
            // TODO: module, entry_point, specialization
            buf.push(
                vk::ComputePipelineCreateInfo::builder()
                    .layout(layout)
                    .build(),
            );
            todo!(" implement compute_pipeline_descriptor");
        }
        buf.as_slice()
    }

    pub fn ray_tracing_pipeline_descriptor(
        &mut self,
        res: &AshResources,
        descs: &[RayTracingPipelineDescriptor<'_>],
    ) -> &[vk::RayTracingPipelineCreateInfoKHR] {
        let buf = self.0.reset_as::<vk::RayTracingPipelineCreateInfoKHR>();
        buf.reserve(descs.len());
        for desc in descs {
            let layout = desc
                .layout
                .map_or(vk::PipelineLayout::null(), |l| res.pipeline_layouts[l]);
            // TODO: modules, groups, specialization
            buf.push(
                vk::RayTracingPipelineCreateInfoKHR::builder()
                    .layout(layout)
                    .max_pipeline_ray_recursion_depth(desc.max_recursion_depth)
                    .build(),
            );
            todo!(" implement ray_tracing_pipeline_descriptor");
        }
        buf.as_slice()
    }
}
