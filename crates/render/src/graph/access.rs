use std::{
    fmt::Debug,
    hash::Hash,
    ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, RangeBounds, Sub},
};

use bitflags::bitflags;

use super::resources::{Texture, TextureUsage};
use crate::buffer::{Buffer, BufferUsage};

pub trait ResourceAccess {
    // Bitflags!
    type Usage: Copy
        + Clone
        + Debug
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
}

impl ResourceAccess for Texture {
    type Usage = TextureUsage;
}

impl ResourceAccess for Buffer {
    type Usage = BufferUsage;
}

bitflags! {
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
    pub struct Access: u32 {
        // const INDIRECT_COMMAND_READ = 0x00000001;
        const INDEX_READ = 0x00000002;
        const VERTEX_ATTRIBUTE_READ = 0x00000004;
        const UNIFORM_READ = 0x00000008;
        const INPUT_ATTACHMENT_READ = 0x00000010;
        const SHADER_READ = 0x00000020;
        const SHADER_WRITE = 0x00000040;
        const COLOR_ATTACHMENT_READ = 0x00000080;
        const COLOR_ATTACHMENT_WRITE = 0x00000100;
        const DEPTH_STENCIL_ATTACHMENT_READ = 0x00000200;
        const DEPTH_STENCIL_ATTACHMENT_WRITE = 0x00000400;
        const TRANSFER_READ = 0x00000800;
        const TRANSFER_WRITE = 0x00001000;
        const HOST_READ = 0x00002000;
        const HOST_WRITE = 0x00004000;
        // const MEMORY_READ = 0x00008000;
        // const MEMORY_WRITE = 0x00010000;

        const ACCELERATION_STRUCTURE_READ = 0x00200000;
        const ACCELERATION_STRUCTURE_WRITE = 0x00400000;

        const NONE = 0;
    }
}

bitflags! {
    pub struct BufferReadAccess: u32 {
        // const IndirectCommand = 0x01;
        const INDEX = 0x0002;
        const VERTEX_ATTRIBUTE = 0x0004;
        const TRANSFER = 0x0800;
        const HOST = 0x2000;

        const VERTEX_SHADER_UNIFORM = 0x00010000;
        const VERTEX_SHADER_STORAGE = 0x00020000;
        const TESS_CTRL_SHADER_UNIFORM = 0x00040000;
        const TESS_CTRL_SHADER_STORAGE = 0x00080000;
        const TESS_EVAL_SHADER_UNIFORM = 0x00100000;
        const TESS_EVAL_SHADER_STORAGE = 0x00200000;
        const GEOMETRY_SHADER_UNIFORM = 0x00400000;
        const GEOMETRY_SHADER_STORAGE = 0x00800000;
        const FRAGMENT_SHADER_UNIFORM = 0x01000000;
        const FRAGMENT_SHADER_STORAGE = 0x02000000;
        const COMPUTE_SHADER_UNIFORM = 0x04000000;
        const COMPUTE_SHADER_STORAGE = 0x08000000;
    }
}

bitflags! {
    pub struct BufferWriteAccess: u32 {
        const TRANSFER = 0x1000;
        const HOST = 0x4000;

        const VERTEX_SHADER_STORAGE = 0x00020000;
        const TESS_CTRL_SHADER_STORAGE = 0x00080000;
        const TESS_EVAL_SHADER_STORAGE = 0x00200000;
        const GEOMETRY_SHADER_STORAGE = 0x00800000;
        const FRAGMENT_SHADER_STORAGE = 0x02000000;
        const COMPUTE_SHADER_STORAGE = 0x08000000;
    }
}

