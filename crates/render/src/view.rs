use std::ops::Range;

use pulz_ecs::Component;
use pulz_transform::{
    components::GlobalTransform,
    math::{USize2, UVec2, Vec2, Vec3},
};

use crate::{math::Mat4, shader::ShaderType, texture::Texture};

pub struct View {
    pub projection: Mat4,
    pub transform: GlobalTransform,
    pub size: USize2,
}

#[derive(Clone, ShaderType)]
struct ViewUniform {
    view_proj: Mat4,
    inverse_view_projection: Mat4,
    view: Mat4,
    inverse_view: Mat4,
    projection: Mat4,
    inverse_projection: Mat4,
    world_position: Vec3,
    size: Vec2,
}

impl View {
    #[inline]
    fn to_uniform(&self) -> ViewUniform {
        let projection = self.projection;
        let inverse_projection = projection.inverse();
        let view = self.transform.to_matrix();
        let inverse_view = view.inverse();
        let view_proj = projection * inverse_view;
        let inverse_view_projection = view * inverse_projection;
        let world_position = self.transform.translation;
        let size = [self.size.x as f32, self.size.y as f32];
        ViewUniform {
            view_proj,
            inverse_view_projection,
            view,
            inverse_view,
            projection,
            inverse_projection,
            world_position,
            size: size.into(),
        }
    }
}

#[derive(Component)]
pub struct ViewTarget {
    pub target: Texture,
    pub sampled: Option<Texture>,
    pub depth: Option<Texture>,
}

#[derive(Component)]
pub struct Viewport {
    pub position: UVec2,
    pub size: USize2,
    pub depth: Range<f32>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Msaa {
    pub samples: u32,
}

impl Default for Msaa {
    fn default() -> Self {
        Self { samples: 4 }
    }
}
