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

pub trait SystemInit: 'static {
    fn init(&mut self, res: &mut Resources);
    fn system_type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    fn system_type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<Self>()
    }
    fn system_label(&self) -> SystemLabel {
        SystemLabel(self.system_type_id(), self.system_type_name())
    }
}

pub trait ExclusiveSystem: SystemInit {
    fn run_exclusive(&mut self, res: &mut Resources);
}

pub trait System: ExclusiveSystem {
    fn run(&mut self, res: &Resources);
    fn update_access(&self, res: &Resources, access: &mut ResourceAccess);
}

pub trait SendSystem: System + Send + Sync {
    fn run_send(&mut self, res: &ResourcesSend);
}

pub trait IntoSystem<Marker> {
    type System: ExclusiveSystem;

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
