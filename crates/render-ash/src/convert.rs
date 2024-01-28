use ash::vk;
use pulz_bitset::BitSet;
use pulz_render::{
    buffer::{BufferDescriptor, BufferUsage},
    graph::{
        access::{Access, Stage},
        pass::PipelineBindPoint,
    },
    math::{USize2, USize3},
    pipeline::{
        BindGroupLayoutDescriptor, BlendFactor, BlendOperation, CompareFunction,
        ComputePipelineDescriptor, DepthStencilState, Face, FrontFace, GraphicsPassDescriptor,
        GraphicsPipelineDescriptor, IndexFormat, LoadOp, PipelineLayoutDescriptor, PrimitiveState,
        PrimitiveTopology, RayTracingPipelineDescriptor, StencilFaceState, StencilOperation,
        StoreOp, VertexFormat,
    },
    texture::{TextureAspects, TextureDescriptor, TextureDimensions, TextureFormat, TextureUsage},
};
use scratchbuffer::ScratchBuffer;

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
        if val.intersects(BufferUsage::INDIRECT) {
            result |= Self::INDIRECT_BUFFER;
        }
        if val.intersects(BufferUsage::INDEX) {
            result |= Self::INDEX_BUFFER;
        }
        if val.intersects(BufferUsage::VERTEX) {
            result |= Self::VERTEX_BUFFER;
        }
        if val.intersects(BufferUsage::UNIFORM) {
            result |= Self::UNIFORM_BUFFER;
        }
        if val.intersects(BufferUsage::STORAGE) {
            result |= Self::STORAGE_BUFFER;
        }
        if val.intersects(BufferUsage::UNIFORM_TEXEL) {
            result |= Self::UNIFORM_TEXEL_BUFFER;
        }
        if val.intersects(BufferUsage::STORAGE_TEXEL) {
            result |= Self::STORAGE_TEXEL_BUFFER;
        }
        if val.intersects(BufferUsage::TRANSFER_READ) {
            result |= Self::TRANSFER_SRC;
        }
        if val.intersects(BufferUsage::TRANSFER_WRITE) {
            result |= Self::TRANSFER_DST;
        }
        if val.intersects(BufferUsage::ACCELERATION_STRUCTURE_STORAGE) {
            result |= Self::ACCELERATION_STRUCTURE_STORAGE_KHR;
        }
        if val.intersects(BufferUsage::ACCELERATION_STRUCTURE_BUILD_INPUT) {
            result |= Self::ACCELERATION_STRUCTURE_BUILD_INPUT_READ_ONLY_KHR;
        }
        if val.intersects(BufferUsage::SHADER_BINDING_TABLE) {
            result |= Self::SHADER_BINDING_TABLE_KHR;
        }
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
            .samples(vk::SampleCountFlags::from_raw(val.sample_count as u32))
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

pub fn default_clear_value_for_format(format: vk::Format) -> vk::ClearValue {
    match format {
        // Depth and stencil formats
        vk::Format::S8_UINT
        | vk::Format::D16_UNORM
        | vk::Format::X8_D24_UNORM_PACK32
        | vk::Format::D24_UNORM_S8_UINT
        | vk::Format::D32_SFLOAT => vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue {
                depth: 1.0,
                stencil: 0,
            },
        },

        _ => vk::ClearValue {
            color: default_clear_color_value_for_format(format),
        },
    }
}

