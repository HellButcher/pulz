use std::collections::BTreeMap;

use crate::{
    atom::Atom,
    label::SystemSetId,
    prelude::ResourceId,
    resource::{ResourceAccess, Resources},
    schedule::{
        graph::{Graph, NodeId},
        resource_tracker::ResourceConflict,
    },
    system::BoxedSystem,
    threadpool::ThreadPool,
    util::DirtyVersion,
};

mod debug;
mod dump;
mod graph;
mod resource_tracker;
mod run;
mod schedule_impl;

pub use self::schedule_impl::ScheduleNodeBuilder;

pub struct Schedule {
    systems: Vec<BoxedSystem>,
    access: Vec<ResourceAccess>,
    phase_labels: BTreeMap<SystemSetId, NodeId>,
    graph: Graph,
    ordered_layers: Vec<Vec<SystemId>>,
    system_dependent_layers: Vec<Layer>,
    #[cfg(not(target_os = "unknown"))]
    threadpool_id: Option<ResourceId<ThreadPool>>,
    atom: Atom,
    version: DirtyVersion,
}

#[repr(transparent)]
pub struct SharedSchedule(Schedule);

#[derive(thiserror::Error, Debug)]
pub enum ScheduleError {
    #[error(transparent)]
    GraphError(#[from] graph::GraphError),

    #[error(transparent)]
    ResourceConflict(#[from] ResourceConflict),
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct SystemId(usize);

impl SystemId {
    pub const UNDEFINED: Self = Self(!0);

    #[inline]
    pub const fn is_undefined(&self) -> bool {
        self.0 == !0
    }

    #[inline]
    pub const fn is_defined(&self) -> bool {
        self.0 != !0
    }
}

pub struct RunSharedSheduleSystem<S>(Option<ResourceId<S>>)
where
    S: AsMut<SharedSchedule> + 'static;

impl<S> RunSharedSheduleSystem<S>
where
    S: AsMut<SharedSchedule> + 'static,
{
    #[inline]
    pub const fn new() -> Self {
        Self(None)
    }
}

impl<S> Default for RunSharedSheduleSystem<S>
where
    S: AsMut<SharedSchedule> + 'static,
{
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
struct Layer(usize);

impl Layer {
    pub const UNDEFINED: Self = Self(!0);
}

impl Resources {
    #[inline]
    pub fn run_schedule<S>(&mut self)
    where
        S: AsMut<Schedule> + 'static,
    {
        self.run_schedule_id(self.expect_id::<S>())
    }

    pub fn run_schedule_id<S>(&mut self, id: ResourceId<S>)
    where
        S: AsMut<Schedule> + 'static,
    {
        self.take_id_and::<S, _>(id, |s, res| s.as_mut().run(res));
    }
}

#[macro_export]
macro_rules! custom_schedule_type {
    (
        $(#[$m:meta])*
        $v:vis struct $name:ident
    ) => {
        $(#[$m])*
        #[repr(transparent)]
        #[derive(Default, Debug)]
        $v struct $name($crate::schedule::Schedule);

        impl $name {
            #[inline]
            pub fn new() -> Self {
                Self($crate::schedule::Schedule::new())
            }
        }

        impl ::std::ops::Deref for $name {
            type Target = $crate::schedule::Schedule;

            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl ::std::ops::DerefMut for $name {
            #[inline]
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl ::std::convert::AsRef<$crate::schedule::Schedule> for $name {
            #[inline]
            fn as_ref(&self) -> &$crate::schedule::Schedule {
                &self.0
            }
        }

        impl ::std::convert::AsMut<$crate::schedule::Schedule> for $name {
            #[inline]
            fn as_mut(&mut self) -> &mut $crate::schedule::Schedule {
                &mut self.0
            }
        }

        impl $crate::system::SystemInit for $name {
            #[inline]
            fn init(&mut self, resources: &mut $crate::resource::Resources) {
                self.0.init(resources);
            }
        }

        impl $crate::system::ExclusiveSystem for $name {
            #[inline]
            fn run_exclusive(&mut self, resources: &mut $crate::resource::Resources) {
                self.0.run(resources);
            }
        }

        impl $crate::system::IntoSystem<()> for $name {
            type System = Self;
            #[inline]
            fn into_system(self) -> Self {
                self
            }
        }
    };
}

#[macro_export]
macro_rules! dump_schedule_dot {
    ($schedule:expr) => {
        use std::io::Write;
        let mut filename = module_path!().replace("::", "_");
        filename.push_str(".sched.dot");
        let mut f = std::fs::File::create(&filename).unwrap();
        writeln!(
            f,
            "/*\n  module: {}\n  file: {}:{}\n*/",
            module_path!(),
            file!(),
            line!()
        )
        .unwrap();
        $schedule.write_dot(&mut f, Some(module_path!())).unwrap();
    };
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicUsize};

    use super::*;
    use crate::{
        resource::{ResMut, ResourceAccess, ResourcesSend},
        system::{ExclusiveSystem, SendSystem, System, SystemInit, system},
    };

    #[test]
    fn test_schedule() {
        struct A;
        struct Sys(Arc<AtomicUsize>);
        let counter = Arc::new(AtomicUsize::new(0));
        impl SystemInit for Sys {
            fn init(&mut self, _resources: &mut Resources) {}
        }
        impl System for Sys {
            fn run(&mut self, _resources: &Resources) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            }
            fn update_access(&self, _resources: &Resources, _access: &mut ResourceAccess) {}
        }
        impl SendSystem for Sys {
            fn run_send(&mut self, _resources: &ResourcesSend) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            }
        }

        struct ExSys;
        impl SystemInit for ExSys {
            fn init(&mut self, _resources: &mut Resources) {}
        }
        impl ExclusiveSystem for ExSys {
            fn run_exclusive(&mut self, res: &mut Resources) {
                res.insert(A);
            }
        }

        struct Data(usize);

        #[system]
        #[__crate_path(crate)]
        fn update1(borrowed: &mut Data) {
            borrowed.0 += 7;
        }

        #[system]
        #[__crate_path(crate)]
        fn update2(mut owned: ResMut<'_, Data>) {
            assert_eq!(owned.0, 10);
            owned.0 += 11;
        }

        let mut resources = Resources::new();
        resources.insert(Data(3));
        let mut schedule = Schedule::new();
        schedule.add_system(Sys(counter.clone()));
        schedule.add_system_exclusive(ExSys);
        let l1 = schedule.add_system(System![update1]).as_label();
        schedule.add_system(System![update2]).after(l1);
        schedule.init(&mut resources);

        //dump_schedule_dot!(&schedule);

        assert_eq!(0, counter.load(std::sync::atomic::Ordering::Acquire));
        assert!(resources.get_mut::<A>().is_none());

        schedule.run(&mut resources);

        assert_eq!(1, counter.load(std::sync::atomic::Ordering::Acquire));
        assert!(resources.get_mut::<A>().is_some());
        assert_eq!(resources.get_mut::<Data>().unwrap().0, 21);
    }
}
