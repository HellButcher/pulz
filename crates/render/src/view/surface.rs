use crate::texture::TextureId;
use window::WindowsMirror;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SurfaceTarget {
    pub texture: TextureId,
    pub sampled: Option<TextureId>,
}

pub type SurfaceTargets = WindowsMirror<SurfaceTarget>;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Msaa {
    pub samples: u32,
}

impl Default for Msaa {
    fn default() -> Self {
        Self { samples: 4 }
    }
}
