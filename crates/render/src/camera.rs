use pulz_assets::{Assets, Handle};
use pulz_ecs::prelude::*;
use pulz_transform::math::{size2, Mat4, Size2};
use pulz_window::WindowId;

use crate::{
    surface::{Surface, WindowSurfaces},
    texture::Image,
};

trait AsProjectionMatrix {
    fn as_projection_matrix(&self) -> Mat4;
    fn update_viewport(&mut self, size: Size2) -> bool;
    fn far(&self) -> f32;
    fn zorder_optimization(&self) -> bool;
}

#[derive(Component)]
pub enum Projection {
    Perspective(PerspectiveProjection),
    Orthographic(OrthographicProjection),
}

impl AsProjectionMatrix for Projection {
    #[inline]
    fn as_projection_matrix(&self) -> Mat4 {
        match self {
            Self::Perspective(p) => p.as_projection_matrix(),
            Self::Orthographic(p) => p.as_projection_matrix(),
        }
    }
    #[inline]
    fn update_viewport(&mut self, size: Size2) -> bool {
        match self {
            Self::Perspective(p) => p.update_viewport(size),
            Self::Orthographic(p) => p.update_viewport(size),
        }
    }
    #[inline]
    fn far(&self) -> f32 {
        match self {
            Self::Perspective(p) => p.far(),
            Self::Orthographic(p) => p.far(),
        }
    }
    #[inline]
    fn zorder_optimization(&self) -> bool {
        match self {
            Self::Perspective(p) => p.zorder_optimization(),
            Self::Orthographic(p) => p.zorder_optimization(),
        }
    }
}

impl Default for Projection {
    #[inline]
    fn default() -> Self {
        Self::Perspective(Default::default())
    }
}

pub struct PerspectiveProjection {
    pub fov: f32,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
}

impl AsProjectionMatrix for PerspectiveProjection {
    #[inline]
    fn as_projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov, self.aspect_ratio, self.near, self.far)
    }
    #[inline]
    fn update_viewport(&mut self, size: Size2) -> bool {
        let new_aspect_ratio = size.x / size.y;
        if self.aspect_ratio != new_aspect_ratio {
            self.aspect_ratio = new_aspect_ratio;
            true
        } else {
            false
        }
    }
    #[inline]
    fn far(&self) -> f32 {
        self.far
    }
    fn zorder_optimization(&self) -> bool {
        false
    }
}

impl Default for PerspectiveProjection {
    fn default() -> Self {
        Self {
            fov: std::f32::consts::PI / 4.0,
            near: 0.1,
            far: 1000.0,
            aspect_ratio: 1.0,
        }
    }
}

pub enum OrthographicOrigin {
    Center,
    BottomLeft,
}

pub enum OrthographicScalingMode {
    // use manually specified values of left/right/bottom/top as they are.
    // the image will stretch wit the window!
    Manual,
    // use the window size
    WindowSize,
    // fits the given rect inside the window while keeping the aspect-ratio of the window.
    AutoFit(Size2),
}

pub struct OrthographicProjection {
    pub left: f32,
    pub right: f32,
    pub bottom: f32,
    pub top: f32,
    pub near: f32,
    pub far: f32,
    pub scaling_mode: OrthographicScalingMode,
    pub origin: OrthographicOrigin,
}

impl AsProjectionMatrix for OrthographicProjection {
    fn as_projection_matrix(&self) -> Mat4 {
        Mat4::orthographic_rh(
            self.left,
            self.right,
            self.bottom,
            self.top,
            self.near,
            self.far,
        )
    }
    fn update_viewport(&mut self, size: Size2) -> bool {
        let adjusted_size = match self.scaling_mode {
            OrthographicScalingMode::Manual => return false,
            OrthographicScalingMode::WindowSize => size,
            OrthographicScalingMode::AutoFit(min) => {
                if size.x * min.y > min.x * size.y {
                    size2(size.x * min.y / size.y, min.y)
                } else {
                    size2(min.x, size.y * min.x / size.x)
                }
            }
        };
        match self.origin {
            OrthographicOrigin::Center => {
                let half = adjusted_size / 2.0;
                self.left = -half.x;
                self.right = half.x;
                self.bottom = -half.y;
                self.top = half.y;
                if let OrthographicScalingMode::WindowSize = self.scaling_mode {
                    self.left = self.left.floor();
                    self.right = self.right.floor();
                    self.bottom = self.bottom.floor();
                    self.top = self.top.floor();
                }
            }
            OrthographicOrigin::BottomLeft => {
                self.left = 0.0;
                self.right = adjusted_size.x;
                self.bottom = 0.0;
                self.top = adjusted_size.y;
            }
        }
        true
    }
    fn far(&self) -> f32 {
        self.far
    }
    fn zorder_optimization(&self) -> bool {
        true
    }
}

impl Default for OrthographicProjection {
    #[inline]
    fn default() -> Self {
        Self {
            left: -1.0,
            right: 1.0,
            bottom: -1.0,
            top: 1.0,
            near: 0.0,
            far: 1000.0,
            scaling_mode: OrthographicScalingMode::WindowSize,
            origin: OrthographicOrigin::Center,
        }
    }
}

impl From<PerspectiveProjection> for Projection {
    fn from(p: PerspectiveProjection) -> Self {
        Self::Perspective(p)
    }
}

impl From<OrthographicProjection> for Projection {
    fn from(p: OrthographicProjection) -> Self {
        Self::Orthographic(p)
    }
}

#[derive(Component)]
pub struct Camera {
    pub order: isize,
    zorder_optimization: bool,
    pub projection_matrix: Mat4,
}

impl Default for Camera {
    fn default() -> Self {
        Self::new()
    }
}

impl Camera {
    pub const fn new() -> Self {
        Self {
            order: 0,
            zorder_optimization: false,
            projection_matrix: Mat4::IDENTITY,
        }
    }
}

#[derive(Component, Copy, Clone, Debug)]
pub enum RenderTarget {
    Window(WindowId),
    Image(Handle<Image>),
}

impl RenderTarget {
    pub fn resolve(&self, surfaces: &WindowSurfaces, _images: &Assets<Image>) -> Option<Surface> {
        match self {
            Self::Window(window_id) => surfaces.get(*window_id).copied(),
            Self::Image(_image_handle) => todo!("surface from image asset"),
        }
    }
}

pub fn update_projections_from_render_targets(
    window_surfaces: &'_ WindowSurfaces,
    images: &'_ Assets<Image>,
    mut projections: Query<'_, (&'_ mut Projection, &'_ RenderTarget)>,
) {
    for (projection, render_target) in projections.iter() {
        if let Some(surface) = render_target.resolve(window_surfaces, images) {
            projection.update_viewport(surface.logical_size());
        }
    }
}

pub fn update_cameras_from_projections(mut cameras: Query<'_, (&'_ mut Camera, &'_ Projection)>) {
    for (camera, projection) in cameras.iter() {
        camera.zorder_optimization = projection.zorder_optimization();
        camera.projection_matrix = projection.as_projection_matrix();
    }
}
