use std::{
    borrow::Cow,
    collections::VecDeque,
    ops::{Deref, DerefMut},
    rc::Weak,
};

use pulz_ecs::Component;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use slotmap::{new_key_type, SlotMap};

use crate::Size2;

new_key_type! {
    #[derive(Component)]
    pub struct WindowId;
}

pub type Iter<'a, T = Window> = slotmap::basic::Iter<'a, WindowId, T>;
pub type IterMut<'a, T = Window> = slotmap::basic::IterMut<'a, WindowId, T>;
pub type WindowsMirror<T> = slotmap::SecondaryMap<WindowId, T>;

#[derive(Debug)]
pub struct WindowDescriptor {
    pub size: Size2,
    pub scale_factor: f64,
    pub title: Cow<'static, str>,
    pub vsync: bool,
}

pub struct Window {
    descriptor: WindowDescriptor,
    pub close_requested: bool,
    command_queue: VecDeque<WindowCommand>,
}

pub struct Windows(SlotMap<WindowId, Window>);

impl WindowDescriptor {
    pub const DEFAULT_TITLE: &'static str =
        concat!(env!("CARGO_PKG_NAME"), ": ", env!("CARGO_PKG_VERSION"));
    #[inline]
    pub fn new() -> Self {
        Self {
            size: Size2::ZERO,
            scale_factor: 1.0,
            title: Cow::Borrowed(Self::DEFAULT_TITLE),
            vsync: true,
        }
    }
}

impl Default for WindowDescriptor {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for Window {
    type Target = WindowDescriptor;
    fn deref(&self) -> &Self::Target {
        &self.descriptor
    }
}

impl DerefMut for Window {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.descriptor
    }
}

impl Windows {
    pub fn new() -> Self {
        Self(SlotMap::with_key())
    }

    #[inline]
    pub fn create(&mut self, descriptor: WindowDescriptor) -> WindowId {
        let window = Window {
            descriptor,
            close_requested: false,
            command_queue: VecDeque::new(),
        };

        //self.new_windows.push_back(id);
        self.0.insert(window)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn get(&self, id: WindowId) -> Option<&Window> {
        self.0.get(id)
    }

    #[inline]
    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.0.get_mut(id)
    }

    #[inline]
    pub fn close(&mut self, id: WindowId) -> bool {
        self.0.remove(id).is_some()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        self.0.iter()
    }

    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_> {
        self.0.iter_mut()
    }
}

impl Default for Windows {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Index<WindowId> for Windows {
    type Output = Window;
    #[inline]
    fn index(&self, id: WindowId) -> &Self::Output {
        &self.0[id]
    }
}

impl std::ops::IndexMut<WindowId> for Windows {
    #[inline]
    fn index_mut(&mut self, id: WindowId) -> &mut Self::Output {
        &mut self.0[id]
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WindowCommand {
    SetTitle(Cow<'static, String>),
    Close,
}

pub trait RawWindow: HasRawWindowHandle + HasRawDisplayHandle {}

impl<W> RawWindow for W where W: HasRawWindowHandle + HasRawDisplayHandle {}

pub type RawWindowHandles = WindowsMirror<Weak<dyn RawWindow>>;
