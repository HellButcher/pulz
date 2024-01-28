use pulz_ecs::prelude::*;

use crate::math::{Mat3, Mat4, Quat, Vec3};

macro_rules! define_transform {
    ($Transform:ident) => {
        #[derive(Debug, PartialEq, Clone, Copy, Component)]
        pub struct $Transform {
            pub translation: Vec3,
            pub rotation: Quat,
            pub scale: Vec3,
        }

        impl $Transform {
            pub const IDENTITY: Self = Self::identity();

            #[inline]
            pub fn from_xyz(x: f32, y: f32, z: f32) -> Self {
                Self::from_translation(Vec3::new(x, y, z))
            }

            #[inline]
            pub const fn identity() -> Self {
                Self {
                    translation: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                    scale: Vec3::ONE,
                }
            }

            #[inline]
            pub fn from_matrix(matrix: Mat4) -> Self {
                let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
                Self {
                    translation,
                    rotation,
                    scale,
                }
            }

            #[inline]
            pub const fn from_translation(translation: Vec3) -> Self {
                Self {
                    translation,
                    ..Self::identity()
                }
            }

            #[inline]
            pub const fn from_rotation(rotation: Quat) -> Self {
                Self {
                    rotation,
                    ..Self::identity()
                }
            }

            #[inline]
            pub const fn from_scale(scale: Vec3) -> Self {
                Self {
                    scale,
                    ..Self::identity()
                }
            }

            #[inline]
            pub fn looking_at(mut self, target: Vec3, up: Vec3) -> Self {
                self.look_at(target, up);
                self
            }

            #[inline]
            pub fn to_matrix(&self) -> Mat4 {
                Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
            }

            #[inline]
            pub fn rotate(&mut self, rotation: Quat) {
                self.rotation *= rotation;
            }

            #[inline]
            pub fn look_at(&mut self, target: Vec3, up: Vec3) {
                let forward = Vec3::normalize(self.translation - target);
                let right = up.cross(forward).normalize();
                let up = forward.cross(right);
                self.rotation = Quat::from_mat3(&Mat3::from_cols(right, up, forward));
            }
        }

        impl Default for $Transform {
            #[inline]
            fn default() -> Self {
                Self::identity()
            }
        }

        impl std::ops::Mul<Self> for $Transform {
            type Output = Self;

            #[inline]
            fn mul(self, transform: Self) -> Self::Output {
                let translation = self * transform.translation;
                let rotation = self.rotation * transform.rotation;
                let scale = self.scale * transform.scale;
                Self {
                    translation,
                    rotation,
                    scale,
                }
            }
        }

        impl std::ops::Mul<Self> for &$Transform {
            type Output = $Transform;

            #[inline]
            fn mul(self, transform: Self) -> Self::Output {
                let translation = self * transform.translation;
                let rotation = self.rotation * transform.rotation;
                let scale = self.scale * transform.scale;
                $Transform {
                    translation,
                    rotation,
                    scale,
                }
            }
        }

        impl std::ops::Mul<Vec3> for $Transform {
            type Output = Vec3;

            #[inline]
            fn mul(self, mut value: Vec3) -> Self::Output {
                value = self.rotation * value;
                value = self.scale * value;
                value += self.translation;
                value
            }
        }

        impl std::ops::Mul<Vec3> for &$Transform {
            type Output = Vec3;

            #[inline]
            fn mul(self, mut value: Vec3) -> Self::Output {
                value = self.rotation * value;
                value = self.scale * value;
                value += self.translation;
                value
            }
        }
    };
}

define_transform!(Transform);
define_transform!(GlobalTransform);

impl From<GlobalTransform> for Transform {
    fn from(transform: GlobalTransform) -> Self {
        Self {
            translation: transform.translation,
            rotation: transform.rotation,
            scale: transform.scale,
        }
    }
}

impl From<Transform> for GlobalTransform {
    fn from(transform: Transform) -> Self {
        Self {
            translation: transform.translation,
            rotation: transform.rotation,
            scale: transform.scale,
        }
    }
}
