mod binding;
mod descriptor;
mod pipeline_layout;

pub use self::{binding::*, descriptor::*, pipeline_layout::*};

crate::backend::define_gpu_resource!(BindGroupLayout, BindGroupLayoutDescriptor<'l>);
crate::backend::define_gpu_resource!(PipelineLayout, PipelineLayoutDescriptor<'l>);
crate::backend::define_gpu_resource!(ComputePipeline, ComputePipelineDescriptor<'l>);
crate::backend::define_gpu_resource!(GraphicsPipeline, GraphicsPipelineDescriptor<'l>);
crate::backend::define_gpu_resource!(RayTracingPipeline, RayTracingPipelineDescriptor<'l>);
