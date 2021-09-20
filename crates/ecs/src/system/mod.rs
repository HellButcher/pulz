use pulz_executor::Executor;

use crate::{world::WorldSend, World};

pub mod param;
pub mod system_fn;

/// # Unsafe
/// when is_send returns true, the implemention of run must ensure, that no unsend resources are accessed
pub unsafe trait System: Send + Sync + 'static {
    fn initialize(&mut self, _world: &mut World) {}
    fn run(&mut self, arg: &World);

    fn is_send(&self) -> bool;

    fn run_send(&mut self, arg: &WorldSend) {
        assert!(self.is_send(), "system is not send");
        // SAFETY: no unsend resources are accessed (defined by unsafe trait contract)
        unsafe { self.run(arg.as_unsend()) }
    }
}

pub trait ExclusiveSystem: 'static {
    fn initialize(&mut self, _world: &mut World) {}
    fn run(&mut self, arg: &mut World);
}

pub trait IntoSystem<Marker> {
    fn into_system(self) -> SystemDescriptor;
}

pub struct SystemDescriptor {
    pub(crate) system_variant: SystemVariant,
    pub(crate) dependencies: Vec<usize>,
}

impl SystemDescriptor {
    pub fn exclusive(self) -> Self {
        match self.system_variant {
            SystemVariant::Exclusive(_) => self,
            SystemVariant::Concurrent(system) => SystemDescriptor {
                system_variant: SystemVariant::Exclusive(Box::new(ConcurrentAsExclusiveSystem(
                    system,
                ))),
                dependencies: self.dependencies,
            },
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
    fn run(&mut self, arg: &World) {
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
    fn run(&mut self, arg: &mut World) {
        self.as_mut().run(arg)
    }
}

#[doc(hidden)]
pub struct ConcurrentAsExclusiveSystem(Box<dyn System>);

impl ExclusiveSystem for ConcurrentAsExclusiveSystem {
    #[inline]
    fn run(&mut self, arg: &mut World) {
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
        }
    }
}
