use crossbeam_utils::sync::WaitGroup;
use pulz_bitset::BitSet;

use crate::{
    label::SystemLabel,
    resource::Resources,
    system::{ExclusiveSystem, IntoSystemDescriptor, SystemDescriptor, SystemVariant},
};

enum TaskGroup {
    // topoligical order of the systems, and the offset (index into `order`) where the system is
    // required first.
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
    ordered_task_groups: Vec<TaskGroup>,
    dirty: bool,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            ordered_task_groups: Vec::new(),
            dirty: true,
        }
    }

    #[inline]
    pub fn with<Marker>(mut self, system: impl IntoSystemDescriptor<Marker>) -> Self {
        self.add_system(system);
        self
    }

    #[inline]
    pub fn add_system<Marker>(&mut self, system: impl IntoSystemDescriptor<Marker>) -> &mut Self {
        self.add_system_inner(system.into_system_descriptor());
        self
    }

    fn add_system_inner(&mut self, system: SystemDescriptor) {
        self.dirty = true;
        self.systems.push(system)
    }

    fn find_system(&self, label: &SystemLabel) -> Option<usize> {
        self.systems.iter().enumerate().find_map(|(i, s)| {
            s.label
                .as_ref()
                .and_then(|l| if l == label { Some(i) } else { None })
        })
    }

    fn update_dependencies(&mut self) {
        let mut before_dep_pairs = Vec::new();
        for i in 0..self.systems.len() {
            let mut dependencies = BitSet::new();
            for after in &self.systems[i].after {
                if let Some(index) = self.find_system(after) {
                    assert_ne!(index, i);
                    dependencies.insert(index);
                }
            }
            for before in &self.systems[i].before {
                if let Some(index) = self.find_system(before) {
                    assert_ne!(index, i);
                    before_dep_pairs.push((i, index));
                }
            }

            // SAFETY: i < len
            unsafe {
                self.systems.get_unchecked_mut(i).dependencies = dependencies;
            }
        }
        for (before, after) in before_dep_pairs {
            unsafe {
                self.systems
                    .get_unchecked_mut(after)
                    .dependencies
                    .insert(before);
            }
        }
    }

    fn build_topological_groups(&self) -> Vec<Vec<usize>> {
        // (lets say, a system is in group `b`, this means that there is at least one
        // dependency for this system in group `b-1`).
        // The order inside the group is the insertion order.
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut completed = BitSet::with_capacity_for(self.systems.len());
        let mut todo = self.systems.len();
        while todo > 0 {
            let mut new_group = Vec::new();
            for (i, system) in self.systems.iter().enumerate() {
                if !completed.contains(i) && completed.contains_all(&system.dependencies) {
                    new_group.push(i);
                    todo -= 1;
                }
            }
            if new_group.is_empty() {
                panic!("unable to build topological order: probbably cycles in systems");
            }
            for i in &new_group {
                completed.insert(*i);
            }
            groups.push(new_group);
        }
        groups
    }

    fn add_resource_dependencies_and_check_conflicts(&mut self, groups: &[Vec<usize>]) {
        // assigns last exclusive system and group to resources.
        // Its a conflict, if they are accessed in the same group.
        let mut ru = Vec::new();
        for (g, group) in groups.iter().enumerate() {
            // first check & update exclusive access (forward)
            for &s in group {
                let system = &mut self.systems[s];
                if let SystemVariant::Concurrent(_, access) = &system.system_variant {
                    for r in access.exclusive.iter() {
                        let item = get_resource_usage_entry_mut(&mut ru, r);
                        if item.0 == g {
                            panic!("resource conflict for exclusive/exclusive access on resource {} between system {} and {}", r, item.1, s);
                        } else if item.0 < g {
                            // add dependency
                            system.dependencies.insert(item.1);
                        }
                        *item = (g, s);
                    }
                }
            }
            // then check shared access
            for &s in group {
                let system = &mut self.systems[s];
                if let SystemVariant::Concurrent(_, access) = &system.system_variant {
                    for r in access.shared.iter() {
                        let item = get_resource_usage_entry_mut(&mut ru, r);
                        if item.0 == g {
                            panic!("resource conflict for shared/exclusive access on resource {} on between system {} and {}", r, item.1, s);
                        } else if item.0 < g {
                            // add dependency
                            system.dependencies.insert(item.1);
                        }
                    }
                }
            }
        }
        // then update exclusive access (backward)
        ru.clear();
        let mut tmp_deps_pairs = Vec::new();
        for (g, group) in groups.iter().enumerate().rev() {
            for &s in group {
                if let SystemVariant::Concurrent(_, access) = &self.systems[s].system_variant {
                    for r in access.exclusive.iter() {
                        *get_resource_usage_entry_mut(&mut ru, r) = (g, s);
                    }
                    for r in access.shared.iter() {
                        let item = get_resource_usage_entry_mut(&mut ru, r);
                        if item.0 != !0 && item.1 != !0 {
                            tmp_deps_pairs.push((s, item.1));
                        }
                    }
                }
            }
        }
        for (before, after) in tmp_deps_pairs {
            unsafe {
                self.systems
                    .get_unchecked_mut(after)
                    .dependencies
                    .insert(before);
            }
        }
    }

    fn move_nonsync_and_exclusive(&self, groups: &mut Vec<Vec<usize>>) {
        if groups.is_empty() {
            return;
        }
        let mut tmp_nosend = Vec::new();
        let mut tmp_excl = Vec::new();
        let len = groups.len();
        for i in 0..len {
            self.add_to_prev_group_if_conflict(&mut tmp_nosend, groups, i);
            self.add_to_prev_group_if_conflict(&mut tmp_excl, groups, i);

            let group = &mut groups[i];
            let mut j = 0;
            while j < group.len() {
                let s = group[j];
                let system = &self.systems[s];
                if system.is_exclusive() {
                    tmp_excl.push(group.remove(j));
                } else if !system.is_send() {
                    tmp_nosend.push(group.remove(j));
                } else {
                    j += 1;
                }
            }
        }
        groups[len - 1].extend(tmp_nosend);
        groups[len - 1].extend(tmp_excl);
    }

    fn add_to_prev_group_if_conflict(
        &self,
        src: &mut Vec<usize>,
        groups: &mut Vec<Vec<usize>>,
        i: usize,
    ) {
        let mut j = 0;
        while j < src.len() {
            let s = src[j];
            let mut conflict = false;
            for &s2 in &groups[i] {
                if self.systems[s2].dependencies.contains(s) {
                    conflict = true;
                    break;
                }
            }
            if conflict {
                src.remove(j);
                groups[i - 1].push(s);
            } else {
                j += 1;
            }
        }
    }

    fn finalize_concurrent_task_group(&mut self, group: &mut Vec<(usize, usize)>) {
        if group.is_empty() {
            return;
        }
        let len = group.len();
        for i in 0..len {
            let s = group[i].0;
            let mut j = i + 1;
            while j < len {
                let t = group[j].0;
                if self.systems[t].dependencies.contains(s) {
                    break;
                } else {
                    j += 1;
                }
            }
            group[i].1 = j;
        }
        self.ordered_task_groups
            .push(TaskGroup::Concurrent(std::mem::take(group)));
    }

    fn rebuild(&mut self) {
        // first update the dependencies metadata
        self.update_dependencies();

        // now group these systems based on their dependency
        let mut groups = self.build_topological_groups();

        // add implicit dependencies, and check conflicts
        self.add_resource_dependencies_and_check_conflicts(&groups);

        // move non-sync and exclusive systems to the end as far as possible (first nonsend then exclusive)
        self.move_nonsync_and_exclusive(&mut groups);

        // build final
        self.ordered_task_groups.clear();
        let mut current_concurrent_group: Vec<(usize, usize)> = Vec::new();
        for s in groups.into_iter().flatten() {
            if self.systems[s].is_exclusive() {
                self.finalize_concurrent_task_group(&mut current_concurrent_group);
                self.ordered_task_groups.push(TaskGroup::Exclusive(s));
            } else {
                current_concurrent_group.push((s, !0usize));
            }
        }
        self.finalize_concurrent_task_group(&mut current_concurrent_group);
    }

    pub fn init(&mut self, resources: &mut Resources) {
        // TODO: track identity of resource-
        if self.dirty {
            for sys in &mut self.systems {
                sys.init(resources)
            }

            self.rebuild();
            self.dirty = false;
        }
    }

    #[inline]
    pub fn run(&mut self, resources: &mut Resources) {
        self.executor(resources).run()
    }

    pub fn executor<'s>(&'s mut self, resources: &'s mut Resources) -> ScheduleExecution<'_> {
        self.init(resources);
        ScheduleExecution {
            systems: &mut self.systems,
            ordered_task_groups: &self.ordered_task_groups,
            resources,
            current_task_group: 0,
            current_sub_entry: 0,

            #[cfg(not(target_os = "unknown"))]
            tasks_rev: Vec::new(),
        }
    }

    pub fn write_dot(&self, w: &mut dyn std::io::Write, title: Option<&str>) -> std::io::Result<()> {
        write!(w, "digraph system {{\n")?;
        write!(w, "  graph [ranksep=0.5,overlap=scale,splines=true,compound=true];\n")?;
        if let Some(title) = title {
            write!(w, "  label[shape=underline,label=\"{title}\"]")?;
        }
        write!(w, "  start [shape=point];\n")?;

        for (i, group) in self.ordered_task_groups.iter().enumerate() {
            match group {
                &TaskGroup::Exclusive(s) => {
                    if let Some(label) = &self.systems[s].label {
                        write!(w, "  s{s} [shape=box, label=\"{label:?}\"];\n")?;
                    } else {
                        write!(w, "  s{s} [shape=box];\n")?;
                    }
                    if i == 0 {
                        write!(w, "  start -> s{s} [style=dashed];\n")?;
                    }
                },
                TaskGroup::Concurrent(entries) => {
                    write!(w, "  subgraph cluster_{i} {{\n")?;
                    for &(s,_) in entries {
                        if let Some(label) = &self.systems[s].label {
                            write!(w, "    s{s} [label=\"{label:?}\"];\n")?;
                        } else {
                            write!(w, "    s{s};\n")?;
                        }
                    }
                    write!(w, "    style=dashed;\n")?;
                    write!(w, "  }}\n")?;

                    let first_in_group = entries.first().unwrap().0;
                    if i == 0 {
                        write!(w, "  start -> s{first_in_group} [style=dashed, lhead=cluster_{i}];\n")?;
                    } else if let TaskGroup::Exclusive(prev) = self.ordered_task_groups[i-1] {
                        write!(w, "  s{prev} -> s{first_in_group} [style=dashed, lhead=cluster_{i}];\n")?;
                    }
                    let next = match self.ordered_task_groups.get(i+1) {
                        Some(TaskGroup::Exclusive(next)) => *next,
                        Some(TaskGroup::Concurrent(entries)) => entries.first().unwrap().0,
                        None => self.systems.len(),
                    };
                    for &(s,e) in entries {
                        if e >= entries.len() {
                            write!(w, "  s{s} -> s{next} [style=dashed];\n")?;
                        } else {
                            let next = entries[e].0;
                            write!(w, "  s{s} -> s{next};\n")?;
                        }
                    }
                }
            }
        }

        let end = self.systems.len();
        write!(w, "  s{end} [shape=point];\n")?;
        if self.ordered_task_groups.is_empty() {
            write!(w, "  start -> s{end} [style=dashed];\n")?;
        } else if let Some(&TaskGroup::Exclusive(prev)) = self.ordered_task_groups.last() {
            write!(w, "  s{prev} -> s{end} [style=dashed];\n")?;
        }


        for (s, system) in self.systems.iter().enumerate() {
            for label in &system.after {
                if let Some(after) = self.find_system(label) {
                    write!(w, "  s{s} -> s{after} [constraint=false, color=red];\n")?;
                }
            }
            for label in &system.before {
                if let Some(before) = self.find_system(label) {
                    write!(w, "  s{s} -> s{before} [constraint=false, color=blue];\n")?;
                }
            }
        }

        // legend
        write!(w, "  subgraph cluster_legend {{\n")?;
        write!(w, "    x0 [shape=point,xlabel=\"Start\"];\n")?;
        write!(w, "    x1 [shape=box, label=\"Exclusive\"];\n")?;
        write!(w, "    subgraph cluster_legend_sub {{\n")?;
        write!(w, "      x2 [label=\"Concurrent\"];\n")?;
        write!(w, "      x3 [label=\"Send\", color=green];\n")?;
        write!(w, "      style=dashed;\n")?;
        write!(w, "    }}\n")?;
        write!(w, "    x4 [shape=point,xlabel=\"End\"];\n")?;
        write!(w, "\n")?;
        write!(w, "    x0 -> x1 [style=dashed]\n")?;
        write!(w, "    x1 -> x2 [color=blue, label=\"is\\nbefore\", constraint=false]\n")?;
        write!(w, "    x2 -> x3 [label=\"critical\\ndep.\"]\n")?;
        write!(w, "    x3 -> x2 [color=red, label=\"is\\nafter\", constraint=false]\n")?;
        write!(w, "    x1 -> x2 [style=dashed, label=\"implicit\\ndep.\",lhead=cluster_legend_sub]\n")?;
        write!(w, "    x3 -> x4 [style=dashed]\n")?;
        write!(w, "    label=\"Legend\"\n")?;
        write!(w, "  }}\n")?;
        // end
        write!(w, "}}\n")?;
        Ok(())
    }
}