pub fn default_clear_color_value_for_format(format: vk::Format) -> vk::ClearColorValue {
    match format {
        vk::Format::R8_SINT
        | vk::Format::R8G8_SINT
        | vk::Format::R8G8B8_SINT
        | vk::Format::B8G8R8_SINT
        | vk::Format::R8G8B8A8_SINT
        | vk::Format::B8G8R8A8_SINT
        | vk::Format::A8B8G8R8_SINT_PACK32
        | vk::Format::A2R10G10B10_SINT_PACK32
        | vk::Format::A2B10G10R10_SINT_PACK32
        | vk::Format::R16_SINT
        | vk::Format::R16G16_SINT
        | vk::Format::R16G16B16_SINT
        | vk::Format::R16G16B16A16_SINT
        | vk::Format::R32_SINT
        | vk::Format::R32G32_SINT
        | vk::Format::R32G32B32_SINT
        | vk::Format::R32G32B32A32_SINT
        | vk::Format::R64_SINT
        | vk::Format::R64G64_SINT
        | vk::Format::R64G64B64_SINT
        | vk::Format::R64G64B64A64_SINT => vk::ClearColorValue {
            int32: [i32::MIN, i32::MIN, i32::MIN, i32::MAX],
        },

        vk::Format::R8_UINT
        | vk::Format::R8G8_UINT
        | vk::Format::R8G8B8_UINT
        | vk::Format::B8G8R8_UINT
        | vk::Format::R8G8B8A8_UINT
        | vk::Format::B8G8R8A8_UINT
        | vk::Format::A8B8G8R8_UINT_PACK32
        | vk::Format::A2R10G10B10_UINT_PACK32
        | vk::Format::A2B10G10R10_UINT_PACK32
        | vk::Format::R16_UINT
        | vk::Format::R16G16_UINT
        | vk::Format::R16G16B16_UINT
        | vk::Format::R16G16B16A16_UINT
        | vk::Format::R32_UINT
        | vk::Format::R32G32_UINT
        | vk::Format::R32G32B32_UINT
        | vk::Format::R32G32B32A32_UINT
        | vk::Format::R64_UINT
        | vk::Format::R64G64_UINT
        | vk::Format::R64G64B64_UINT
        | vk::Format::R64G64B64A64_UINT => vk::ClearColorValue {
            uint32: [0, 0, 0, u32::MAX],
        },

        _ => vk::ClearColorValue {
            float32: [0.0, 0.0, 0.0, 1.0],
        },
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
        if val.intersects(TextureUsage::INPUT_ATTACHMENT) {
            result |= Self::INPUT_ATTACHMENT;
        }
        if val.intersects(TextureUsage::COLOR_ATTACHMENT) {
            result |= Self::COLOR_ATTACHMENT;
        }
        if val.intersects(TextureUsage::DEPTH_STENCIL_ATTACHMENT) {
            result |= Self::DEPTH_STENCIL_ATTACHMENT;
        }
        if val.intersects(TextureUsage::TRANSFER_READ) {
            result |= Self::TRANSFER_SRC;
        }
        if val.intersects(TextureUsage::TRANSFER_WRITE) {
            result |= Self::TRANSFER_DST;
        }
        if val.intersects(TextureUsage::SAMPLED) {
            result |= Self::SAMPLED;
        }
        if val.intersects(TextureUsage::STORAGE) {
            result |= Self::STORAGE;
        }
        result
    }
}

impl VkFrom<Access> for vk::ImageLayout {
    #[inline]
    fn from(val: &Access) -> Self {
        let mut num = 0;
        let mut r = Self::UNDEFINED;
        if val.intersects(Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE) {
            r = Self::COLOR_ATTACHMENT_OPTIMAL;
            num += 1;
        }
        if val.intersects(Access::DEPTH_STENCIL_ATTACHMENT_WRITE) {
            r = Self::DEPTH_STENCIL_ATTACHMENT_OPTIMAL;
            // TODO: single write variants
            num += 1;
        } else if val.intersects(Access::DEPTH_STENCIL_ATTACHMENT_READ) {
            r = Self::DEPTH_STENCIL_READ_ONLY_OPTIMAL;
            num += 1;
        }
        if val.intersects(Access::TRANSFER_READ) {
            r = Self::TRANSFER_SRC_OPTIMAL;
            num += 1;
        }
        if val.intersects(Access::TRANSFER_WRITE) {
            r = Self::TRANSFER_DST_OPTIMAL;
            num += 1;
        }
        if val.intersects(Access::PRESENT) {
            r = Self::PRESENT_SRC_KHR;
            num += 1;
        }
        if num <= 1 {
            return r;
        }
        if !val.intersects(Access::ANY_WRITE) {
            Self::READ_ONLY_OPTIMAL
        } else {
            Self::GENERAL
        }
    }
}

