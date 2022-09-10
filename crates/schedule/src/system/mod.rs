use crate::resource::{ResourceAccess, Resources, ResourcesSend};

pub mod param;
pub mod system_fn;

/// # Safety
/// when is_send returns true, the implemention of run must ensure, that no unsend resources are accessed.
/// The `is_send` method must not return `true`, when unsend resources are accessed!
pub unsafe trait System<Args = ()>: Send + Sync {
    fn init(&mut self, resources: &mut Resources);
    fn run(&mut self, resources: &Resources, args: Args);

    fn is_send(&self) -> bool;

    fn run_send(&mut self, resources: &ResourcesSend, args: Args) {
        assert!(self.is_send(), "system is not send");
        // SAFETY: no unsend resources are accessed (defined by unsafe trait contract)
        unsafe { self.run(resources.as_unsend(), args) }
    }

    fn update_access(&self, resources: &Resources, access: &mut ResourceAccess);

    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

pub trait ExclusiveSystem<Args = ()> {
    fn init(&mut self, _resources: &mut Resources);
    fn run(&mut self, resources: &mut Resources, args: Args);

    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

pub trait IntoSystem<Args, Marker> {
    type System: System<Args>;

    fn into_system(self) -> Self::System;
}

pub trait IntoExclusiveSystem<Args, Marker> {
    type System: ExclusiveSystem<Args>;

    fn into_exclusive_system(self) -> Self::System;
}

#[doc(hidden)]
pub struct ConcurrentSystemMarker;
impl<S, Args> IntoSystem<Args, ConcurrentSystemMarker> for S
where
    S: System<Args>,
{
    type System = Self;
    #[inline]
    fn into_system(self) -> Self {
        self
    }
}

#[doc(hidden)]
pub struct ExclusiveSystemMarker;
impl<S, Args> IntoExclusiveSystem<Args, ExclusiveSystemMarker> for S
where
    S: ExclusiveSystem<Args>,
{
    type System = Self;
    #[inline]
    fn into_exclusive_system(self) -> Self {
        self
    }
}

pub trait IntoSystemDescriptor<Marker>: Sized {
    fn into_system_descriptor(self) -> SystemDescriptor;
}

#[doc(hidden)]
pub struct SystemFnMarker;
impl<S, Marker> IntoSystemDescriptor<(SystemFnMarker, Marker)> for S
where
    S: IntoSystem<(), Marker>,
    S::System: 'static,
{
    fn into_system_descriptor(self) -> SystemDescriptor {
        SystemDescriptor {
            system_variant: SystemVariant::Concurrent(
                Box::new(self.into_system()),
                ResourceAccess::new(),
            ),
            is_initialized: false,
            is_send: false,
        }
    }
}

#[doc(hidden)]
pub struct ExclusiveSystemFnMarker;
impl<S, Marker> IntoSystemDescriptor<(ExclusiveSystemFnMarker, Marker)> for S
where
    S: IntoExclusiveSystem<(), Marker>,
    S::System: 'static,
{
    fn into_system_descriptor(self) -> SystemDescriptor {
        SystemDescriptor {
            system_variant: SystemVariant::Exclusive(Box::new(self.into_exclusive_system())),
            is_initialized: false,
            is_send: false,
        }
    }
}

pub struct SystemDescriptor {
    pub(crate) system_variant: SystemVariant,
    // TODO: add a mechanism, that tracks identity of resource-set
    is_initialized: bool,
    is_send: bool,
}

impl SystemDescriptor {
    #[inline]
    pub fn is_concurrent(&self) -> bool {
        matches!(self.system_variant, SystemVariant::Concurrent(_, _))
    }

    #[inline]
    pub fn is_exclusive(&self) -> bool {
        matches!(self.system_variant, SystemVariant::Exclusive(_))
    }

    #[inline]
    pub fn type_name(&self) -> &'static str {
        match &self.system_variant {
            SystemVariant::Concurrent(s, _) => s.type_name(),
            SystemVariant::Exclusive(s) => s.type_name(),
        }
    }

