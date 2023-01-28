use crate::{
    pipeline::{PipelineLayout, SpecializationInfo},
    shader::ShaderModule,
};

crate::backend::define_gpu_resource!(ComputePipeline, ComputePipelineDescriptor<'l>);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComputePipelineDescriptor<'a> {
    pub label: Option<&'a str>,
    pub layout: Option<PipelineLayout>,
    pub module: ShaderModule,
    pub entry_point: &'a str,
    pub specialization: SpecializationInfo<'a>,
}
