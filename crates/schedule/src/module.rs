//! The [`Module`] trait for encapsulating related resource and system registration.

use std::any::TypeId;

pub use pulz_schedule_macros::system_module;

use crate::resource::Resources;

/// A self-contained unit of resource and system registration that installs itself exactly once.
///
/// Implement [`Module::init`] to register resources, systems, and other modules.
/// Call [`Resources::install`] to install; duplicate calls are no-ops.
pub trait Module: Sized + 'static {
    /// Performs the one-time registration of resources and systems.
    fn init(self, _resources: &mut Resources) {}

    /// Installs this module into `resources`, returning `true` if this was the first call.
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
    /// Installs `module`, running its [`Module::init`] exactly once.
    #[inline]
    pub fn install<M: Module>(&mut self, module: M) -> bool {
        module.install(self)
    }
}
