pub struct BindGroupLayoutDescriptor<'a> {
    pub label: Option<&'a str>,
    pub entries: &'a [BindGroupLayoutEntry],
}

#[derive(Copy, Clone)]
pub struct BindGroupLayoutEntry {
    pub binding: u32,
    // pub visibility: ShaderStages,
    // pub ty: BindingType,
    // TODO:
    pub count: u32,
}

pub use pulz_render_macros::AsBindingLayout;

pub trait AsBindingLayout {
    // TODO (also macro)
}
