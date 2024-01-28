use std::rc::Rc;

use pulz_ecs::impl_any_cast;

use crate::{HasWindowAndDisplayHandle, Window, WindowId};

pub trait WindowSystemListener: 'static {
    fn on_created(
        &mut self,
        _window_id: WindowId,
        _window_desc: &Window,
        _window: Rc<dyn HasWindowAndDisplayHandle>,
    ) {
    }
    fn on_closed(&mut self, _window_id: WindowId) {}
    fn on_resumed(&mut self) {}
    fn on_suspended(&mut self) {}
}

impl_any_cast!(dyn WindowSystemListener);
