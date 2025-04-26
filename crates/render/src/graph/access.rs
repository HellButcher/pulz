use std::{fmt::Debug, hash::Hash};

use bitflags::bitflags;
use pulz_assets::Handle;
use serde::{Deserialize, Serialize};

use crate::{
    buffer::Buffer,
    camera::RenderTarget,
    texture::{Texture, TextureDimensions, TextureFormat},
};

pub trait ResourceAccess: Copy + Eq + Default + Hash {
    type Format: PartialEq + Debug + Copy + Hash;
    type Size: PartialEq + Copy + Debug;
    type ExternHandle: Debug;

    fn default_format(access: Access) -> Self::Format;
    fn merge_size_max(a: Self::Size, b: Self::Size) -> Option<Self::Size>;
}

impl ResourceAccess for Texture {
    type Format = TextureFormat;
    type Size = TextureDimensions;
    type ExternHandle = RenderTarget;

    #[inline]
    fn default_format(access: Access) -> Self::Format {
        if access.intersects(
            Access::DEPTH_STENCIL_ATTACHMENT_READ
                | Access::DEPTH_STENCIL_ATTACHMENT_STENCIL_WRITE
                | Access::DEPTH_STENCIL_INPUT_ATTACHMENT_READ,
        ) {
            TextureFormat::Depth24PlusStencil8
        } else {
            TextureFormat::Rgba8UnormSrgb
        }
    }

    #[inline]
    fn merge_size_max(a: Self::Size, b: Self::Size) -> Option<Self::Size> {
        use TextureDimensions::*;
        match (a, b) {
            (D1(a), D1(b)) => Some(D1(a.max(b))),
            (D2(a), D2(b)) => Some(D2(a.max(b))),
            (
                D2Array {
                    size: a1,
                    array_len: a2,
                },
                D2Array {
                    size: b1,
                    array_len: b2,
                },
            ) => Some(D2Array {
                size: a1.max(b1),
                array_len: a2.max(b2),
            }),
            (Cube(a), Cube(b)) => Some(Cube(a.max(b))),
            (
                CubeArray {
                    size: a1,
                    array_len: a2,
                },
                CubeArray {
                    size: b1,
                    array_len: b2,
                },
            ) => Some(CubeArray {
                size: a1.max(b1),
                array_len: a2.max(b2),
            }),
            (D3(a), D3(b)) => Some(D3(a.max(b))),

            _ => None,
        }
    }
}

impl ResourceAccess for Buffer {
    type Format = ();
    type Size = usize;
    type ExternHandle = Handle<Self>;

    #[inline]
    fn default_format(_access: Access) -> Self::Format {}