impl VkFrom<Access> for vk::AccessFlags {
    #[inline]
    fn from(val: &Access) -> Self {
        let mut result = Self::empty();
        if *val == Access::GENERAL {
            return Self::MEMORY_READ | Self::MEMORY_WRITE;
        } else if *val == Access::MEMORY_READ {
            return Self::MEMORY_READ;
        } else if *val == Access::MEMORY_WRITE {
            return Self::MEMORY_WRITE;
        }
        if val.intersects(Access::INDIRECT_COMMAND_READ) {
            result |= Self::INDIRECT_COMMAND_READ;
        }
        if val.intersects(Access::INDEX_READ) {
            result |= Self::INDEX_READ;
        }
        if val.intersects(Access::VERTEX_ATTRIBUTE_READ) {
            result |= Self::VERTEX_ATTRIBUTE_READ;
        }
        if val.intersects(
            Access::COLOR_INPUT_ATTACHMENT_READ | Access::DEPTH_STENCIL_INPUT_ATTACHMENT_READ,
        ) {
            result |= Self::INPUT_ATTACHMENT_READ;
        }
        if val.intersects(Access::UNIFORM_READ) {
            result |= Self::UNIFORM_READ;
        }
        if val.intersects(Access::SAMPLED_READ | Access::SAMPLED_READ) {
            result |= Self::SHADER_READ;
        }
        if val.intersects(Access::COLOR_ATTACHMENT_READ) {
            result |= Self::COLOR_ATTACHMENT_READ;
        }
        if val.intersects(Access::COLOR_ATTACHMENT_WRITE) {
            result |= Self::COLOR_ATTACHMENT_WRITE;
        }
        if val.intersects(Access::DEPTH_STENCIL_ATTACHMENT_READ) {
            result |= Self::DEPTH_STENCIL_ATTACHMENT_READ;
        }
        if val.intersects(Access::DEPTH_STENCIL_ATTACHMENT_WRITE) {
            result |= Self::DEPTH_STENCIL_ATTACHMENT_WRITE;
        }
        if val.intersects(Access::TRANSFER_READ) {
            result |= Self::TRANSFER_READ;
        }
        if val.intersects(Access::TRANSFER_WRITE) {
            result |= Self::TRANSFER_WRITE;
        }
        if val.intersects(Access::HOST_READ) {
            result |= Self::HOST_READ;
        }
        if val.intersects(Access::HOST_WRITE) {
            result |= Self::HOST_WRITE;
        }
        if val.intersects(
            Access::ACCELERATION_STRUCTURE_READ | Access::ACCELERATION_STRUCTURE_BUILD_READ,
        ) {
            result |= Self::ACCELERATION_STRUCTURE_READ_KHR;
        }
        if val.intersects(Access::ACCELERATION_STRUCTURE_BUILD_WRITE) {
            result |= Self::ACCELERATION_STRUCTURE_WRITE_KHR;
        }
        result
    }
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

impl VkFrom<LoadOp> for vk::AttachmentLoadOp {
    #[inline]
    fn from(val: &LoadOp) -> Self {
        match val {
            LoadOp::Load => Self::LOAD,
            LoadOp::Clear => Self::CLEAR,
            LoadOp::DontCare => Self::DONT_CARE,
        }
    }
}

impl VkFrom<StoreOp> for vk::AttachmentStoreOp {
    #[inline]
    fn from(val: &StoreOp) -> Self {
        match val {
            StoreOp::Store => Self::STORE,
            StoreOp::DontCare => Self::DONT_CARE,
        }
    }
}

pub struct CreateInfoConverter6(
    ScratchBuffer,
    ScratchBuffer,
    ScratchBuffer,
    ScratchBuffer,
    ScratchBuffer,
    ScratchBuffer,
);

pub struct CreateInfoConverter2(ScratchBuffer, ScratchBuffer);

fn get_or_create_subpass_dep(
    buf: &mut ScratchBuffer<vk::SubpassDependency>,
    src: u32,
    dst: u32,
    src_access_mask: vk::AccessFlags,
) -> &mut vk::SubpassDependency {
    buf.binary_search_insert_by_key_with(
        &(src, dst),
        |d| (d.src_subpass, d.dst_subpass),
        || {
            vk::SubpassDependency::builder()
                .src_subpass(src)
                .dst_subpass(dst)
                .src_stage_mask(
                    if src_access_mask.contains(vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE) {
                        vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                            | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS
                    } else {
                        vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    },
                )
                .src_access_mask(src_access_mask)
                // use BY-REGION by default
                .dependency_flags(vk::DependencyFlags::BY_REGION)
                .build()
        },
    )
}

impl CreateInfoConverter6 {
    #[inline]
    pub const fn new() -> Self {
        Self(
            ScratchBuffer::new(),
            ScratchBuffer::new(),
            ScratchBuffer::new(),
            ScratchBuffer::new(),
            ScratchBuffer::new(),
            ScratchBuffer::new(),
        )
    }

