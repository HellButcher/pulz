use std::any::TypeId;

use crate::{resource::Resources, schedule::Schedule};

pub trait ModuleWithOutput: Sized + 'static {
    type Output<'l>;

    fn install_modules(&self, _resources: &mut Resources) {}
    fn install_once(&self, _resources: &mut Resources) {}
    fn install_resources(self, _resources: &mut Resources) -> Self::Output<'_>;
    fn install_systems(_schedule: &mut Schedule) {}

    #[inline]
    fn install(self, resources: &mut Resources) -> Self::Output<'_> {
        let is_first = resources.modules.insert(TypeId::of::<Self>());
        if is_first {
            self.install_modules(resources);
            self.install_once(resources);
     
            let resources_mut: *mut Resources = resources;
            let mut schedule = resources.remove::<Schedule>().unwrap();
            let output = self.install_resources(resources);
            Self::install_systems(&mut schedule);
            // SAFETY: will not access schedule, because it was removed
            unsafe {&mut *resources_mut}.insert_again(schedule);
            output
        } else {
            self.install_resources(resources)
        }
    }
}

pub trait Module: Sized + 'static {
    fn install_modules(&self, _resources: &mut Resources) {}
    fn install_once(&self, _resources: &mut Resources) {}
    fn install_resources(self, _resources: &mut Resources) {}
    fn install_systems(_schedule: &mut Schedule) {}
}

impl<M: Module> ModuleWithOutput for M {
    type Output<'l> = ();
    #[inline]
    fn install_modules(&self, resources: &mut Resources) {
        M::install_modules(self, resources)
    }
    #[inline]
    fn install_once(&self, resources: &mut Resources) {
        M::install_once(self, resources)
    }
    #[inline]
    fn install_resources(self, resources: &mut Resources) {
        M::install_resources(self, resources)
    }
    #[inline]
    fn install_systems(schedule: &mut Schedule) {
        M::install_systems(schedule)
    }
}

impl<F> Module for F
where
    F: FnOnce(&mut Resources) + 'static,
{
    #[inline]
    fn install_resources(self, resources: &mut Resources) {
        self(resources)
    }
}

impl Resources {
    #[inline]
    pub fn install<M: ModuleWithOutput>(&mut self, module: M) -> M::Output::<'_> {
        module.install(self)
    }
}