bitflags! {
    pub struct TextureReadAccess: u32 {
        const INPUT_ATTACHMENT = 0x0010;
        const COLOR_ATTACHMENT = 0x0080;
        const DEPTH_STENCIL_ATTACHMENT = 0x0200;
        const TRANSFER = 0x0800;
        const HOST = 0x2000;

        const VERTEX_SHADER = 0x00020000;
        const TESS_CTRL_SHADER = 0x00080000;
        const TESS_EVAL_SHADER = 0x00200000;
        const GEOMETRY_SHADER = 0x00800000;
        const FRAGMENT_SHADER = 0x02000000;
        const COMPUTE_SHADER = 0x08000000;
    }
}

bitflags! {
    pub struct TextureWriteAccess: u32 {
        const COLOR_ATTACHMENT = 0x0100;
        const DEPTH_STENCIL_ATTACHMENT = 0x0400;
        const TRANSFER = 0x1000;
        const HOST = 0x4000;

        const VERTEX_SHADER = 0x00020000;
        const TESS_CTRL_SHADER = 0x00080000;
        const TESS_EVAL_SHADER = 0x00200000;
        const GEOMETRY_SHADER = 0x00800000;
        const FRAGMENT_SHADER = 0x02000000;
        const COMPUTE_SHADER = 0x08000000;
    }
}

trait AccessStage: Copy {
    fn stage(self) -> Stage;
}

impl AccessStage for BufferReadAccess {
    #[inline]
    fn stage(self) -> Stage {
        let mut stage = Stage::empty();
        //if self.contains(Self::IndirectCommand) { stage |= Stage::DRAW_INDIRECT; }
        if self.contains(Self::INDEX) {
            stage |= Stage::VERTEX_INPUT;
        }
        if self.contains(Self::VERTEX_ATTRIBUTE) {
            stage |= Stage::VERTEX_INPUT;
        }
        if self.contains(Self::TRANSFER) {
            stage |= Stage::TRANSFER;
        }
        if self.contains(Self::HOST) {
            stage |= Stage::HOST;
        }
        if self.contains(Self::VERTEX_SHADER_UNIFORM) {
            stage |= Stage::VERTEX_SHADER;
        }
        if self.contains(Self::VERTEX_SHADER_STORAGE) {
            stage |= Stage::VERTEX_SHADER;
        }
        if self.contains(Self::TESS_CTRL_SHADER_UNIFORM) {
            stage |= Stage::TESSELLATION_CONTROL_SHADER;
        }
        if self.contains(Self::TESS_CTRL_SHADER_STORAGE) {
            stage |= Stage::TESSELLATION_CONTROL_SHADER;
        }
        if self.contains(Self::TESS_EVAL_SHADER_UNIFORM) {
            stage |= Stage::TESSELLATION_EVALUATION_SHADER;
        }
        if self.contains(Self::TESS_EVAL_SHADER_STORAGE) {
            stage |= Stage::TESSELLATION_EVALUATION_SHADER;
        }
        if self.contains(Self::GEOMETRY_SHADER_UNIFORM) {
            stage |= Stage::GEOMETRY_SHADER;
        }
        if self.contains(Self::GEOMETRY_SHADER_STORAGE) {
            stage |= Stage::GEOMETRY_SHADER;
        }
        if self.contains(Self::FRAGMENT_SHADER_UNIFORM) {
            stage |= Stage::FRAGMENT_SHADER;
        }
        if self.contains(Self::FRAGMENT_SHADER_STORAGE) {
            stage |= Stage::FRAGMENT_SHADER;
        }
        if self.contains(Self::COMPUTE_SHADER_UNIFORM) {
            stage |= Stage::COMPUTE_SHADER;
        }
        if self.contains(Self::COMPUTE_SHADER_STORAGE) {
            stage |= Stage::COMPUTE_SHADER;
        }
        stage
    }
}

