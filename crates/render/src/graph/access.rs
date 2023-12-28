use std::{
    fmt::Debug,
    hash::Hash,
    ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Sub},
};

use bitflags::bitflags;
use pulz_assets::Handle;

use crate::{
    buffer::{Buffer, BufferUsage},
    camera::RenderTarget,
    texture::{Texture, TextureDimensions, TextureFormat, TextureUsage},
};
pub trait ResourceAccess: Copy + Eq + Default + Hash {
    // Bitflags!
    type Usage: Copy
        + Clone
        + Debug
        + Default
        + Eq
        + BitOr
        + BitOrAssign
        + BitAnd
        + BitAndAssign
        + BitXor
        + BitXorAssign
        + Not
        + Sub
        + Hash;

    type Format: PartialEq + Debug + Copy + Hash;
    type Size: PartialEq + Copy + Debug;
    type ExternHandle: Debug;

    fn check_usage_is_pass_compatible(combined_usage: Self::Usage);

    fn default_format(usage: Self::Usage) -> Self::Format;
    fn merge_size_max(a: Self::Size, b: Self::Size) -> Option<Self::Size>;
}

impl ResourceAccess for Texture {
    type Usage = TextureUsage;
    type Format = TextureFormat;
    type Size = TextureDimensions;
    type ExternHandle = RenderTarget;

    fn check_usage_is_pass_compatible(combined_usage: Self::Usage) {
        if combined_usage.is_non_attachment() {
            panic!("Can't use texture as non-attachment resource multiple times in the same pass");
        }
    }

    #[inline]
    fn default_format(usage: Self::Usage) -> Self::Format {
        if usage.contains(TextureUsage::DEPTH_STENCIL_ATTACHMENT) {
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
    type Usage = BufferUsage;
    type Format = ();
    type Size = usize;
    type ExternHandle = Handle<Buffer>;

    fn check_usage_is_pass_compatible(_combined_usage: Self::Usage) {
        panic!("Can't use buffer multiple times in the same pass");
    }

    #[inline]
    fn default_format(_usage: Self::Usage) -> Self::Format {}

    #[inline]
    fn merge_size_max(a: usize, b: usize) -> Option<usize> {
        Some(a.max(b))
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

impl ShaderStage {
    #[inline]
    pub const fn as_stage(self) -> Stage {
        Stage::from_bits_truncate(self.bits())
    }
}

impl From<ShaderStage> for Stage {
    #[inline]
    fn from(shader_state: ShaderStage) -> Self {
        shader_state.as_stage()
    }
}
