use pulz_assets::Handle;

use crate::{
    buffer::{Buffer, BufferUsage},
    camera::RenderTarget,
    graph::resources::PhysicalResource,
    texture::{Texture, TextureDimensions, TextureFormat, TextureUsage},
};

pub trait GpuResource: slotmap::Key {
    type Descriptor<'l>;
}

macro_rules! define_gpu_resource {
  ($type_name:ident, $descriptor_type:ident $(<$life:tt>)?) => {
    ::slotmap::new_key_type!{
      pub struct $type_name;
    }

    impl $crate::backend::GpuResource for $type_name {
      type Descriptor<'l> = $descriptor_type $(<$life>)?;
    }
  };
}

// export macro to crate
pub(crate) use define_gpu_resource;

pub trait CommandEncoder {
    fn insert_debug_marker(&mut self, label: &str);
    fn push_debug_group(&mut self, label: &str);
    fn pop_debug_group(&mut self);
}

pub trait PhysicalResourceResolver {
    fn resolve_render_target(
        &mut self,
        render_target: &RenderTarget,
    ) -> Option<PhysicalResource<Texture>>;
    fn resolve_buffer(&mut self, handle: &Handle<Buffer>) -> Option<PhysicalResource<Buffer>>;
    fn create_transient_texture(
        &mut self,
        format: TextureFormat,
        size: TextureDimensions,
        usage: TextureUsage,
    ) -> Option<Texture>;
    fn create_transient_buffer(&mut self, size: usize, usage: BufferUsage) -> Option<Buffer>;
}
