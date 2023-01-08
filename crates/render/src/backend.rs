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

pub trait RenderBackend {
    type Error: std::error::Error;

    type Buffer;
    type Texture;
    type ShaderModule;
}
