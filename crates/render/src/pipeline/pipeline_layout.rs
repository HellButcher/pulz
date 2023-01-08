use super::BindGroupLayout;

pub struct PipelineLayoutDescriptor<'a> {
    pub label: Option<&'a str>,
    pub bind_group_layouts: &'a [BindGroupLayout],
}
