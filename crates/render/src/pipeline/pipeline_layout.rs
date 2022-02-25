use std::num::NonZeroU32;

use super::BindGroupLayoutId;

pub struct PipelineLayoutDescriptor<'a> {
    pub label: Option<&'a str>,
    pub bind_group_layouts: &'a [BindGroupLayoutId],
}

pub struct BindGroupLayoutDescriptor<'a> {
    pub label: Option<&'a str>,
    pub entries: &'a [BindGroupLayoutEntry],
}

pub struct BindGroupLayoutEntry {
    pub binding: u32,
    // pub visibility: ShaderStages,
    // pub ty: BindingType,
    // TODO:
    pub count: Option<NonZeroU32>,
}