    pub fn graphics_pass(&mut self, desc: &GraphicsPassDescriptor) -> &vk::RenderPassCreateInfo {
        // collect attachments
        let attachments = self.0.clear_and_use_as::<vk::AttachmentDescription>();
        let num_attachments = desc.attachments().len();
        attachments.reserve(num_attachments);
        let mut attachment_dep_data = Vec::with_capacity(num_attachments);
        for (i, a) in desc.attachments().iter().enumerate() {
            let load_store_ops = desc.load_store_ops().get(i).copied().unwrap_or_default();
            attachments.push(
                vk::AttachmentDescription::builder()
                    .format(a.format.vk_into())
                    .samples(vk::SampleCountFlags::from_raw(a.samples as u32))
                    .load_op(load_store_ops.load_op.vk_into())
                    .store_op(load_store_ops.store_op.vk_into())
                    .stencil_load_op(load_store_ops.load_op.vk_into())
                    .stencil_store_op(load_store_ops.store_op.vk_into())
                    .initial_layout(if load_store_ops.load_op == LoadOp::Load {
                        a.initial_access.vk_into()
                    } else {
                        vk::ImageLayout::UNDEFINED
                    })
                    .final_layout(a.final_access.vk_into())
                    .build(),
            );
            attachment_dep_data.push((vk::SUBPASS_EXTERNAL, vk::AccessFlags::NONE));
        }

        // calculate subpass deps
        let sp_deps = self.1.clear_and_use_as::<vk::SubpassDependency>();
        let a_refs = self.2.clear_and_use_as::<vk::AttachmentReference>();
        let num_subpasses = desc.subpasses().len();
        let mut attachment_usage = BitSet::with_capacity_for(num_attachments * num_subpasses);
        for (i, sp) in desc.subpasses().iter().enumerate() {
            // TODO: handle Write>Read>Write!
            // TODO: also non-attachment dubpass-deps
            let dst = i as u32;
            for &(a, _u) in sp.input_attachments() {
                let a = a as usize;
                let (src, src_access) = attachment_dep_data[a];
                if src != vk::SUBPASS_EXTERNAL {
                    let dep = get_or_create_subpass_dep(sp_deps, src, dst, src_access);
                    dep.dst_stage_mask |= vk::PipelineStageFlags::FRAGMENT_SHADER;
                    dep.dst_access_mask |= vk::AccessFlags::INPUT_ATTACHMENT_READ;
                }
                attachment_usage.insert(i * num_attachments + a);

                a_refs.push(vk::AttachmentReference {
                    attachment: a as u32, // if a==!0 => vk::ATTACHMENT_UNUSED
                    //layout: u.vk_into(),
                    layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                });
            }
            for &(a, _u) in sp.color_attachments() {
                let a = a as usize;
                let (src, src_access) = attachment_dep_data[a];
                if src != vk::SUBPASS_EXTERNAL {
                    let dep = get_or_create_subpass_dep(sp_deps, src, dst, src_access);
                    dep.dst_stage_mask |= vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT;
                    dep.dst_access_mask |= vk::AccessFlags::COLOR_ATTACHMENT_READ;
                }
                attachment_dep_data[a] = (dst, vk::AccessFlags::COLOR_ATTACHMENT_WRITE);
                attachment_usage.insert(i * num_attachments + a);

                //attachments[a].final_layout = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
                a_refs.push(vk::AttachmentReference {
                    attachment: a as u32, // if a==!0 => vk::ATTACHMENT_UNUSED
                    //layout: u.vk_into(),
                    layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                });
            }
            if let Some((a, _u)) = sp.depth_stencil_attachment() {
                let a = a as usize;
                let (src, src_access) = attachment_dep_data[a];
                if src != vk::SUBPASS_EXTERNAL {
                    let dep = get_or_create_subpass_dep(sp_deps, src, dst, src_access);
                    dep.dst_stage_mask |= vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                        | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;
                    dep.dst_access_mask |= vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ;
                }
                attachment_dep_data[a] = (dst, vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE);
                attachment_usage.insert(i * num_attachments + a);

                //attachments[a].final_layout = vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL;
                a_refs.push(vk::AttachmentReference {
                    attachment: a as u32,
                    //layout: u.vk_into(),
                    layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                });
            }
        }
        drop(attachment_dep_data);

        // preserve attachment
        let mut a_preserve_tmp: Vec<Vec<u32>> = Vec::new();
        a_preserve_tmp.resize_with(desc.subpasses().len(), Vec::new);
        loop {
            let mut changed = false;
            for dep in sp_deps.iter() {
                if dep.src_subpass == vk::SUBPASS_EXTERNAL
                    || dep.dst_subpass == vk::SUBPASS_EXTERNAL
                {
                    continue;
                }
                let src = dep.src_subpass as usize;
                let dst = dep.dst_subpass as usize;
                // There is a subpass dependency from S1 (`src`) to S (`dst`).
                let a_start = src * num_subpasses;
                let a_preserve_tmp_offset = a_preserve_tmp[dst].len();
                for a in attachment_usage.iter_range(a_start..a_start + num_attachments) {
                    // There is a subpass S1 that uses or preserves the attachment (`a`)
                    if !attachment_usage.contains(dst * num_attachments + a) {
                        // The attachment is not used or preserved in subpass S.
                        a_preserve_tmp[dst].push(a as u32);
                        changed = true;
                    }
                }
                for &a in &a_preserve_tmp[dst][a_preserve_tmp_offset..] {
                    // mark as used (perserved)
                    attachment_usage.insert(dst * num_attachments + a as usize);
                }
            }
            if !changed {
                break;
            }
        }
        let a_preserves = self.3.clear_and_use_as::<u32>();
        for a in a_preserve_tmp.iter().flatten().copied() {
            a_preserves.push(a);
        }

        let a_refs = a_refs.as_slice();
        let a_preserves = a_preserves.as_slice();
        let subpasses = self.4.clear_and_use_as::<vk::SubpassDescription>();
        let mut a_refs_offset = 0;
        let mut a_preserves_offset = 0;
        for (i, s) in desc.subpasses().iter().enumerate() {
            let end_input_offset = a_refs_offset + s.input_attachments().len();
            let end_color_offset = end_input_offset + s.color_attachments().len();
            let end_preserves_offset = a_preserves_offset + a_preserve_tmp[i].len();
            let mut b = vk::SubpassDescription::builder()
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .preserve_attachments(&a_preserves[a_preserves_offset..end_preserves_offset])
                .input_attachments(&a_refs[a_refs_offset..end_input_offset])
                .color_attachments(&a_refs[end_input_offset..end_color_offset]);
            // TODO: resolve
            a_refs_offset = end_color_offset;
            a_preserves_offset = end_preserves_offset;
            if s.depth_stencil_attachment().is_some() {
                a_refs_offset += 1;
                b = b.depth_stencil_attachment(&a_refs[end_color_offset]);
            }
            subpasses.push(b.build());
        }

        let buf = self.5.clear_and_use_as::<vk::RenderPassCreateInfo>();
        buf.reserve(1);
        buf.push(
            vk::RenderPassCreateInfo::builder()
                .attachments(attachments.as_slice())
                .subpasses(subpasses.as_slice())
                .dependencies(sp_deps.as_slice())
                .build(),
        );
        &buf.as_slice()[0]
    }
}

impl CreateInfoConverter2 {
    #[inline]
    pub const fn new() -> Self {
        Self(ScratchBuffer::new(), ScratchBuffer::new())
    }

