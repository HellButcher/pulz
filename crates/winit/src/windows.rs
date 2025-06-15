use std::collections::HashMap;

pub use winit::window::{Window as WinitWindow, WindowId as WinitWindowId};

#[derive(Default)]
pub struct WinitWindows {
    windows: HashMap<WinitWindowId, WinitWindow>,
    _phantom: core::marker::PhantomData<*const ()>, // !Send + !Sync
}

fn is_visible(window: &WinitWindow) -> bool {
    window.is_visible().unwrap_or(true) && !window.is_minimized().unwrap_or(false)
}

impl WinitWindows {
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            _phantom: core::marker::PhantomData,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &WinitWindow> {
        self.windows.values()
    }

    pub fn is_visible(&self) -> bool {
        self.windows.values().any(is_visible)
    }

    pub fn has_focus(&self) -> bool {
        self.windows
            .values()
            .any(|window| window.has_focus() && is_visible(window))
    }

    pub fn request_redraw(&self) {
        for window in self.windows.values() {
            if is_visible(window) {
                window.request_redraw();
            }
        }
    }
}
