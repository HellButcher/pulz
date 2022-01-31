use crate::math::Mat4;
use crevice::std140::AsStd140;

use transform::components::GlobalTransform;

pub mod surface;

pub struct View {
    pub projection: Mat4,
    pub transform: GlobalTransform,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, AsStd140)]
struct ViewUniform {
    view_proj: mint::ColumnMatrix4<f32>,
    projection: mint::ColumnMatrix4<f32>,
    world_position: mint::Vector3<f32>,
}

impl View {
    #[inline]
    fn to_uniform(&self) -> ViewUniform {
        let view_proj = self.projection * self.transform.to_matrix().inverse();
        ViewUniform {
            view_proj: view_proj.into(),
            projection: self.projection.into(),
            world_position: self.transform.translation.into(),
        }
    }
}
