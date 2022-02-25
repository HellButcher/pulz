use crate::backend::RenderBackendTypes;
use slotmap::{new_key_type, SlotMap};

macro_rules! define_arena_wrappers {
  ($root_vis:vis struct $root_type_name:ident {
    $(
      $v:vis $field_name:ident : $collection_type_name:ident ($id_type_name:ident) -> $backend_type_name:ident;
    )*
  }) => {
    $(
      new_key_type!{
        $v struct $id_type_name;
      }

      $v type $collection_type_name<B: RenderBackendTypes + ?Sized> = SlotMap<$id_type_name,B::$backend_type_name>;

    )*

    $root_vis struct $root_type_name<B: RenderBackendTypes + ?Sized> {
      $(
        $v $field_name: $collection_type_name<B>,
      )*
    }

    impl<B: RenderBackendTypes> $root_type_name<B>{
      #[inline]
      pub fn new() -> Self {
        Self{
          $(
            $field_name: $collection_type_name::<B>::with_key(),
          )*
        }
      }
    }

    impl<B: RenderBackendTypes> Default for $root_type_name<B>{
      fn default() -> Self {
          Self::new()
      }
    }
  };
}

define_arena_wrappers! {
  pub struct RenderBackendResources{
    pub buffers: Buffers(BufferId) -> Buffer;
    pub textures: Textures(TextureId) -> Texture;
    pub shader_modules: ShaderModules(ShaderModuleId) -> ShaderModule;
    pub bind_group_layouts: BindGroupLayouts(BindGroupLayoutId) -> BindGroupLayout;
    pub pipeline_layouts: PipelineLayouts(PipelineLayoutId) -> PipelineLayout;
    pub graphics_pipelines: GraphicsPipelines(GraphicsPipelineId) -> GraphicsPipeline;
    pub compute_pipelines: ComputePipelines(ComputePipelineId) -> ComputePipeline;
  }
}