    pub fn bind_group_layout(
        &mut self,
        desc: &BindGroupLayoutDescriptor<'_>,
    ) -> &vk::DescriptorSetLayoutCreateInfo {
        let buf0 = self.0.clear_and_use_as::<vk::DescriptorSetLayoutBinding>();
        buf0.reserve(desc.entries.len());
        for e in desc.entries.as_ref() {
            buf0.push(
                vk::DescriptorSetLayoutBinding::builder()
                    .binding(e.binding)
                    .descriptor_count(e.count)
                    .build(),
            );
            // TODO: descriptor_type, stage_flags, immutable_samplers
            todo!();
        }

        let buf = self
            .1
            .clear_and_use_as::<vk::DescriptorSetLayoutCreateInfo>();
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
        let buf0 = self.0.clear_and_use_as::<vk::DescriptorSetLayout>();
        buf0.reserve(desc.bind_group_layouts.len());
        for bgl in desc.bind_group_layouts.as_ref() {
            buf0.push(res.bind_group_layouts[*bgl]);
        }

        let buf = self.1.clear_and_use_as::<vk::PipelineLayoutCreateInfo>();
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
        let buf = self.0.clear_and_use_as::<vk::GraphicsPipelineCreateInfo>();
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
        let buf = self.0.clear_and_use_as::<vk::ComputePipelineCreateInfo>();
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
        let buf = self
            .0
            .clear_and_use_as::<vk::RayTracingPipelineCreateInfoKHR>();
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
