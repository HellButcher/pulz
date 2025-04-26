use serde::{Deserialize, Serialize};

use super::BindGroupLayout;

crate::backend::define_gpu_resource!(PipelineLayout, PipelineLayoutDescriptor<'l>);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PipelineLayoutDescriptor<'a> {
    pub label: Option<&'a str>,
    #[serde(with = "crate::utils::serde_slots::cow_vec")]
    pub bind_group_layouts: std::borrow::Cow<'a, [BindGroupLayout]>,
}