fn get_resource_usage_entry_mut(
    resource_usage: &mut Vec<(usize, usize)>,
    r: usize,
) -> &mut (usize, usize) {
    if resource_usage.len() <= r {
        resource_usage.resize(r + 1, (!0usize, !0usize));
    }
    // SAFETY: we have checked the length, and resized it if smaller
    unsafe { resource_usage.get_unchecked_mut(r) }
}

impl Default for Schedule {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

struct TGItem<'s>(&'s SystemDescriptor, usize, usize);
impl std::fmt::Debug for TGItem<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("System");
        s.field("index", &self.1);
        s.field("label", &self.0.label);
        s.field("exclusive", &self.0.is_exclusive());
        s.field("send", &self.0.is_send());
        s.field("next", &self.2);
        s.finish()
    }
}

struct TGDebug<'s>(&'s [SystemDescriptor], &'s TaskGroup);

impl std::fmt::Debug for TGDebug<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_list();
        match &self.1 {
            TaskGroup::Exclusive(i) => {
                s.entry(&self.0[*i]);
            }
            TaskGroup::Concurrent(group) => {
                for &(i, next) in group {
                    s.entry(&TGItem(&self.0[i], i, next));
                }
            }
        }
        s.finish()
    }
}

impl std::fmt::Debug for Schedule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Schedule");
        s.field("dirty", &self.dirty);
        if self.dirty {
            s.field("systems", &self.systems);
        } else {
            let tmp: Vec<_> = self
                .ordered_task_groups
                .iter()
                .map(|g| TGDebug(&self.systems, g))
                .collect();
            s.field("order", &tmp);
        }
        s.finish()
    }
}

