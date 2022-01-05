use crate::resource::{Resources, ResourcesSend};

pub mod param;
pub mod system_fn;

/// # Safety
/// when is_send returns true, the implemention of run must ensure, that no unsend resources are accessed.
/// The `is_send` method must not return `true`, when unsend resources are accessed!
pub unsafe trait System: Send + Sync + 'static {
    fn initialize(&mut self, _resources: &mut Resources) {}
    fn run(&mut self, arg: &Resources);

    fn is_send(&self) -> bool;

    fn run_send(&mut self, arg: &ResourcesSend) {
        assert!(self.is_send(), "system is not send");
        // SAFETY: no unsend resources are accessed (defined by unsafe trait contract)
        unsafe { self.run(arg.as_unsend()) }
    }
}

pub trait ExclusiveSystem: 'static {
    fn initialize(&mut self, _resources: &mut Resources) {}
    fn run(&mut self, arg: &mut Resources);
}

pub trait IntoSystem<Marker> {
    fn into_system(self) -> SystemDescriptor;
}

pub struct SystemDescriptor {
    pub(crate) system_variant: SystemVariant,
    pub(crate) dependencies: Vec<usize>,
    pub(crate) initialized: bool,
}

impl SystemDescriptor {
    pub fn exclusive(self) -> Self {
        match self.system_variant {
            SystemVariant::Exclusive(_) => self,
            SystemVariant::Concurrent(system) => Self {
                system_variant: SystemVariant::Exclusive(Box::new(ConcurrentAsExclusiveSystem(
                    system,
                ))),
                dependencies: self.dependencies,
                initialized: self.initialized,
            },
        }
    }

    pub fn initialize(&mut self, resources: &mut Resources) {
        if !self.initialized {
            self.initialized = true;
            match self.system_variant {
                SystemVariant::Exclusive(ref mut system) => system.initialize(resources),
                SystemVariant::Concurrent(ref mut system) => system.initialize(resources),
            }
        }
    }

    pub fn run(&mut self, resources: &mut Resources) {
        self.initialize(resources);
        match self.system_variant {
            SystemVariant::Exclusive(ref mut system) => system.run(resources),
            SystemVariant::Concurrent(ref mut system) => system.run(resources),
        }
    }
}

pub(crate) enum SystemVariant {
    Exclusive(Box<dyn ExclusiveSystem>),
    Concurrent(Box<dyn System>),
}

unsafe impl<S> System for Box<S>
where
    S: System,
{
    #[inline]
    fn run(&mut self, arg: &Resources) {
        self.as_mut().run(arg)
    }

    #[inline]
    fn is_send(&self) -> bool {
        self.as_ref().is_send()
    }
}

impl<S> ExclusiveSystem for Box<S>
where
    S: ExclusiveSystem,
{
    #[inline]
    fn run(&mut self, arg: &mut Resources) {
        self.as_mut().run(arg)
    }
}

#[doc(hidden)]
pub struct ConcurrentAsExclusiveSystem(Box<dyn System>);

impl ExclusiveSystem for ConcurrentAsExclusiveSystem {
    #[inline]
    fn run(&mut self, arg: &mut Resources) {
        self.0.run(arg)
    }
}

#[doc(hidden)]
pub struct SystemDescriptorMarker;
impl IntoSystem<SystemDescriptorMarker> for SystemDescriptor {
    #[inline]
    fn into_system(self) -> SystemDescriptor {
        self
    }
}

#[doc(hidden)]
pub struct ConcurrentSystemMarker;
impl<S> IntoSystem<ConcurrentSystemMarker> for S
where
    S: System,
{
    fn into_system(self) -> SystemDescriptor {
        SystemDescriptor {
            system_variant: SystemVariant::Concurrent(Box::new(self)),
            dependencies: Vec::new(),
            initialized: false,
        }
    }
}

#[doc(hidden)]
pub struct ExclusiveSystemMarker;
impl<S> IntoSystem<ExclusiveSystemMarker> for S
where
    S: ExclusiveSystem,
{
    fn into_system(self) -> SystemDescriptor {
        SystemDescriptor {
            system_variant: SystemVariant::Exclusive(Box::new(self)),
            dependencies: Vec::new(),
            initialized: false,
        }
    }
}
