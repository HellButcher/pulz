use std::{
    fmt::Debug,
    hash::Hash,
    ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Sub},
};

use bitflags::bitflags;

use crate::{
    buffer::{Buffer, BufferUsage},
    texture::{Texture, TextureDimensions, TextureFormat, TextureUsage},
};

pub trait ResourceAccess: Copy + Eq + Hash {
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

    type Format: PartialEq + Debug + Copy;
    type Size: PartialEq + Copy;

    fn check_usage_is_pass_compatible(combined_usage: Self::Usage);

    fn default_format(usage: Self::Usage) -> Self::Format;
}

impl ResourceAccess for Texture {
    type Usage = TextureUsage;
    type Format = TextureFormat;
    type Size = TextureDimensions;

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
}

impl ResourceAccess for Buffer {
    type Usage = BufferUsage;
    type Format = ();
    type Size = usize;

    fn check_usage_is_pass_compatible(_combined_usage: Self::Usage) {
        panic!("Can't use buffer multiple times in the same pass");
    }

    #[inline]
    fn default_format(_usage: Self::Usage) -> Self::Format {}
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
