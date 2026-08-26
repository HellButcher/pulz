//! Parallel system schedule: topological DAG ordering and resource-conflict detection.

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

/// A DAG of systems that executes them in dependency order, running independent systems in parallel.
///
/// Build a schedule by calling `add_system` / `add_system_exclusive`, then call `init` once
/// before the first `run`. Use [`custom_schedule_type!`] to define newtype wrappers.
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

/// A `Schedule` newtype that implements `AsMut<SharedSchedule>` for use as a resource.
#[repr(transparent)]
pub struct SharedSchedule(Schedule);

/// Errors that can occur when building or running a schedule.
#[derive(thiserror::Error, Debug)]
pub enum ScheduleError {
    #[error(transparent)]
    GraphError(#[from] graph::GraphError),

    #[error(transparent)]
    ResourceConflict(#[from] ResourceConflict),
}

/// An index identifying a system within a [`Schedule`].
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct SystemId(usize);

impl SystemId {
    /// Sentinel value representing an unassigned system id.
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
            /// Creates a new empty schedule.
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
    use std::{
        assert_matches,
        sync::{Arc, atomic::AtomicUsize},
    };

    use super::*;
    use crate::{
        label::{CoreSystemSet, LabelId},
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
        fn update1(borrowed: &mut Data) {
            borrowed.0 += 7;
        }

        #[system]
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

    // --- Schedule construction tests ---

    #[test]
    fn schedule_new_is_empty() {
        let s = Schedule::new();
        assert!(s.systems.is_empty());
    }

    #[test]
    fn schedule_add_system_creates_node() {
        struct A;
        struct Sys(Arc<AtomicUsize>);
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

        let mut schedule = Schedule::new();
        let builder = schedule.add_system(Sys(Arc::new(AtomicUsize::new(0))));
        let _label = builder.as_label();
        // Label should be a valid SystemSetId
    }

    #[test]
    fn schedule_add_system_unsend() {
        struct SysUnsend;
        impl SystemInit for SysUnsend {
            fn init(&mut self, _resources: &mut Resources) {}
        }
        impl System for SysUnsend {
            fn run(&mut self, _resources: &Resources) {}
            fn update_access(&self, _resources: &Resources, _access: &mut ResourceAccess) {}
        }

        let mut schedule = Schedule::new();
        let node = schedule.add_system_unsend(SysUnsend);
        assert_matches!(node.get_boxed_system_for_test(), BoxedSystem::Unsend(_));
    }

    #[test]
    fn schedule_add_system_exclusive() {
        struct SysExcl;
        impl SystemInit for SysExcl {
            fn init(&mut self, _resources: &mut Resources) {}
        }
        impl ExclusiveSystem for SysExcl {
            fn run_exclusive(&mut self, _res: &mut Resources) {}
        }

        let mut schedule = Schedule::new();
        let node = schedule.add_system_exclusive(SysExcl);
        assert_matches!(node.get_boxed_system_for_test(), BoxedSystem::Exclusive(_));
    }

    // --- Dependency tests ---

    #[test]
    fn schedule_after_dependency() {
        struct Data(usize);
        #[system]
        fn sys_a(borrowed: &mut Data) {
            borrowed.0 += 3;
        }
        #[system]
        fn sys_b(borrowed: &mut Data) {
            borrowed.0 *= 2;
        }

        let mut resources = Resources::new();
        resources.insert(Data(0));
        let mut schedule = Schedule::new();
        let a_label = schedule.add_system(System![sys_a]).as_label();
        schedule.add_system(System![sys_b]).after(a_label);
        schedule.init(&mut resources);
        schedule.run_local(&mut resources);
        assert_eq!(resources.get_mut::<Data>().unwrap().0, 6);
    }

    #[test]
    fn schedule_before_dependency() {
        struct Data(usize);
        #[system]
        fn sys_a(borrowed: &mut Data) {
            borrowed.0 += 3;
        }
        #[system]
        fn sys_b(borrowed: &mut Data) {
            borrowed.0 *= 2;
        }

        let mut resources = Resources::new();
        resources.insert(Data(0));
        let mut schedule = Schedule::new();
        let b_label = schedule.add_system(System![sys_b]).as_label();
        schedule.add_system(System![sys_a]).before(b_label);
        schedule.init(&mut resources);
        schedule.run_local(&mut resources);
        assert_eq!(resources.get_mut::<Data>().unwrap().0, 6);
    }

    #[test]
    fn schedule_multiple_runs() {
        struct Data(usize);
        #[system]
        fn increment(borrowed: &mut Data) {
            borrowed.0 += 1;
        }

        let mut resources = Resources::new();
        resources.insert(Data(0));
        let mut schedule = Schedule::new();
        schedule.add_system(System![increment]);
        schedule.init(&mut resources);

        for _ in 0..5 {
            schedule.run_local(&mut resources);
        }
        assert_eq!(resources.get_mut::<Data>().unwrap().0, 5);
    }

    // --- SharedSchedule tests ---

    #[test]
    fn shared_schedule_new() {
        let s = SharedSchedule::new();
        assert!(s.systems.is_empty());
    }
}