impl ExclusiveSystem for Schedule {
    fn init(&mut self, resources: &mut Resources) {
        self.init(resources)
    }
    #[inline]
    fn run(&mut self, resources: &mut Resources) {
        self.run(resources)
    }
}

impl Resources {
    #[inline]
    pub fn run<Marker>(&mut self, sys: impl IntoSystemDescriptor<Marker>) {
        sys.into_system_descriptor().run(self)
    }
}

#[must_use]
pub struct ScheduleExecution<'s> {
    systems: &'s mut [SystemDescriptor],
    ordered_task_groups: &'s [TaskGroup],
    resources: &'s mut Resources,
    current_task_group: usize,
    current_sub_entry: usize,
    #[cfg(not(target_os = "unknown"))]
    // Is one item longer than task_group.len().
    // The task `i` of a task_group will wait on WaitGroup [task_group.len() - current_sub_entry]!
    tasks_rev: Vec<WaitGroup>,
}

#[cfg(not(target_os = "unknown"))]
pub mod threadpool {
    use std::{cell::RefCell, ops::DerefMut, panic::AssertUnwindSafe, str::FromStr, sync::Mutex};

    pub use ::threadpool::{Builder, ThreadPool};
    use once_cell::sync::OnceCell;
    static GLOBAL: OnceCell<Mutex<ThreadPool>> = OnceCell::new();

