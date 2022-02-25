mod descriptor;
mod image;
pub use self::descriptor::*;
pub use self::image::*;

use crate::cache::Cache;
use crate::cache::Cacheable;
pub use crate::render_resource::TextureId;

impl Cacheable for TextureDescriptor {
    type Target = TextureId;

    fn create(&self, renderer: &mut dyn crate::backend::RenderBackend) -> Self::Target {
        renderer.create_texture(self)
    }

    fn destroy(&self, id: Self::Target, renderer: &mut dyn crate::backend::RenderBackend) {
        renderer.destroy_texture(id)
    }
}

pub type TextureCache = Cache<TextureDescriptor>;
