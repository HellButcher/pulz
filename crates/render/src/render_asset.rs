use crate::backend::RenderBackend;

pub trait RenderAsset<B: RenderBackend> {
    type Target;

    fn prepare(&self, backend: &mut B) -> Self::Target;
}