    #[inline]
    pub(crate) fn access(&self) -> Option<&ResourceAccess> {
        match &self.system_variant {
            SystemVariant::Concurrent(_, a) => Some(a),
            SystemVariant::Exclusive(_) => None,
        }
    }

    pub fn into_exclusive(self) -> Self {
        match self.system_variant {
            SystemVariant::Exclusive(_) => self,
            SystemVariant::Concurrent(system, _) => Self {
                system_variant: SystemVariant::Exclusive(Box::new(ConcurrentAsExclusiveSystem(
                    system,
                ))),
                is_initialized: self.is_initialized,
                is_send: self.is_send,
            },
        }
    }

    pub fn init(&mut self, resources: &mut Resources) {
        if self.is_initialized {
            return;
        }
        match self.system_variant {
            SystemVariant::Exclusive(ref mut system) => {
                system.init(resources);
                self.is_send = false;
            }
            SystemVariant::Concurrent(ref mut system, ref mut access) => {
                system.init(resources);
                system.update_access(resources, access);
                self.is_send = system.is_send();
            }
        }
        self.is_initialized = true;
    }

    pub fn is_send(&self) -> bool {
        self.is_send
    }

    pub fn run(&mut self, resources: &mut Resources) {
        assert!(self.is_initialized);
        match self.system_variant {
            SystemVariant::Exclusive(ref mut system) => system.run(resources, ()),
            SystemVariant::Concurrent(ref mut system, _) => system.run(resources, ()),
        }
    }

    pub fn run_send(&mut self, resources: &ResourcesSend) {
        assert!(self.is_initialized && self.is_send);
        match self.system_variant {
            SystemVariant::Concurrent(ref mut system, _) => system.run_send(resources, ()),
            _ => panic!("exclusive systems are not `send`!"),
        }
    }
}

impl std::fmt::Debug for SystemDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("System");
        s.field("type", &self.type_name());
        s.field("exclusive", &self.is_exclusive());
        s.field("send", &self.is_send());
        s.finish()
    }
}

pub(crate) enum SystemVariant {
    Exclusive(Box<dyn ExclusiveSystem>),
    Concurrent(Box<dyn System>, ResourceAccess),
}

unsafe impl<Args, S> System<Args> for Box<S>
where
    S: System<Args> + ?Sized,
{
    fn init(&mut self, resources: &mut Resources) {
        self.as_mut().init(resources)
    }

    #[inline]
    fn run(&mut self, resources: &Resources, args: Args) {
        self.as_mut().run(resources, args)
    }

    #[inline]
    fn run_send(&mut self, resources: &ResourcesSend, args: Args) {
        self.as_mut().run_send(resources, args)
    }

    #[inline]
    fn is_send(&self) -> bool {
        self.as_ref().is_send()
    }

    #[inline]
    fn update_access(&self, resources: &Resources, access: &mut ResourceAccess) {
        self.as_ref().update_access(resources, access)
    }
}

impl<Args, S> ExclusiveSystem<Args> for Box<S>
where
    S: ExclusiveSystem<Args> + ?Sized,
{
    fn init(&mut self, resources: &mut Resources) {
        self.as_mut().init(resources)
    }

    #[inline]
    fn run(&mut self, resources: &mut Resources, args: Args) {
        self.as_mut().run(resources, args)
    }
}

#[doc(hidden)]
pub struct ConcurrentAsExclusiveSystem<S: ?Sized>(S);

impl<Args, S> ExclusiveSystem<Args> for ConcurrentAsExclusiveSystem<S>
where
    S: System<Args> + ?Sized,
{
    fn init(&mut self, resources: &mut Resources) {
        self.0.init(resources)
    }
    #[inline]
    fn run(&mut self, resources: &mut Resources, args: Args) {
        self.0.run(resources, args)
    }
}

impl IntoSystemDescriptor<()> for SystemDescriptor {
    #[inline]
    fn into_system_descriptor(self) -> SystemDescriptor {
        self
    }
}
