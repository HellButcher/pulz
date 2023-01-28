use bitflags::bitflags;
use pulz_transform::math::{usize2, usize3};

use crate::math::{USize2, USize3};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct TextureDescriptor {
    pub dimensions: TextureDimensions,
    pub mip_level_count: u32,
    pub sample_count: u8,
    pub format: TextureFormat,
    pub aspects: TextureAspects,
    pub usage: TextureUsage,
}

impl TextureDescriptor {
    pub const fn new() -> Self {
        Self {
            dimensions: TextureDimensions::D2(USize2::ONE),
            mip_level_count: 1,
            sample_count: 1,
            format: TextureFormat::DEFAULT,
            aspects: TextureAspects::DEFAULT,
            usage: TextureUsage::ALL_READ,
        }
    }

    pub fn aspects(&self) -> TextureAspects {
        if self.aspects.is_empty() {
            self.format.aspects()
        } else {
            self.aspects
        }
    }

    #[inline]
    pub fn data_layout(&self) -> Option<ImageDataLayout> {
        ImageDataLayout::from_format(self.dimensions.subimage_extents(), self.format)
    }
}

impl Default for TextureDescriptor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum TextureDimensions {
    D1(u32),
    D2(USize2),
    D2Array { size: USize2, array_len: u32 },
    Cube(USize2),
    CubeArray { size: USize2, array_len: u32 },
    D3(USize3),
}

impl TextureDimensions {
    #[inline]
    pub fn extents(&self) -> USize3 {
        match *self {
            Self::D1(len) => usize3(len, 1, 1),
            Self::D2(size) => usize3(size.x, size.y, 1),
            Self::D2Array { size, array_len } => usize3(size.x, size.y, array_len),
            Self::Cube(size) => usize3(size.x, size.y, 6),
            Self::CubeArray { size, array_len } => usize3(size.x, size.y, array_len * 6),
            Self::D3(size) => size,
        }
    }

    #[inline]
    pub fn subimage_extents(&self) -> USize2 {
        match *self {
            Self::D1(len) => usize2(len, 1),
            Self::D2(size) => size,
            Self::D2Array { size, .. } => size,
            Self::Cube(size) => size,
            Self::CubeArray { size, .. } => size,
            Self::D3(size) => usize2(size.x, size.y),
        }
    }
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[non_exhaustive]
pub enum TextureFormat {
    // 8-bit formats
    R8Unorm = 0,
    R8Snorm = 1,
    R8Uint = 2,
    R8Sint = 3,

    // 16-bit formats
    R16Uint = 4,
    R16Sint = 5,
    R16Float = 6,
    Rg8Unorm = 7,
    Rg8Snorm = 8,
    Rg8Uint = 9,
    Rg8Sint = 10,

    // 32-bit formats
    R32Uint = 11,
    R32Sint = 12,
    R32Float = 13,
    Rg16Uint = 14,
    Rg16Sint = 15,
    Rg16Float = 16,
    Rgba8Unorm = 17,
    Rgba8UnormSrgb = 18,
    Rgba8Snorm = 19,
    Rgba8Uint = 20,
    Rgba8Sint = 21,
    Bgra8Unorm = 22,
    Bgra8UnormSrgb = 23,

    // Packed 32-bit formats
    Rgb9E5Ufloat = 24,
    Rgb10A2Unorm = 25,
    Rg11B10Float = 26,

    // 64-bit formats
    Rg32Uint = 27,
    Rg32Sint = 28,
    Rg32Float = 29,
    Rgba16Uint = 30,
    Rgba16Sint = 31,
    Rgba16Float = 32,

    // 128-bit formats
    Rgba32Uint = 33,
    Rgba32Sint = 34,
    Rgba32Float = 35,

    // Depth and stencil formats
    // https://gpuweb.github.io/gpuweb/#depth-formats
    Depth16Unorm = 36, // depth, 2 bytes per pixel
    Depth24Plus = 37,  // depth, (3-)4 bytes per pixel!!!
    Depth32Float = 38, // depth  4 bytes per pixel

    // // these can have a variable size!
    Stencil8 = 39,            // stencil, 1-4 bytes per pixel
    Depth24PlusStencil8 = 40, // depth+stencil, 4-8 bytes per pixel
}

impl TextureFormat {
    pub const DEFAULT: Self = Self::Rgba8UnormSrgb;

