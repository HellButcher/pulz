use std::{borrow::Cow, collections::VecDeque};

use pulz_ecs::{module::ModuleWithOutput, Component};
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
pub struct WindowAttributes {
    pub size: Option<Size2>,
    pub title: Cow<'static, str>,
    pub vsync: bool,
}

pub struct Window {
    pub size: Size2,
    pub scale_factor: f64,
    pub vsync: bool,
    pub is_pending: bool,
    pub is_close_requested: bool,
    command_queue: VecDeque<WindowCommand>,
}

pub struct Windows {
    windows: SlotMap<WindowId, Window>,
    created: VecDeque<(WindowId, WindowAttributes)>,
}

impl WindowAttributes {
    pub const fn new() -> Self {
        Self {
            size: None,
            title: Cow::Borrowed(""),
            vsync: true,
        }
    }
}

impl Default for WindowAttributes {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Window {
    #[inline]
    pub const fn new() -> Self {
        Self {
            size: Size2::ZERO,
            scale_factor: 1.0,
            vsync: true,
            is_pending: true,
            is_close_requested: false,
            command_queue: VecDeque::new(),
        }
    }

    pub fn from_attributes(attributes: &WindowAttributes) -> Self {
        Self {
            size: attributes.size.unwrap_or(Size2::ZERO),
            vsync: attributes.vsync,
            ..Self::new()
        }
    }
}

impl Default for Window {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Windows {
    pub fn new() -> Self {
        Self {
            windows: SlotMap::with_key(),
            created: VecDeque::new(),
        }
    }

    #[inline]
    pub fn create(&mut self, attributes: WindowAttributes) -> WindowId {
        let window = Window::from_attributes(&attributes);
        let id = self.windows.insert(window);
        self.created.push_back((id, attributes));
        id
    }

    #[doc(hidden)]
    pub fn create_new(&mut self) -> (WindowId, &mut Window) {
        let id = self.windows.insert(Window::new());
        let window = self.get_mut(id).unwrap();
        (id, window)
    }

    #[doc(hidden)]
    pub fn pop_next_window_to_create(
        &mut self,
    ) -> Option<(WindowId, &mut Window, WindowAttributes)> {
        let (id, attributes) = loop {
            let (id, attributes) = self.created.pop_front()?;
            if self.windows.contains_key(id) {
                break (id, attributes);
            }
        };
        let window = self.windows.get_mut(id).unwrap();
        Some((id, window, attributes))
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    #[inline]
    pub fn get(&self, id: WindowId) -> Option<&Window> {
        self.windows.get(id)
    }

    #[inline]
    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.windows.get_mut(id)
    }

    #[inline]
    pub fn close(&mut self, id: WindowId) -> bool {
        self.windows.remove(id).is_some()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        self.windows.iter()
    }

    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_> {
        self.windows.iter_mut()
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
        &self.windows[id]
    }
}

impl std::ops::IndexMut<WindowId> for Windows {
    #[inline]
    fn index_mut(&mut self, id: WindowId) -> &mut Self::Output {
        &mut self.windows[id]
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WindowCommand {
    SetTitle(Cow<'static, String>),
    SetVisible(bool),
    SetFullscreen(bool),
    Close,
}

pub struct WindowModule;

impl ModuleWithOutput for WindowModule {
    type Output<'l> = &'l mut Windows;

    fn install_resources(self, resources: &mut pulz_ecs::prelude::Resources) -> Self::Output<'_> {
        let id = resources.init::<Windows>();
        resources.get_mut_id(id).unwrap()
    }
}