    thread_local!(static CURRENT: RefCell<Option<ThreadPool>> = RefCell::new(None));

    pub fn replace_global_pool<F, R>(pool: ThreadPool) -> Option<ThreadPool> {
        let mut tmp = Some(pool);
        let mutex = GLOBAL.get_or_init(|| Mutex::new(tmp.take().unwrap()));
        tmp.map(|tmp| std::mem::replace(mutex.lock().unwrap().deref_mut(), tmp))
    }

    pub fn with_pool<F, R>(pool: ThreadPool, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        CURRENT.with(|current| {
            let old = current.replace(Some(pool));
            let result = std::panic::catch_unwind(AssertUnwindSafe(f));
            current.replace(old);
            match result {
                Err(panic) => std::panic::resume_unwind(panic),
                Ok(value) => value,
            }
        })
    }

    fn get_or_init_global() -> &'static Mutex<ThreadPool> {
        GLOBAL.get_or_init(|| {
            let pool = if let Some(num_threads) = std::env::var("PULZ_SCHEDULER_NUM_THREADS")
                .ok()
                .as_deref()
                .and_then(|s| usize::from_str(s).ok())
            {
                ThreadPool::new(num_threads)
            } else {
                ThreadPool::default()
            };
            Mutex::new(pool)
        })
    }

    pub(crate) fn spawn<F>(task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        CURRENT.with(|current| {
            {
                if let Some(current) = current.borrow().as_ref() {
                    current.execute(task);
                    return;
                }
            }
            let global = { get_or_init_global().lock().unwrap().clone() };
            current.replace(Some(global.clone()));
            global.execute(task);
        });
    }
}

