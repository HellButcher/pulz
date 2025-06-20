use std::any::TypeId;

pub use pulz_schedule_macros::system_module;

use crate::resource::Resources;

pub trait Module: Sized + 'static {
    fn init(self, _resources: &mut Resources) {}

    #[inline]
    fn install(self, resources: &mut Resources) -> bool {
        let is_first = resources.insert_module(TypeId::of::<Self>());
        if is_first {
            self.init(resources);
            true
        } else {
            false
        }
    }
}

impl<F> Module for F
where
    F: FnOnce(&mut Resources) + 'static,
{
    #[inline]
    fn init(self, resources: &mut Resources) {
        self(resources)
    }
}

impl Resources {
    #[inline]
    pub fn install<M: Module>(&mut self, module: M) -> bool {
        module.install(self)
    }
}
