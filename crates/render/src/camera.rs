use pulz_assets::{Assets, Handle};
use pulz_ecs::prelude::*;
use pulz_transform::math::{vec2, Mat4, Size2, USize2};
use pulz_window::{Window, WindowId, Windows};

use crate::texture::Image;

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
                    vec2(size.x * min.y / size.y, min.y)
                } else {
                    vec2(min.x, size.y * min.x / size.x)
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
    target_info: Option<RenderTargetInfo>,
}

impl Camera {
    pub const fn new() -> Self {
        Self {
            order: 0,
            zorder_optimization: false,
            projection_matrix: Mat4::IDENTITY,
            target_info: None,
        }
    }
    pub fn to_logical_size(&self, physical_size: USize2) -> Size2 {
        let scale_factor = self.target_info.as_ref().map_or(1.0, |t| t.scale_factor);
        (physical_size.as_dvec2() / scale_factor).as_vec2()
    }

    #[inline]
    pub fn logical_target_size(&self) -> Option<Size2> {
        self.target_info
            .as_ref()
            .map(|t| self.to_logical_size(t.physical_size))
    }

    #[inline]
    pub fn physical_target_size(&self) -> Option<USize2> {
        self.target_info.as_ref().map(|t| t.physical_size)
    }
}

#[derive(Component)]
pub enum RenderTarget {
    Window(WindowId),
    Image(Handle<Image>),
}

#[derive(Copy, Clone, PartialEq)]
struct RenderTargetInfo {
    pub physical_size: USize2,
    pub scale_factor: f64,
}

impl RenderTargetInfo {
    #[inline]
    fn from_window(window: &Window) -> Self {
        Self {
            physical_size: window.size,
            scale_factor: window.scale_factor,
        }
    }

    #[inline]
    fn from_image(image: &Image) -> Self {
        Self {
            physical_size: image.descriptor.dimensions.subimage_extents(),
            scale_factor: 1.0,
        }
    }
}

pub fn update_cameras(
    windows: Res<'_, Windows>,
    images: Res<'_, Assets<Image>>,
    mut cameras: Query<
        '_,
        (
            &'_ mut Camera,
            Option<&'_ mut Projection>,
            Option<&'_ RenderTarget>,
        ),
    >,
) {
    for (camera, projection, render_target) in cameras.iter() {
        let target_info = match render_target {
            None => None,
            Some(&RenderTarget::Window(window_id)) => {
                windows.get(window_id).map(RenderTargetInfo::from_window)
            }
            Some(&RenderTarget::Image(image_handle)) => {
                images.get(image_handle).map(RenderTargetInfo::from_image)
            }
        };
        let changed = target_info != camera.target_info;
        if changed {
            camera.target_info = target_info;

            if let Some(target_info) = target_info {
                let logical_size =
                    (target_info.physical_size.as_dvec2() / target_info.scale_factor).as_vec2();
                // TODO: viewport size?

                // update projection
                if let Some(projection) = projection {
                    projection.update_viewport(logical_size);
                    camera.zorder_optimization = projection.zorder_optimization();
                    camera.projection_matrix = projection.as_projection_matrix();
                }
            }
        }
    }
}