impl<'s> ScheduleExecution<'s> {
    fn check_end_and_reset(&mut self) -> bool {
        if self.current_task_group < self.ordered_task_groups.len() {
            true
        } else {
            // reset position, so a new iteration can begin
            self.current_task_group = 0;
            self.current_sub_entry = 0;
            false
        }
    }

    /// Runs a single iteration of all active systems on the *current thread*.
    pub fn run_local(&mut self) {
        while self.run_next_system_local() {}
    }

    /// Runs the next active system on the *current thread*.
    ///
    /// This method will return `true` as long there are more active systems to run in this iteration.
    /// When the iteration is completed, `false` is returned, and the execution is reset.
    /// When called after it had returned `false`, a new iteration will start and it will
    /// return `true` again, until this iteration is completed (and so on).
    pub fn run_next_system_local(&mut self) -> bool {
        match self.ordered_task_groups.get(self.current_task_group) {
            Some(&TaskGroup::Exclusive(system_index)) => {
                self.systems[system_index].run(self.resources);
                self.current_task_group += 1;
            }
            Some(TaskGroup::Concurrent(entries)) => {
                if let Some(&(system_index, _signal_task)) = entries.get(self.current_sub_entry) {
                    self.systems[system_index].run(self.resources);
                    self.current_sub_entry += 1;
                } else {
                    self.current_task_group += 1;
                    self.current_sub_entry = 0;
                }
            }
            None => (),
        }
        self.check_end_and_reset()
    }

    /// The current target does not support spawning threads.
    /// Therefore this is an alias to `run_local`
    #[cfg(target_os = "unknown")]
    #[inline]
    pub fn run(&mut self) {
        self.run_local()
    }

    /// The current target does not support spawning threads.
    /// Therefore this is an alias to `run_next_system_local`
    ///
    /// SAFETY: because this is just an alias to `run_next_system_local`,
    /// this is actually safe!
    #[cfg(target_os = "unknown")]
    #[inline]
    pub unsafe fn run_next_system_unchecked(&mut self) -> bool {
        self.run_next_system_local()
    }

    /// Runs a single iteration of all active systems on the *current thread*.
    #[cfg(not(target_os = "unknown"))]
    #[inline]
    pub fn run(&mut self) {
        use std::panic::AssertUnwindSafe;

        let result = std::panic::catch_unwind(AssertUnwindSafe(|| unsafe {
            while self.run_next_system_unchecked() {}
        }));
        self.join();
        if let Err(err) = result {
            std::panic::resume_unwind(err);
        }
    }

