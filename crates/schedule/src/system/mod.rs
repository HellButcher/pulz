//! System traits and the `#[system]` / `#[derive(System)]` proc-macro entry points.

use std::marker::PhantomData;

use crate::{
    label::SystemLabel,
    resource::{ResourceAccess, Resources, ResourcesSend},
};

mod boxed;
mod data;
mod func_system;

pub use data::{SystemData, SystemDataSend};
pub use func_system::FuncSystem;
pub use pulz_schedule_macros::{into_system as System, system};

pub(crate) use self::boxed::BoxedSystem;

/// Initialization interface shared by all system variants.
///
/// Called once when the system is first added to a schedule.
pub trait SystemInit: 'static {
    /// Initialises the system's resources, e.g. registering components or resource ids.
    fn init(&mut self, res: &mut Resources);
    fn system_type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    fn system_type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<Self>()
    }
    fn system_label(&self) -> SystemLabel {
        SystemLabel(self.system_type_name())
    }
}

/// A system that requires exclusive access to [`Resources`].
pub trait ExclusiveSystem: SystemInit {
    /// Runs the system with a mutable reference to all resources.
    fn run_exclusive(&mut self, res: &mut Resources);
}

/// A system that only needs shared access to [`Resources`] and declares its resource access upfront.
pub trait System: ExclusiveSystem {
    /// Runs the system with a shared reference to resources.
    fn run(&mut self, res: &Resources);
    /// Declares which resources this system reads and writes.
    fn update_access(&self, res: &Resources, access: &mut ResourceAccess);
}

/// A [`System`] that is also `Send + Sync` and can run on a thread pool.
pub trait SendSystem: System + Send + Sync {
    /// Runs the system on a worker thread with send-safe resource access.
    fn run_send(&mut self, res: &ResourcesSend);
}

/// Conversion trait that turns a value (or function) into a concrete [`ExclusiveSystem`].
pub trait IntoSystem<Marker> {
    /// The concrete system type produced.
    type System: ExclusiveSystem;

    /// Converts this value into a system.
    fn into_system(self) -> Self::System;
}

#[doc(hidden)]
pub struct SelfSystemMarker<S>(PhantomData<fn(S)>);

#[diagnostic::do_not_recommend]
impl<S: ExclusiveSystem> IntoSystem<SelfSystemMarker<S>> for S {
    type System = S;

    #[inline]
    fn into_system(self) -> Self::System {
        self
    }
}

#[doc(hidden)]
pub struct IntoSystemFnMarker;

#[diagnostic::do_not_recommend]
impl<S, F> IntoSystem<IntoSystemFnMarker> for F
where
    F: FnOnce() -> S,
    S: ExclusiveSystem,
{
    type System = S;

    #[inline]
    fn into_system(self) -> Self::System {
        self()
    }
}

#[diagnostic::do_not_recommend]
impl<S: System> ExclusiveSystem for S {
    #[inline]
    fn run_exclusive<'a>(&'a mut self, res: &'a mut Resources) {
        self.run(res)
    }
}
