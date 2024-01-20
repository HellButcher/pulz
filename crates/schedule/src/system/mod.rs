use crate::resource::{ResourceAccess, Resources, ResourcesSend};

pub mod data;
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

pub struct SystemDescriptor {
    pub(crate) system_variant: SystemVariant,
    // TODO: add a mechanism, that tracks identity of resource-set
    is_initialized: bool,
}

impl SystemDescriptor {
    pub(crate) fn new<S, Marker>(s: S) -> Self
    where
        S: IntoSystem<(), Marker>,
        S::System: 'static,
    {
        let system = s.into_system();
        Self {
            system_variant: SystemVariant::Concurrent(Box::new(system), ResourceAccess::new()),
            is_initialized: false,
        }
    }

    pub(crate) fn new_exclusive<S, Marker>(s: S) -> Self
    where
        S: IntoExclusiveSystem<(), Marker>,
        S::System: 'static,
    {
        let system = s.into_exclusive_system();
        Self {
            system_variant: SystemVariant::Exclusive(Box::new(system)),
            is_initialized: false,
        }
    }

    #[inline]
    pub fn is_concurrent(&self) -> bool {
        matches!(self.system_variant, SystemVariant::Concurrent(_, _))
    }

    #[inline]
    pub fn is_exclusive(&self) -> bool {
        matches!(self.system_variant, SystemVariant::Exclusive(_))
    }

    pub fn is_send(&self) -> bool {
        match &self.system_variant {
            SystemVariant::Concurrent(s, _) => s.is_send(),
            SystemVariant::Exclusive(_s) => false,
        }
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
            }
            SystemVariant::Concurrent(ref mut system, ref mut access) => {
                system.init(resources);
                system.update_access(resources, access);
            }
        }
        self.is_initialized = true;
    }

    pub fn run_exclusive(&mut self, resources: &mut Resources) {
        assert!(self.is_initialized);
        match self.system_variant {
            SystemVariant::Exclusive(ref mut system) => system.run(resources, ()),
            SystemVariant::Concurrent(ref mut system, _) => system.run(resources, ()),
        }
    }

    pub fn run_shared(&mut self, resources: &Resources) {
        assert!(self.is_initialized);
        match self.system_variant {
            SystemVariant::Exclusive(_) => panic!("no exclusive access"),
            SystemVariant::Concurrent(ref mut system, _) => system.run(resources, ()),
        }
    }

    pub fn run_send(&mut self, resources: &ResourcesSend) {
        assert!(self.is_initialized && self.is_send());
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

    #[inline]
    fn type_name(&self) -> &'static str {
        self.as_ref().type_name()
    }
}

impl<Args, S> ExclusiveSystem<Args> for Box<S>
where
    S: ExclusiveSystem<Args> + ?Sized,
{
    #[inline]
    fn init(&mut self, resources: &mut Resources) {
        self.as_mut().init(resources)
    }

    #[inline]
    fn run(&mut self, resources: &mut Resources, args: Args) {
        self.as_mut().run(resources, args)
    }

    fn type_name(&self) -> &'static str {
        self.as_ref().type_name()
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
    #[inline]
    fn into_system_descriptor(self) -> SystemDescriptor {
        SystemDescriptor::new(self)
    }
}

#[doc(hidden)]
pub struct ExclusiveSystemFnMarker;
impl<S, Marker> IntoSystemDescriptor<(ExclusiveSystemFnMarker, Marker)> for S
where
    S: IntoExclusiveSystem<(), Marker>,
    S::System: 'static,
{
    #[inline]
    fn into_system_descriptor(self) -> SystemDescriptor {
        SystemDescriptor::new_exclusive(self)
    }
}

impl IntoSystemDescriptor<()> for SystemDescriptor {
    #[inline]
    fn into_system_descriptor(self) -> SystemDescriptor {
        self
    }
}