    /// Runs the next active system. The system will be spawned onto a thread-pool,
    /// when supported by the system.
    ///
    /// This method will return `true` as long there are more active systems to run in this iteration.
    /// When the iteration is completed, `false` is returned, and the execution is reset.
    /// When called after it had returned `false`, a new iteration will start and it will
    /// return `true` again, until this iteration is completed (and so on).
    ///
    /// SAFETY: This function may spawn tasks in a thread-pool. These spawned tasks
    /// MUST NOT outlive lifetime `'s` on Self.
    /// The `Drop`-impl and 'join'-methis ensures, that all spawned tasks are completed before
    /// `self` is dropped by blocked-waiting on these tasks.
    /// But this can be bypassed with `std::mem::forget`. So you MUST NOT call
    /// `forget` or call `join` to enshure, all tasks are completed.
    #[cfg(not(target_os = "unknown"))]
    pub unsafe fn run_next_system_unchecked(&mut self) -> bool {
        match self.ordered_task_groups.get(self.current_task_group) {
            Some(&TaskGroup::Exclusive(system_index)) => {
                self.systems[system_index].run(self.resources);
                self.current_task_group += 1;
            }
            Some(TaskGroup::Concurrent(entries)) => {
                if let Some(&(system_index, _signal_task)) = entries.get(self.current_sub_entry) {
                    self.tasks_rev
                        .resize_with(entries.len() + 1 - self.current_sub_entry, Default::default);
                    let current_wait_group = self.tasks_rev.pop().unwrap();
                    let signal_wait_group =
                        self.tasks_rev[entries.len() - self.current_sub_entry - 1].clone();

                    // UNSAFE: cast these lifetimes to a 'static scope for usage in
                    // spawned tasks. The requirement is, that these tasks do not
                    // outlive lifetime `'s` on `Self`. This is ensured by the `Drop`-impl,
                    // but this can be bypassed with `std::mem::forget`.
                    let system: *mut _ = &mut self.systems[system_index].system_variant;
                    let system = &mut *system;
                    let resources: *const _ = self.resources;
                    let resources = &*resources;

                    if let SystemVariant::Concurrent(system, _) = system {
                        if system.is_send() {
                            let resources = resources.as_send(); // shared borrow
                            self::threadpool::spawn(move || {
                                current_wait_group.wait();
                                system.run_send(resources);
                                drop(signal_wait_group);
                            });
                        } else {
                            // execute local
                            current_wait_group.wait();
                            system.run(resources);
                            drop(signal_wait_group);
                        }
                    } else {
                        panic!("expected a concurrent system!");
                    }
                    self.current_sub_entry += 1;
                } else {
                    self.current_task_group += 1;
                    self.current_sub_entry = 0;
                    self.join();
                }
            }
            None => (),
        }

        self.check_end_and_reset()
    }

    #[cfg(not(target_os = "unknown"))]
    pub fn join(&mut self) {
        // wait for all outstanding tasks
        while let Some(wait_group) = self.tasks_rev.pop() {
            wait_group.wait();
        }
    }
}

impl Drop for ScheduleExecution<'_> {
    fn drop(&mut self) {
        self.join();
    }
}

#[macro_export]
macro_rules! dump_schedule_dot {
    ($schedule:expr) => {
        use std::io::Write;
        let mut filename = module_path!().replace("::", "_");
        filename.push_str(".sched.dot");
        let mut f = std::fs::File::create(&filename).unwrap();
        write!(f, "# mod: {}\n# file: {}:{}\n", module_path!(), file!(), line!()).unwrap();
        $schedule.write_dot(&mut f, Some(module_path!())).unwrap();
    };
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::system::{ExclusiveSystem, System};

    #[test]
    fn test_schedule() {
        struct A;
        struct Sys(Arc<std::sync::atomic::AtomicUsize>);
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        unsafe impl System for Sys {
            fn init(&mut self, _resources: &mut Resources) {}
            fn run(&mut self, _arg: &Resources) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            }
            fn is_send(&self) -> bool {
                true
            }
            fn update_access(
                &self,
                _resources: &Resources,
                _access: &mut crate::resource::ResourceAccess,
            ) {
            }
        }
        struct ExSys;
        impl ExclusiveSystem for ExSys {
            fn init(&mut self, _resources: &mut Resources) {}
            fn run(&mut self, arg: &mut Resources) {
                arg.insert(A);
            }
        }

        let mut resources = Resources::new();
        let mut schedule = Schedule::new().with(Sys(counter.clone())).with(ExSys);
        schedule.init(&mut resources);

        //dump_schedule_dot!(&schedule);

        assert_eq!(0, counter.load(std::sync::atomic::Ordering::Acquire));
        assert!(resources.get_mut::<A>().is_none());

        schedule.run(&mut resources);

        assert_eq!(1, counter.load(std::sync::atomic::Ordering::Acquire));
        assert!(resources.get_mut::<A>().is_some());
    }
}
