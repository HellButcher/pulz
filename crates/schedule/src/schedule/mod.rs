#[cfg(not(target_os = "unknown"))]
use crossbeam_utils::sync::WaitGroup;

use crate::{
    resource::Resources,
    schedule::graph::DependencyGraph,
    system::{IntoSystemDescriptor, SystemDescriptor},
};

mod debug;
mod execution;
mod graph;
mod resource_tracker;
mod schedule;

#[cfg(not(target_os = "unknown"))]
pub use self::execution::threadpool;
pub use self::schedule::SystemEntryBuilder;

enum TaskGroup {
    // topoligical order of the systems, and the offset (index into this array) where a resource
    // produces/modified by the system is first consumed/read.
    // For example, the array `[(12,2), (13,2), (10,3)]` means, that the system at index `12` and
    // the system at index `13` are a dependency of the system at index `10`. So system `12` and `13`
    // need to be completed before system `10` can start.
    // The `2` refers to the third entry of this array `(10,3)`, so this means system `10`.
    // The `3` refers to the end of this array, so it is the last entry, and is not a dependency
    // of any entry in this group.
    Concurrent(Vec<(usize, usize)>),
    Exclusive(usize),
}

pub struct Schedule {
    systems: Vec<SystemDescriptor>,
    graph: DependencyGraph,
    ordered_task_groups: Vec<TaskGroup>,
    dirty: bool,
}

#[must_use]
pub struct ScheduleExecution<'s> {
    systems: &'s mut [SystemDescriptor],
    ordered_task_groups: &'s [TaskGroup],
    resources: &'s mut Resources,
    #[cfg(not(target_os = "unknown"))]
    // Is one item longer than task_group.len().
    // The task `i` of a task_group will wait on WaitGroup [task_group.len() - current_sub_entry]!
    tasks_rev: Vec<WaitGroup>,
}

#[must_use]
pub struct SharedScheduleExecution<'s> {
    systems: &'s mut [SystemDescriptor],
    concurrent_tasks: &'s [(usize, usize)],
    resources: &'s Resources,
    #[cfg(not(target_os = "unknown"))]
    // Is one item longer than task_group.len().
    // The task `i` of a task_group will wait on WaitGroup [task_group.len() - current_sub_entry]!
    tasks_rev: Vec<WaitGroup>,
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Debug)]
pub struct SystemId(usize);

impl Resources {
    #[inline]
    pub fn run<Marker>(&mut self, sys: impl IntoSystemDescriptor<Marker>) {
        let mut d = sys.into_system_descriptor();
        d.init(self);
        d.run_exclusive(self);
    }
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
        resource::ResourceAccess,
        system::{ExclusiveSystem, System},
    };

    #[test]
    fn test_schedule() {
        struct A;
        struct Sys(Arc<AtomicUsize>);
        let counter = Arc::new(AtomicUsize::new(0));
        unsafe impl System for Sys {
            fn init(&mut self, _resources: &mut Resources) {}
            fn run(&mut self, _arg: &Resources, _arg2: ()) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            }
            fn is_send(&self) -> bool {
                true
            }
            fn update_access(&self, _resources: &Resources, _access: &mut ResourceAccess) {}
        }
        struct ExSys;
        impl ExclusiveSystem for ExSys {
            fn init(&mut self, _resources: &mut Resources) {}
            fn run(&mut self, arg: &mut Resources, _arg2: ()) {
                arg.insert(A);
            }
        }

        let mut resources = Resources::new();
        let mut schedule = Schedule::new();
        schedule.add_system(Sys(counter.clone()));
        schedule.add_system(ExSys);
        schedule.init(&mut resources);

        //dump_schedule_dot!(&schedule);

        assert_eq!(0, counter.load(std::sync::atomic::Ordering::Acquire));
        assert!(resources.get_mut::<A>().is_none());

        schedule.run(&mut resources);

        assert_eq!(1, counter.load(std::sync::atomic::Ordering::Acquire));
        assert!(resources.get_mut::<A>().is_some());
    }
}
