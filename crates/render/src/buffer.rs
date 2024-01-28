use bitflags::bitflags;
use serde::{Deserialize, Serialize};

use crate::graph::access::Access;

crate::backend::define_gpu_resource!(Buffer, BufferDescriptor);

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct BufferDescriptor {
    pub size: usize,
    pub usage: BufferUsage,
}

impl BufferDescriptor {
    pub const fn new() -> Self {
        Self {
            size: 0,
            usage: BufferUsage::empty(),
        }
    }
}

impl Default for BufferDescriptor {
    fn default() -> Self {
        Self::new()
    }
}
bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
    pub struct BufferUsage: u32 {
        const TRANSFER_READ = 0x0001;
        const TRANSFER_WRITE = 0x0002;
        const HOST_READ = 0x0004;
        const HOST_WRITE = 0x0008;
        const INDEX = 0x0010;
        const VERTEX = 0x0020;
        const INDIRECT = 0x0040;
        const UNIFORM = 0x0080;
        const STORAGE = 0x0100;
        const UNIFORM_TEXEL = 0x0200;
        const STORAGE_TEXEL = 0x0400;
        const ACCELERATION_STRUCTURE_BUILD_INPUT = 0x0800;
        const ACCELERATION_STRUCTURE_STORAGE = 0x1000;
        const SHADER_BINDING_TABLE = 0x2000;
        const NONE = 0;
    }
}

impl Access {
    pub fn as_buffer_usage(self) -> BufferUsage {
        let mut result = BufferUsage::NONE;
        if self.intersects(Self::INDIRECT_COMMAND_READ) {
            result |= BufferUsage::INDIRECT;
        }
        if self.intersects(Self::INDEX_READ) {
            result |= BufferUsage::INDEX;
        }
        if self.intersects(Self::VERTEX_ATTRIBUTE_READ) {
            result |= BufferUsage::VERTEX;
        }

        if self.intersects(Self::SHADER_READ | Self::SHADER_WRITE) {
            result |= BufferUsage::STORAGE;
        }
        if self.intersects(Self::UNIFORM_READ) {
            result |= BufferUsage::UNIFORM;
        }

        if self.intersects(Self::TRANSFER_READ) {
            result |= BufferUsage::TRANSFER_READ;
        }
        if self.intersects(Self::TRANSFER_WRITE) {
            result |= BufferUsage::TRANSFER_WRITE;
        }
        if self.intersects(Self::HOST_READ) {
            result |= BufferUsage::HOST_READ;
        }
        if self.intersects(Self::HOST_WRITE) {
            result |= BufferUsage::HOST_WRITE;
        }
        // TODO: check this
        if self.intersects(
            Self::ACCELERATION_STRUCTURE_BUILD_READ | Self::ACCELERATION_STRUCTURE_BUILD_WRITE,
        ) {
            result |= BufferUsage::ACCELERATION_STRUCTURE_STORAGE;
        }
        result
    }
}

impl From<Access> for BufferUsage {
    #[inline]
    fn from(access: Access) -> Self {
        access.as_buffer_usage()
    }
}
