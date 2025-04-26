use pulz_ecs::impl_any_cast;

use crate::{DisplayHandle, Window, WindowHandle, WindowId};

pub trait WindowSystemListener: 'static {
    fn on_created(
        &mut self,
        _window_id: WindowId,
        _window_props: &Window,
        _display_handle: DisplayHandle<'_>,
        _window_handle: WindowHandle<'_>,
    ) {
    }
    fn on_closed(&mut self, _window_id: WindowId) {}
    fn on_resumed(&mut self) {}
    fn on_suspended(&mut self) {}
}

impl_any_cast!(dyn WindowSystemListener);
