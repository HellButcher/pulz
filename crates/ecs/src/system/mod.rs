use crate::World;

pub mod param;
pub mod system_fn;

pub trait System: Send + Sync {
    fn initialize(&mut self, _world: &mut World) {}
    fn run(&mut self, arg: &World);
}

pub trait ExclusiveSystem {
    fn initialize(&mut self, _world: &mut World) {}
    fn run(&mut self, arg: &mut World);
}

pub trait IntoSystem<'l, Marker: 'l> {
    fn into_system(self) -> SystemDescriptor<'l>;
}

pub struct SystemDescriptor<'l> {
    pub(crate) system_variant: SystemVariant<'l>,
    pub(crate) dependencies: Vec<usize>,
}

impl SystemDescriptor<'_> {
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

pub(crate) enum SystemVariant<'l> {
    Exclusive(Box<dyn ExclusiveSystem + 'l>),
    Concurrent(Box<dyn System + 'l>),
}

impl<S> System for Box<S>
where
    S: System,
{
    #[inline]
    fn run(&mut self, arg: &World) {
        self.as_mut().run(arg)
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
pub struct ConcurrentAsExclusiveSystem<'l>(Box<dyn System + 'l>);

impl<'l> ExclusiveSystem for ConcurrentAsExclusiveSystem<'l> {
    #[inline]
    fn run(&mut self, arg: &mut World) {
        self.0.run(arg)
    }
}

#[doc(hidden)]
pub struct SystemDescriptorMarker;
impl<'l> IntoSystem<'l, SystemDescriptorMarker> for SystemDescriptor<'l> {
    #[inline]
    fn into_system(self) -> SystemDescriptor<'l> {
        self
    }
}

#[doc(hidden)]
pub struct ConcurrentSystemMarker;
impl<'l, S> IntoSystem<'l, ConcurrentSystemMarker> for S
where
    S: System + 'l,
{
    fn into_system(self) -> SystemDescriptor<'l> {
        SystemDescriptor {
            system_variant: SystemVariant::Concurrent(Box::new(self)),
            dependencies: Vec::new(),
        }
    }
}

#[doc(hidden)]
pub struct ExclusiveSystemMarker;
impl<'l, S> IntoSystem<'l, ExclusiveSystemMarker> for S
where
    S: ExclusiveSystem + 'l,
{
    fn into_system(self) -> SystemDescriptor<'l> {
        SystemDescriptor {
            system_variant: SystemVariant::Exclusive(Box::new(self)),
            dependencies: Vec::new(),
        }
    }
}