impl AccessStage for BufferWriteAccess {
    #[inline]
    fn stage(self) -> Stage {
        let mut stage = Stage::empty();
        if self.contains(Self::TRANSFER) {
            stage |= Stage::TRANSFER;
        }
        if self.contains(Self::HOST) {
            stage |= Stage::HOST;
        }
        if self.contains(Self::VERTEX_SHADER_STORAGE) {
            stage |= Stage::VERTEX_SHADER;
        }
        if self.contains(Self::TESS_CTRL_SHADER_STORAGE) {
            stage |= Stage::TESSELLATION_CONTROL_SHADER;
        }
        if self.contains(Self::TESS_EVAL_SHADER_STORAGE) {
            stage |= Stage::TESSELLATION_EVALUATION_SHADER;
        }
        if self.contains(Self::GEOMETRY_SHADER_STORAGE) {
            stage |= Stage::GEOMETRY_SHADER;
        }
        if self.contains(Self::FRAGMENT_SHADER_STORAGE) {
            stage |= Stage::FRAGMENT_SHADER;
        }
        if self.contains(Self::COMPUTE_SHADER_STORAGE) {
            stage |= Stage::COMPUTE_SHADER;
        }
        stage
    }
}

impl AccessStage for TextureReadAccess {
    #[inline]
    fn stage(self) -> Stage {
        let mut stage = Stage::empty();
        if self.contains(Self::INPUT_ATTACHMENT) {
            stage |= Stage::FRAGMENT_SHADER;
        }
        if self.contains(Self::COLOR_ATTACHMENT) {
            stage |= Stage::COLOR_ATTACHMENT_OUTPUT;
        }
        if self.contains(Self::DEPTH_STENCIL_ATTACHMENT) {
            stage |= Stage::FRAGMENT_TESTS;
        }
        if self.contains(Self::TRANSFER) {
            stage |= Stage::TRANSFER;
        }
        if self.contains(Self::HOST) {
            stage |= Stage::HOST;
        }
        if self.contains(Self::VERTEX_SHADER) {
            stage |= Stage::VERTEX_SHADER;
        }
        if self.contains(Self::TESS_CTRL_SHADER) {
            stage |= Stage::TESSELLATION_CONTROL_SHADER;
        }
        if self.contains(Self::TESS_EVAL_SHADER) {
            stage |= Stage::TESSELLATION_EVALUATION_SHADER;
        }
        if self.contains(Self::GEOMETRY_SHADER) {
            stage |= Stage::GEOMETRY_SHADER;
        }
        if self.contains(Self::FRAGMENT_SHADER) {
            stage |= Stage::FRAGMENT_SHADER;
        }
        if self.contains(Self::COMPUTE_SHADER) {
            stage |= Stage::COMPUTE_SHADER;
        }
        stage
    }
}

impl AccessStage for TextureWriteAccess {
    #[inline]
    fn stage(self) -> Stage {
        let mut stage = Stage::empty();
        if self.contains(Self::COLOR_ATTACHMENT) {
            stage |= Stage::COLOR_ATTACHMENT_OUTPUT;
        }
        if self.contains(Self::DEPTH_STENCIL_ATTACHMENT) {
            stage |= Stage::FRAGMENT_TESTS;
        }
        if self.contains(Self::TRANSFER) {
            stage |= Stage::TRANSFER;
        }
        if self.contains(Self::HOST) {
            stage |= Stage::HOST;
        }
        if self.contains(Self::VERTEX_SHADER) {
            stage |= Stage::VERTEX_SHADER;
        }
        if self.contains(Self::TESS_CTRL_SHADER) {
            stage |= Stage::TESSELLATION_CONTROL_SHADER;
        }
        if self.contains(Self::TESS_EVAL_SHADER) {
            stage |= Stage::TESSELLATION_EVALUATION_SHADER;
        }
        if self.contains(Self::GEOMETRY_SHADER) {
            stage |= Stage::GEOMETRY_SHADER;
        }
        if self.contains(Self::FRAGMENT_SHADER) {
            stage |= Stage::FRAGMENT_SHADER;
        }
        if self.contains(Self::COMPUTE_SHADER) {
            stage |= Stage::COMPUTE_SHADER;
        }
        stage
    }
}
