use pulz_ecs::resource::Resources;
use slotmap::SlotMap;

use crate::{RawWindow, Window, WindowId};

pub trait WindowSystemListener: 'static {
    fn on_created(
        &self,
        _res: &Resources,
        _window_id: WindowId,
        _window_desc: &Window,
        _window_raw: &dyn RawWindow,
    ) {
    }
    fn on_resumed(&self, _res: &Resources) {}
    fn on_closed(&self, _res: &Resources, _window_id: WindowId) {}
    fn on_suspended(&self, _res: &Resources) {}
}

slotmap::new_key_type! {
   pub struct WindowSystemListenerId;
}

#[derive(Default)]
pub struct WindowSystemListeners(SlotMap<WindowSystemListenerId, Box<dyn WindowSystemListener>>);

impl WindowSystemListeners {
    #[inline]
    pub fn insert(&mut self, l: impl WindowSystemListener) -> WindowSystemListenerId {
        self.0.insert(Box::new(l))
    }

    #[inline]
    pub fn remove(&mut self, id: WindowSystemListenerId) -> bool {
        self.0.remove(id).is_some()
    }
    pub fn call_on_created(
        &self,
        res: &Resources,
        window_id: WindowId,
        window_descr: &Window,
        window_raw: &dyn RawWindow,
    ) {
        for (_, l) in self.0.iter() {
            l.on_created(res, window_id, window_descr, window_raw);
        }
    }
    pub fn call_on_resumed(&self, res: &Resources) {
        for (_, l) in self.0.iter() {
            l.on_resumed(res);
        }
    }
    pub fn call_on_closed(&self, res: &Resources, window_id: WindowId) {
        for (_, l) in self.0.iter() {
            l.on_closed(res, window_id);
        }
    }
    pub fn call_on_suspended(&self, res: &Resources) {
        for (_, l) in self.0.iter() {
            l.on_suspended(res);
        }
    }
}
