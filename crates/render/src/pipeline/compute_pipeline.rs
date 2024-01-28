use serde::{Deserialize, Serialize};

use crate::{
    pipeline::{PipelineLayout, SpecializationInfo},
    shader::ShaderModule,
};

crate::backend::define_gpu_resource!(ComputePipeline, ComputePipelineDescriptor<'l>);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ComputePipelineDescriptor<'a> {
    pub label: Option<&'a str>,
    #[serde(with = "crate::utils::serde_slots::option")]
    pub layout: Option<PipelineLayout>,
    #[serde(with = "crate::utils::serde_slots")]
    pub module: ShaderModule,
    pub entry_point: &'a str,
    pub specialization: SpecializationInfo<'a>,
}
