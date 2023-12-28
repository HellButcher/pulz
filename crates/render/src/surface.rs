use fnv::FnvHashMap as HashMap;
use pulz_transform::math::{Size2, USize2};
use pulz_window::{WindowId, WindowsMirror};

use crate::texture::{Texture, TextureFormat};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Surface {
    pub texture: Texture,
    pub format: TextureFormat,
    pub physical_size: USize2,
    pub scale_factor: f64,
}

impl Surface {
    #[inline]
    pub fn to_logical_size(&self, physical_size: USize2) -> Size2 {
        (physical_size.as_dvec2() / self.scale_factor).as_vec2()
    }

    #[inline]
    pub fn logical_size(&self) -> Size2 {
        self.to_logical_size(self.physical_size)
    }

    #[inline]
    pub fn physical_size(&self) -> USize2 {
        self.physical_size
    }
}

pub type Iter<'a, T = Surface> = slotmap::secondary::Iter<'a, WindowId, T>;
pub type IterMut<'a, T = Surface> = slotmap::secondary::IterMut<'a, WindowId, T>;

pub struct WindowSurfaces {
    surfaces: WindowsMirror<Surface>,
    by_texture: HashMap<Texture, WindowId>,
}

impl WindowSurfaces {
    pub fn new() -> Self {
        Self {
            surfaces: WindowsMirror::new(),
            by_texture: HashMap::default(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.surfaces.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }

    #[inline]
    pub fn get(&self, id: WindowId) -> Option<&Surface> {
        self.surfaces.get(id)
    }

    #[inline]
    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut Surface> {
        self.surfaces.get_mut(id)
    }

    #[inline]
    pub fn remove(&mut self, id: WindowId) -> bool {
        self.surfaces.remove(id).is_some()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        self.surfaces.iter()
    }

    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_> {
        self.surfaces.iter_mut()
    }
}

impl Default for WindowSurfaces {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Index<WindowId> for WindowSurfaces {
    type Output = Surface;
    #[inline]
    fn index(&self, id: WindowId) -> &Self::Output {
        &self.surfaces[id]
    }
}

impl std::ops::IndexMut<WindowId> for WindowSurfaces {
    #[inline]
    fn index_mut(&mut self, id: WindowId) -> &mut Self::Output {
        &mut self.surfaces[id]
    }
}