    pub fn num_components(self) -> u8 {
        use self::TextureFormat::*;
        match self {
            // 8-bit formats
            R8Unorm | R8Snorm | R8Uint | R8Sint => 1,

            // 16-bit formats
            R16Uint | R16Sint | R16Float => 1,
            Rg8Unorm | Rg8Snorm | Rg8Uint | Rg8Sint => 2,

            // 32-bit formats
            R32Uint | R32Sint | R32Float => 1,
            Rg16Uint | Rg16Sint | Rg16Float => 2,
            Rgba8Unorm | Rgba8UnormSrgb | Rgba8Snorm | Rgba8Uint | Rgba8Sint | Bgra8Unorm
            | Bgra8UnormSrgb => 4,

            // Packed 32-bit formats
            Rgb9E5Ufloat => 3,
            Rgb10A2Unorm => 4,
            Rg11B10Float => 3,

            // 64-bit formats
            Rg32Uint | Rg32Sint | Rg32Float => 2,
            Rgba16Uint | Rgba16Sint | Rgba16Float => 4,

            // 128-bit formats
            Rgba32Uint | Rgba32Sint | Rgba32Float => 4,

            // Depth and stencil formats
            Depth32Float => 1,
            Depth16Unorm => 1,

            // // these can have a variable size!
            Stencil8 => 1,
            Depth24Plus => 1,
            Depth24PlusStencil8 => 2,
        }
    }

    pub fn bytes_per_pixel(self) -> Option<u8> {
        use self::TextureFormat::*;
        Some(match self {
            // 8-bit formats
            R8Unorm | R8Snorm | R8Uint | R8Sint => 1,

            // 16-bit formats
            R16Uint | R16Sint | R16Float | Rg8Unorm | Rg8Snorm | Rg8Uint | Rg8Sint => 2,

            // 32-bit formats
            R32Uint | R32Sint | R32Float | Rg16Uint | Rg16Sint | Rg16Float | Rgba8Unorm
            | Rgba8UnormSrgb | Rgba8Snorm | Rgba8Uint | Rgba8Sint | Bgra8Unorm | Bgra8UnormSrgb => {
                4
            }

            // Packed 32-bit formats
            Rgb9E5Ufloat | Rgb10A2Unorm | Rg11B10Float => 4,

            // 64-bit formats
            Rg32Uint | Rg32Sint | Rg32Float | Rgba16Uint | Rgba16Sint | Rgba16Float => 8,

            // 128-bit formats
            Rgba32Uint | Rgba32Sint | Rgba32Float => 16,

            // Depth and stencil formats
            Depth16Unorm => 2,
            Depth32Float => 4,

            // these can have a variable size!
            // https://gpuweb.github.io/gpuweb/#depth-formats
            // TODO: provide a way to query these
            Stencil8 | Depth24Plus | Depth24PlusStencil8 => {
                return None;
            }
        })
    }

    pub fn aspects(self) -> TextureAspects {
        match self {
            Self::Stencil8 => TextureAspects::STENCIL,
            Self::Depth16Unorm | Self::Depth24Plus | Self::Depth32Float => TextureAspects::DEPTH,
            Self::Depth24PlusStencil8 => TextureAspects::DEPTH | TextureAspects::STENCIL,

            _ => TextureAspects::COLOR,
        }
    }
}

impl Default for TextureFormat {
    #[inline]
    fn default() -> Self {
        Self::DEFAULT
    }
}

bitflags! {
    pub struct TextureAspects: u32 {
        const COLOR = 1;
        const DEPTH = 2;
        const STENCIL = 4;

        const DEFAULT = 0;
    }
}

impl Default for TextureAspects {
    #[inline]
    fn default() -> Self {
        Self::DEFAULT
    }
}

bitflags! {
  pub struct TextureUsage: u32 {
      const TRANSFER_SRC = 1;
      const TRANSFER_DST = 2;
      const SAMPLED = 4;
      const STORAGE = 8;
      const COLOR_ATTACHMENT = 16;
      const DEPTH_STENCIL_ATTACHMENT = 32;
      const INPUT_ATTACHMENT = 64;

      // modifiers
      const BY_REGION = 128;

      const NONE = 0;
      const ALL_READ = Self::TRANSFER_SRC.bits | Self::SAMPLED.bits | Self::INPUT_ATTACHMENT.bits;
      const ALL_WRITE = Self::TRANSFER_DST.bits | Self::STORAGE.bits | Self::COLOR_ATTACHMENT.bits | Self::DEPTH_STENCIL_ATTACHMENT.bits;
      const ALL_ATTACHMENTS = Self::COLOR_ATTACHMENT.bits | Self::DEPTH_STENCIL_ATTACHMENT.bits | Self::INPUT_ATTACHMENT.bits;
  }
}

impl TextureUsage {
    #[inline]
    pub const fn is_attachment(self) -> bool {
        self.intersects(Self::ALL_ATTACHMENTS)
    }

    #[inline]
    pub const fn is_non_attachment(self) -> bool {
        self.intersects(Self::ALL_ATTACHMENTS.complement())
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default)]
pub struct ImageDataLayout {
    pub offset: usize,
    pub bytes_per_row: u32,
    pub rows_per_image: u32,
}

impl ImageDataLayout {
    pub fn from_format(extents: USize2, format: TextureFormat) -> Option<Self> {
        let bytes_per_pixel = format.bytes_per_pixel()?;
        Some(Self {
            offset: 0,
            bytes_per_row: extents.x * bytes_per_pixel as u32,
            rows_per_image: extents.y,
        })
    }
}
