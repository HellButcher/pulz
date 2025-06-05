use std::time::Instant;

use crate::{
    label::{CoreSystemPhase, SystemPhase, SystemPhaseId, UndefinedSystemPhase},
    prelude::{Resources, Schedule},
    resource::ResourceAccess,
    schedule::{
        ScheduleExecution, SharedScheduleExecution, SystemId, TaskGroup,
        graph::{DependencyGraph, DependencyNode, FIRST_NODE_INDEX},
        resource_tracker::ResourceMutTracker,
    },
    system::{ExclusiveSystem, IntoSystemDescriptor, System, SystemDescriptor, SystemVariant},
};

impl Schedule {
    pub fn new() -> Self {
        let mut graph = DependencyGraph::new();
        graph.insert_phase(CoreSystemPhase::First.as_label()); // < index=0 (FIRST_NODE_INDEX)
        graph.insert_phase(CoreSystemPhase::Last.as_label()); // < index=1 (LAST_NODE_INDEX)
        graph.insert_phase(CoreSystemPhase::Update.as_label());
        Self {
            systems: Vec::new(),
            graph,
            ordered_task_groups: Vec::new(),
            dirty: true,
        }
    }

    #[inline]
    pub fn add_system<Marker>(
        &mut self,
        system: impl IntoSystemDescriptor<Marker>,
    ) -> SystemEntryBuilder<'_> {
        self._add_system(system.into_system_descriptor())
    }

    fn _add_system(&mut self, system: SystemDescriptor) -> SystemEntryBuilder<'_> {
        self.dirty = true;
        let index = self.systems.len();
        self.systems.push(system);
        SystemEntryBuilder {
            graph: &mut self.graph,
            id: SystemId(index),
            dependency_node: !0,
            phase: UndefinedSystemPhase::Undefined.as_label(),
        }
    }

    #[inline]
    pub fn add_phase_chain(&mut self, phases: impl IntoIterator<Item = impl SystemPhase>) {
        self._add_phase_chain(phases.into_iter().map(|l| l.as_label()));
    }
    fn _add_phase_chain(&mut self, mut phases: impl Iterator<Item = SystemPhaseId>) {
        // get index of first entry of the sequence
        let Some(phase_label) = phases.next() else {
            return;
        };
        self.dirty = true;
        let mut prev = self.graph.insert_phase(phase_label).index;
        // handle rest of chain
        for phase_label in phases {
            let phase = self.graph.insert_phase(phase_label);
            phase.dependencies.insert(prev);
            prev = phase.index;
        }
    }

    #[inline]
    pub fn add_phase_dependency(&mut self, first: impl SystemPhase, second: impl SystemPhase) {
        self.dirty = true;
        let first_index = self.graph.insert_phase(first.as_label()).index;
        self.graph
            .insert_phase(second.as_label())
            .dependencies
            .insert(first_index);
    }

    fn has_exclusive_systems(&self) -> bool {
        self.systems.iter().any(|s| s.is_exclusive())
    }

    fn get_system_accesses<'a>(
        &'a self,
        group: &'a [usize], // group[i] = dependency node
    ) -> impl Iterator<Item = (usize, &'a ResourceAccess)> + 'a {
        group
            .iter()
            .flat_map(|&n| self.graph.nodes[n].systems.iter().copied())
            .filter_map(|s| self.systems[s].access().map(|a| (s, a)))
    }

    fn mark_system_resource_dependencies_and_check_conflicts(
        &self,
        result: &mut [usize], // result[system] = first dependency (by resources)
        groups: &[Vec<usize>], // groups[group][i] = dependency node
    ) {
        let mut resources = ResourceMutTracker::new();
        for (g, group) in groups.iter().enumerate() {
            for (s, access) in self.get_system_accesses(group) {
                if let Err(e) = resources.mark_access(access, g, s, result) {
                    let _ = self.debug_dump_if_env_ext(Some(groups), None);
                    panic!(
                        "resource conflict ({e:?})\nuse PULZ_DUMP_SCHEDULE=[path] to dump a .dot file of the schedule."
                    );
                }
            }
        }
    }

    fn mark_system_dependencies_from_graph(
        &self,
        result: &mut [usize],  // result[system] = first dependency (explicit)
        groups: &[Vec<usize>], // groups[group][i] = dependency node
    ) {
        // special handling for first group
        for &s in &self.graph.nodes[FIRST_NODE_INDEX].systems {
            if result[s] > 1 {
                result[s] = 1;
            }
        }

        for (g, group) in groups.iter().enumerate() {
            for s in group
                .iter()
                .flat_map(|&n| self.graph.nodes[n].dependencies.iter())
                .flat_map(|n| self.graph.nodes[n].systems.iter().copied())
            {
                if result[s] > g {
                    result[s] = g;
                }
            }
        }
    }

    fn get_conflict_groups_for_systems(
        &self,
        groups: &[Vec<usize>], // feoups[group][i] = dependency node
    ) -> Vec<usize> {
        // `groups` define, when a system/node can be scheduled FIRST.
        // This methiod will produce a list (index=system) that tells,
        // when a system can be scheduled LAST (by resource dependencies)
        // (defines the smallest index of the group where it is required next).
        let mut result = Vec::new();
        result.resize(self.systems.len(), !0);
        self.mark_system_resource_dependencies_and_check_conflicts(&mut result, groups);
        self.mark_system_dependencies_from_graph(&mut result, groups);
        result
    }

    fn move_nonsync_and_exclusive(
        &self,
        groups: &mut [Vec<usize>],
        system_conflict_groups: &[usize],
    ) {
        if groups.is_empty() {
            return;
        }
        let mut tmp_nosend = Vec::new();
        let mut tmp_excl = Vec::new();
        let len = groups.len();
        for i in 0..len {
            Self::add_to_prev_group_if_conflict(system_conflict_groups, &mut tmp_nosend, groups, i);
            Self::add_to_prev_group_if_conflict(system_conflict_groups, &mut tmp_excl, groups, i);

            let group = &mut groups[i];
            let mut j = 0;
            while j < group.len() {
                let s = group[j];
                let system = &self.systems[s];
                if system.is_exclusive() {
                    tmp_excl.push(group.swap_remove(j));
                } else if !system.is_send() {
                    tmp_nosend.push(group.swap_remove(j));
                } else {
                    j += 1;
                }
            }
        }
        groups[len - 1].extend(tmp_nosend);
        groups[len - 1].extend(tmp_excl);
    }

    fn add_to_prev_group_if_conflict(
        system_conflict_groups: &[usize],
        src: &mut Vec<usize>,
        groups: &mut [Vec<usize>],
        i: usize,
    ) {
        let mut j = 0;
        while j < src.len() {
            let s = src[j];
            let conflict = system_conflict_groups[s] <= i;
            if conflict {
                src.swap_remove(j);
                groups[i - 1].push(s);
            } else {
                j += 1;
            }
        }
    }

    fn rebuild(&mut self) {
        // group systems based on their dependency graph
        let groups = match self.graph.build_topological_groups() {
            Ok(groups) => groups,
            Err(groups) => {
                let _ = self.debug_dump_if_env_ext(Some(&groups), None);
                panic!(
                    "unable to build topological order: probbably cycles in systems.\nuse PULZ_DUMP_SCHEDULE=[path] to dump a .dot file of the schedule."
                );
            }
        };

        // add implicit dependencies, and check conflicts
        let system_conflict_groups = self.get_conflict_groups_for_systems(&groups);

        // map dependency-nodes to systems
        // groups[group][i] = dependency node => groups[group][j] = system
        let mut groups = groups
            .into_iter()
            .map(|g| {
                g.into_iter()
                    .flat_map(|n| self.graph.nodes[n].systems.iter().copied())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        // move non-sync and exclusive systems to the end as far as possible (first nonsend then exclusive)
        self.move_nonsync_and_exclusive(&mut groups, &system_conflict_groups);

        // build final
        self.ordered_task_groups.clear();
        let mut current_concurrent_group: Vec<(usize, usize)> = Vec::new();
        let mut current_group_start = 0;
        for (i, group) in groups.iter().enumerate() {
            for &s in group {
                if self.systems[s].is_exclusive() {
                    if !current_concurrent_group.is_empty() {
                        current_group_start = i;
                        self.ordered_task_groups
                            .push(TaskGroup::Concurrent(std::mem::take(
                                &mut current_concurrent_group,
                            )));
                    }
                    self.ordered_task_groups.push(TaskGroup::Exclusive(s));
                } else {
                    // translate conflict group index to offset into current group
                    let conflict_group_index = system_conflict_groups[s];
                    let conflict_index = if conflict_group_index == !0 {
                        !0
                    } else {
                        groups[current_group_start..conflict_group_index]
                            .iter()
                            .map(|g| g.len())
                            .sum()
                    };
                    current_concurrent_group.push((s, conflict_index));
                }
            }
        }
        if !current_concurrent_group.is_empty() {
            self.ordered_task_groups
                .push(TaskGroup::Concurrent(current_concurrent_group));
        }

        self.dirty = false;
        //self.debug_dump_if_env_ext(Some(&groups), Some(&system_conflict_groups)).unwrap();
    }

    pub fn init(&mut self, resources: &mut Resources) {
        // TODO: track identity of resource-
        if self.dirty {
            for sys in &mut self.systems {
                sys.init(resources)
            }

            self.rebuild();
        }
    }

    #[inline]
    pub fn run(&mut self, resources: &mut Resources) {
        self.executor(resources).run();
    }

    pub fn executor<'s>(&'s mut self, resources: &'s mut Resources) -> ScheduleExecution<'s> {
        self.init(resources);
        ScheduleExecution {
            systems: &mut self.systems,
            ordered_task_groups: &self.ordered_task_groups,
            resources,
            tasks_rev: Vec::new(),
        }
    }

    pub(crate) fn shared_executor<'s>(
        &'s mut self,
        resources: &'s Resources,
    ) -> SharedScheduleExecution<'s> {
        assert!(!self.has_exclusive_systems());
        let concurrent_tasks = if self.ordered_task_groups.is_empty() {
            &[]
        } else if self.ordered_task_groups.len() == 1 {
            if let TaskGroup::Concurrent(cg) = &self.ordered_task_groups[0] {
                cg.as_slice()
            } else {
                panic!("expected concurrent group");
            }
        } else {
            panic!("unexpected task group count");
        };
        SharedScheduleExecution {
            systems: &mut self.systems,
            concurrent_tasks,
            resources,

            #[cfg(not(target_os = "unknown"))]
            tasks_rev: Vec::new(),
        }
    }

    pub fn debug_dump_if_env(&self) -> std::io::Result<()> {
        self.debug_dump_if_env_ext(None, None)
    }

    fn debug_dump_if_env_ext(
        &self,
        groups: Option<&[Vec<usize>]>,
        conflict_groups: Option<&[usize]>,
    ) -> std::io::Result<()> {
        use std::io::Write;
        if let Some(path) = std::env::var_os("PULZ_DUMP_SCHEDULE") {
            let mut f = std::fs::File::create(path)?;
            let backtrace = backtrace::Backtrace::new();
            writeln!(
                f,
                "// Debug Dump for schedule created on {:?}",
                Instant::now()
            )?;
            writeln!(f, "/*\n  Backtrace\n  =========\n{backtrace:?}\n*/")?;
            self.write_dot(&mut f, Some(module_path!()))?;

            writeln!(f, "/*\n  Schedule\n  =========\n{self:#?}")?;
            if let Some(groups) = groups {
                writeln!(f, "\n  Groups\n  =========\n{groups:#?}")?;
            }
            if let Some(conflict_groups) = conflict_groups {
                writeln!(f, "  Conflict Groups\n  =========\n{conflict_groups:#?}")?;
            }
            writeln!(f, "*/")?;
        }
        Ok(())
    }

    pub fn write_dot(
        &self,
        w: &mut dyn std::io::Write,
        title: Option<&str>,
    ) -> std::io::Result<()> {
        writeln!(w, "digraph system {{")?;
        writeln!(
            w,
            "  graph [ranksep=0.5,overlap=scale,splines=true,compound=true];"
        )?;
        if let Some(title) = title {
            writeln!(w, "  label[shape=underline,label=\"{title}\"]")?;
        }
        writeln!(w, "  start [shape=point];\n")?;

        if self.dirty {
            for (s, system) in self.systems.iter().enumerate() {
                writeln!(w, "  s{s} [shape=box, label=\"{}\"];", system.type_name())?;
            }
        } else {
            for (i, group) in self.ordered_task_groups.iter().enumerate() {
                match group {
                    &TaskGroup::Exclusive(s) => {
                        writeln!(
                            w,
                            "  s{s} [shape=box, label=\"{}\"];",
                            self.systems[s].type_name()
                        )?;
                        if i == 0 {
                            writeln!(w, "  start -> s{s} [style=dashed];")?;
                        }
                    }
                    TaskGroup::Concurrent(entries) => {
                        writeln!(w, "  subgraph cluster_{i} {{")?;
                        for &(s, _) in entries {
                            writeln!(w, "    s{s} [label=\"{}\"];", self.systems[s].type_name())?;
                        }
                        writeln!(w, "    style=dashed;")?;
                        writeln!(w, "  }}")?;

                        let first_in_group = entries.first().unwrap().0;
                        if i == 0 {
                            writeln!(
                                w,
                                "  start -> s{first_in_group} [style=dashed, lhead=cluster_{i}];"
                            )?;
                        } else if let TaskGroup::Exclusive(prev) = self.ordered_task_groups[i - 1] {
                            writeln!(
                                w,
                                "  s{prev} -> s{first_in_group} [style=dashed, lhead=cluster_{i}];"
                            )?;
                        }
                        let next = match self.ordered_task_groups.get(i + 1) {
                            Some(TaskGroup::Exclusive(next)) => *next,
                            Some(TaskGroup::Concurrent(entries)) => entries.first().unwrap().0,
                            None => self.systems.len(),
                        };
                        for &(s, e) in entries {
                            if e >= entries.len() {
                                writeln!(w, "  s{s} -> s{next} [style=dashed];")?;
                            } else {
                                let next = entries[e].0;
                                writeln!(w, "  s{s} -> s{next};")?;
                            }
                        }
                    }
                }
            }
        }

        let end = self.systems.len();
        writeln!(w, "  s{end} [shape=point];")?;
        if self.dirty {
            writeln!(w, "  start -> s{end} [style=dashed];")?;
        } else if let Some(&TaskGroup::Exclusive(prev)) = self.ordered_task_groups.last() {
            writeln!(w, "  s{prev} -> s{end} [style=dashed];")?;
        }

        // legend
        writeln!(w, "  subgraph cluster_legend {{")?;
        writeln!(w, "    x0 [shape=point,xlabel=\"Start\"];")?;
        writeln!(w, "    x1 [shape=box, label=\"Exclusive\"];")?;
        writeln!(w, "    subgraph cluster_legend_sub {{")?;
        writeln!(w, "      x2 [label=\"Concurrent\"];")?;
        writeln!(w, "      x3 [label=\"Send\", color=green];")?;
        writeln!(w, "      style=dashed;")?;
        writeln!(w, "    }}")?;
        writeln!(w, "    x4 [shape=point,xlabel=\"End\"];")?;
        writeln!(w)?;
        writeln!(w, "    x0 -> x1 [style=dashed]")?;
        writeln!(
            w,
            "    x1 -> x2 [color=blue, label=\"is\\nbefore\", constraint=false]"
        )?;
        writeln!(w, "    x2 -> x3 [label=\"critical\\ndep.\"]")?;
        writeln!(
            w,
            "    x3 -> x2 [color=red, label=\"is\\nafter\", constraint=false]"
        )?;
        writeln!(
            w,
            "    x1 -> x2 [style=dashed, label=\"implicit\\ndep.\",lhead=cluster_legend_sub]"
        )?;
        writeln!(w, "    x3 -> x4 [style=dashed]")?;
        writeln!(w, "    label=\"Legend\"")?;
        writeln!(w, "  }}")?;
        // end
        writeln!(w, "}}")?;
        Ok(())
    }
}

