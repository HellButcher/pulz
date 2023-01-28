use super::BindGroupLayout;

crate::backend::define_gpu_resource!(PipelineLayout, PipelineLayoutDescriptor<'l>);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PipelineLayoutDescriptor<'a> {
    pub label: Option<&'a str>,
    pub bind_group_layouts: std::borrow::Cow<'a, [BindGroupLayout]>,
}