    #[inline]
    fn merge_size_max(a: usize, b: usize) -> Option<usize> {
        Some(a.max(b))
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
    pub struct Access: u32 {
        const INDIRECT_COMMAND_READ               = 0x00000001;
        const INDEX_READ                          = 0x00000002;
        const VERTEX_ATTRIBUTE_READ               = 0x00000004;
        const COLOR_INPUT_ATTACHMENT_READ         = 0x00000008;
        const DEPTH_STENCIL_INPUT_ATTACHMENT_READ = 0x00000010;

        // combined with shader stage: 0x??000000
        const UNIFORM_READ = 0x00000020;
        const SHADER_READ  = 0x00000040;
        const SAMPLED_READ  = 0x00000080;

        const COLOR_ATTACHMENT_READ  = 0x00000100;
        const DEPTH_STENCIL_ATTACHMENT_READ          = 0x00000200;
        const TRANSFER_READ  = 0x00000400;
        const HOST_READ      = 0x00000800;
        const ACCELERATION_STRUCTURE_READ        = 0x00001000;
        const ACCELERATION_STRUCTURE_BUILD_READ  = 0x00002000;
        const PRESENT  = 0x00004000;

        // combined with shader stage: 0x??000000
        const SHADER_WRITE = 0x00010000;

        const COLOR_ATTACHMENT_WRITE = 0x00020000;
        const DEPTH_STENCIL_ATTACHMENT_DEPTH_WRITE   = 0x00040000;
        const DEPTH_STENCIL_ATTACHMENT_STENCIL_WRITE = 0x00080000;
        const DEPTH_STENCIL_ATTACHMENT_WRITE         = Self::DEPTH_STENCIL_ATTACHMENT_DEPTH_WRITE.bits() | Self::DEPTH_STENCIL_ATTACHMENT_STENCIL_WRITE.bits();
        const TRANSFER_WRITE = 0x00100000;
        const HOST_WRITE     = 0x00200000;
        const ACCELERATION_STRUCTURE_BUILD_WRITE = 0x00400000;

        const VERTEX_SHADER                  = 0x01000000;
        const TESSELLATION_CONTROL_SHADER    = 0x02000000;
        const TESSELLATION_EVALUATION_SHADER = 0x04000000;
        const GEOMETRY_SHADER                = 0x08000000;
        const FRAGMENT_SHADER                = 0x10000000;
        const COMPUTE_SHADER                 = 0x20000000;
        const RAY_TRACING_SHADER             = 0x40000000;

        const NONE = 0;
        const ANY_READ = 0x0000FFFF;
        const ANY_WRITE = 0x00FF0000;
        const ANY_SHADER_STAGE = 0xFF000000;
        const GRAPICS_ATTACHMENTS = Self::COLOR_INPUT_ATTACHMENT_READ.bits() | Self::DEPTH_STENCIL_INPUT_ATTACHMENT_READ.bits() | Self::COLOR_ATTACHMENT_READ.bits() | Self::COLOR_ATTACHMENT_WRITE.bits() | Self::DEPTH_STENCIL_ATTACHMENT_READ.bits() | Self::DEPTH_STENCIL_ATTACHMENT_WRITE.bits();
        const MEMORY_READ = Self::ANY_SHADER_STAGE.bits() | Self::ANY_READ.bits();
        const MEMORY_WRITE = Self::ANY_SHADER_STAGE.bits() | Self::ANY_WRITE.bits();
        const GENERAL = 0xFFFFFFFF;
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
    pub struct Stage: u32 {
        // const TOP_OF_PIPE = 0x00000001;
        const DRAW_INDIRECT = 0x00000002;
        const VERTEX_INPUT = 0x00000004;
        const VERTEX_SHADER = 0x00000008;
        const TESSELLATION_CONTROL_SHADER = 0x00000010;
        const TESSELLATION_EVALUATION_SHADER = 0x00000020;
        const GEOMETRY_SHADER = 0x00000040;
        const FRAGMENT_SHADER = 0x00000080;
        const EARLY_FRAGMENT_TESTS = 0x00000100;
        const LATE_FRAGMENT_TESTS = 0x00000200;
        const FRAGMENT_TESTS = 0x00000300; // EARLY_FRAGMENT_TESTS | LATE_FRAGMENT_TESTS

        const COLOR_ATTACHMENT_OUTPUT = 0x00000400;
        const COMPUTE_SHADER = 0x00000800;
        const TRANSFER = 0x00001000;
        // const BOTTOM_OF_PIPE = 0x00002000;
        const HOST = 0x00004000;
        // const ALL_GRAPHICS = 0x00008000;

        const ACCELERATION_STRUCTURE_BUILD = 0x02000000;
        const RAY_TRACING_SHADER = 0x00200000;

        const NONE = 0;
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
    pub struct ShaderStage: u32 {
        // SUBSET of Stage
        const VERTEX = 0x00000008;
        const TESSELLATION_CONTROL = 0x00000010;
        const TESSELLATION_EVALUATION = 0x00000020;
        const GEOMETRY = 0x00000040;
        const FRAGMENT = 0x00000080;
        const COMPUTE = 0x00000800;
        const RAY_TRACING = 0x00200000;

        const NONE = 0;
    }
}

impl Access {
    #[inline]
    pub fn as_stage(self) -> Stage {
        let mut result = Stage::NONE;
        if self.intersects(Self::INDIRECT_COMMAND_READ) {
            result |= Stage::DRAW_INDIRECT;
        }
        if self.intersects(Self::INDEX_READ | Self::VERTEX_ATTRIBUTE_READ) {
            result |= Stage::VERTEX_INPUT;
        }
        if self.intersects(
            Self::COLOR_INPUT_ATTACHMENT_READ | Self::DEPTH_STENCIL_INPUT_ATTACHMENT_READ,
        ) {
            result |= Stage::FRAGMENT_SHADER;
        }

        if self.intersects(Self::VERTEX_SHADER) {
            result |= Stage::VERTEX_SHADER;
        }
        if self.intersects(Self::TESSELLATION_CONTROL_SHADER) {
            result |= Stage::TESSELLATION_CONTROL_SHADER;
        }
        if self.intersects(Self::TESSELLATION_EVALUATION_SHADER) {
            result |= Stage::TESSELLATION_EVALUATION_SHADER;
        }
        if self.intersects(Self::GEOMETRY_SHADER) {
            result |= Stage::GEOMETRY_SHADER;
        }
        if self.intersects(Self::FRAGMENT_SHADER) {
            result |= Stage::FRAGMENT_SHADER;
        }
        if self.intersects(Self::COMPUTE_SHADER) {
            result |= Stage::COMPUTE_SHADER;
        }
        if self.intersects(Self::RAY_TRACING_SHADER) {
            result |= Stage::RAY_TRACING_SHADER;
        }

        if self.intersects(Self::COLOR_ATTACHMENT_READ | Self::COLOR_ATTACHMENT_WRITE) {
            result |= Stage::COLOR_ATTACHMENT_OUTPUT;
        }
        if self
            .intersects(Self::DEPTH_STENCIL_ATTACHMENT_READ | Self::DEPTH_STENCIL_ATTACHMENT_WRITE)
        {
            result |= Stage::FRAGMENT_TESTS;
        }
        if self.intersects(Self::TRANSFER_READ | Self::TRANSFER_WRITE) {
            result |= Stage::TRANSFER;
        }
        if self.intersects(Self::HOST_READ | Self::HOST_WRITE) {
            result |= Stage::HOST;
        }
        if self.intersects(
            Self::ACCELERATION_STRUCTURE_BUILD_READ | Self::ACCELERATION_STRUCTURE_BUILD_WRITE,
        ) {
            result |= Stage::ACCELERATION_STRUCTURE_BUILD;
        }

        result
    }

    #[inline]
    pub const fn is_read(self) -> bool {
        self.intersects(Self::ANY_READ)
    }
    #[inline]
    pub const fn is_write(self) -> bool {
        self.intersects(Self::ANY_WRITE)
    }

    #[inline]
    pub const fn is_graphics_attachment(self) -> bool {
        self.intersects(Self::GRAPICS_ATTACHMENTS)
    }
}

impl ShaderStage {
    #[inline]
    pub const fn as_stage(self) -> Stage {
        Stage::from_bits_truncate(self.bits())
    }

    #[inline]
    pub fn as_access(self) -> Access {
        let mut result = Access::NONE;
        if self.contains(Self::VERTEX) {
            result |= Access::VERTEX_SHADER;
        }
        if self.contains(Self::TESSELLATION_CONTROL) {
            result |= Access::TESSELLATION_CONTROL_SHADER;
        }
        if self.contains(Self::TESSELLATION_EVALUATION) {
            result |= Access::TESSELLATION_EVALUATION_SHADER;
        }
        if self.contains(Self::GEOMETRY) {
            result |= Access::GEOMETRY_SHADER;
        }
        if self.contains(Self::FRAGMENT) {
            result |= Access::FRAGMENT_SHADER;
        }
        if self.contains(Self::COMPUTE) {
            result |= Access::COMPUTE_SHADER;
        }
        if self.contains(Self::RAY_TRACING) {
            result |= Access::RAY_TRACING_SHADER;
        }
        result
    }
}

impl From<ShaderStage> for Stage {
    #[inline]
    fn from(shader_state: ShaderStage) -> Self {
        shader_state.as_stage()
    }
}

impl From<ShaderStage> for Access {
    #[inline]
    fn from(shader_state: ShaderStage) -> Self {
        shader_state.as_access()
    }
}
