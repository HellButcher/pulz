use bitflags::bitflags;
use serde::{Deserialize, Serialize};

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
    #[derive(Default, Serialize, Deserialize)]
    pub struct BufferUsage: u32 {
        const TRANSFER_SRC = 1;
        const TRANSFER_DST = 2;
        const HOST = 4; // used in combination with TRANSFER_SRC / TRANSFER_DST
        const INDEX = 8;
        const VERTEX = 17;
        const UNIFORM = 32;
        const STORAGE = 64;
        const INDIRECT = 128;

        const NONE = 0;
    }
}