impl Default for Schedule {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

pub struct SystemEntryBuilder<'l> {
    graph: &'l mut DependencyGraph,
    id: SystemId,
    dependency_node: usize,
    phase: SystemPhaseId,
}

impl SystemEntryBuilder<'_> {
    #[inline]
    pub fn id(&self) -> SystemId {
        self.id
    }
    fn get_dependency_node(&mut self) -> &mut DependencyNode {
        if self.dependency_node == !0 {
            let node = self.graph.insert_new();
            node.systems.push(self.id.0);
            self.dependency_node = node.index;
        }
        &mut self.graph.nodes[self.dependency_node]
    }
    #[inline]
    pub fn into_phase(&mut self, label: impl SystemPhase) -> &mut Self {
        // will be delayed until drop
        self.phase = label.as_label();
        self
    }
    #[inline]
    pub fn before(&mut self, label: impl SystemPhase) -> &mut Self {
        self._insert_before(label.as_label());
        self
    }
    #[inline]
    pub fn after(&mut self, label: impl SystemPhase) -> &mut Self {
        self._insert_after(label.as_label());
        self
    }
    fn _insert_before(&mut self, label: SystemPhaseId) {
        let system_node_index = self.get_dependency_node().index;
        let phase_node = self.graph.insert_phase(label);
        phase_node.dependencies.insert(system_node_index);
    }
    fn _insert_after(&mut self, label: SystemPhaseId) {
        let phase_node_index = self.graph.insert_phase(label).index;
        let system_node = self.get_dependency_node();
        system_node.dependencies.insert(phase_node_index);
    }
}

