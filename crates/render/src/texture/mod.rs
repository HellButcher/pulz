mod descriptor;
mod image;

pub use self::{descriptor::*, image::*};

crate::backend::define_gpu_resource!(Texture, TextureDescriptor);
