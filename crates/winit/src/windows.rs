use std::collections::HashMap;

pub use winit::window::{Window as WinitWindow, WindowId as WinitWindowId};

/// Stores all open winit windows, keyed by their [`WinitWindowId`].
#[derive(Default)]
pub struct WinitWindows {
    windows: HashMap<WinitWindowId, WinitWindow>,
    _phantom: core::marker::PhantomData<*const ()>, // !Send + !Sync
}

fn is_visible(window: &WinitWindow) -> bool {
    window.is_visible().unwrap_or(true) && !window.is_minimized().unwrap_or(false)
}

impl WinitWindows {
    /// Creates an empty window registry.
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            _phantom: core::marker::PhantomData,
        }
    }

    /// Iterates over all open windows.
    pub fn iter(&self) -> impl Iterator<Item = &WinitWindow> {
        self.windows.values()
    }

    /// Returns `true` if at least one window is visible and not minimized.
    pub fn is_visible(&self) -> bool {
        self.windows.values().any(is_visible)
    }

    /// Returns `true` if at least one visible window currently has keyboard focus.
    pub fn has_focus(&self) -> bool {
        self.windows
            .values()
            .any(|window| window.has_focus() && is_visible(window))
    }

    /// Requests a redraw on all visible windows.
    pub fn request_redraw(&self) {
        for window in self.windows.values() {
            if is_visible(window) {
                window.request_redraw();
            }
        }
    }
}