impl Drop for SystemEntryBuilder<'_> {
    fn drop(&mut self) {
        if self.dependency_node != !0 {
            let parent = self.graph.insert_phase(self.phase);
            parent.sub_nodes.insert(self.dependency_node);
            let parent_index = parent.index;
            self.graph.nodes[self.dependency_node].parent = parent_index;
        } else {
            let systems = &mut self.graph.insert_phase(self.phase).systems;
            insert_sorted(systems, self.id.0);
        }
    }
}

fn insert_sorted(vec: &mut Vec<usize>, value: usize) {
    if let Err(pos) = vec.binary_search(&value) {
        vec.insert(pos, value);
    }
}

#[repr(transparent)]
struct ExclusiveSystemSchedule(Schedule);

impl ExclusiveSystem for ExclusiveSystemSchedule {
    #[inline]
    fn init(&mut self, resources: &mut Resources) {
        self.0.init(resources)
    }
    #[inline]
    fn run(&mut self, resources: &mut Resources, _args: ()) {
        self.0.run(resources)
    }
}

#[repr(transparent)]
struct ConcurrentSystemSchedule(Schedule);

/// SAFETY: ConcurrentSystemSchedule doesn't contain exclusive systems
unsafe impl Send for ConcurrentSystemSchedule {}
/// SAFETY: ConcurrentSystemSchedule doesn't contain exclusive systems, and Schedule is not modified anymore
unsafe impl Sync for ConcurrentSystemSchedule {}

unsafe impl System for ConcurrentSystemSchedule {
    #[inline]
    fn init(&mut self, resources: &mut Resources) {
        assert!(!self.0.has_exclusive_systems());
        self.0.init(resources)
    }

    #[inline]
    fn run(&mut self, resources: &Resources, _args: ()) {
        self.0.shared_executor(resources).run()
    }

    #[inline]
    fn is_send(&self) -> bool {
        false
    }

    #[inline]
    fn update_access(&self, _resources: &Resources, access: &mut ResourceAccess) {
        // merge system access
        let mut scratch = ResourceAccess::new();
        for system in &self.0.systems {
            if let SystemVariant::Concurrent(_, ref access) = system.system_variant {
                scratch.union_with(access);
            }
        }
        scratch.shared.difference_with(&scratch.exclusive);
        access.union_with(&scratch);
    }
}
#[doc(hidden)]
pub struct ScheduleSystemMarker;
impl IntoSystemDescriptor<ScheduleSystemMarker> for Schedule {
    fn into_system_descriptor(self) -> SystemDescriptor {
        if self.has_exclusive_systems() {
            ExclusiveSystemSchedule(self).into_system_descriptor()
        } else {
            ConcurrentSystemSchedule(self).into_system_descriptor()
        }
    }
}
