use bitflags::bitflags;

crate::backend::define_gpu_resource!(Buffer, BufferDescriptor);

#[derive(Debug, Clone, Eq, PartialEq)]
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
    pub struct BufferUsage: u32 {
        const MAP_READ = 1;
        const MAP_WRITE = 2;
        const TRANSFER_SRC = 4;
        const TRANSFER_DST = 8;
        const INDEX = 16;
        const VERTEX = 32;
        const UNIFORM = 64;
        const STORAGE = 128;
        const INDIRECT = 256;

        const NONE = 0;
    }
}
