use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::{
    pipeline::{PipelineLayout, SpecializationInfo},
    shader::ShaderModule,
};

crate::backend::define_gpu_resource!(RayTracingPipeline, RayTracingPipelineDescriptor<'l>);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RayTracingPipelineDescriptor<'a> {
    pub label: Option<&'a str>,
    #[serde(with = "crate::utils::serde_slots::option")]
    pub layout: Option<PipelineLayout>,
    pub modules: Cow<'a, [RayTracingShaderModule<'a>]>,
    pub groups: Cow<'a, [RayTracingShaderGroup]>,
    pub max_recursion_depth: u32,
    pub specialization: SpecializationInfo<'a>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RayTracingShaderGroup {
    pub group_type: RayTracingGroupType,
    pub general_shader: u32,
    pub closest_hit_shader: u32,
    pub any_hit_shader: u32,
    pub intersection_shader: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RayTracingShaderModule<'a> {
    pub stage: RayTracingStage,
    #[serde(with = "crate::utils::serde_slots")]
    pub module: ShaderModule,
    pub entry_point: &'a str,
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum RayTracingStage {
    #[default]
    Raygen,
    AnyHit,
    ClosestHit,
    Miss,
    Intersection,
    Callable,
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum RayTracingGroupType {
    #[default]
    General,
    TrianglesHitGroup,
    ProceduralHitGroup,
}
