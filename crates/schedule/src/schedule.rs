use std::time::Instant;

use crossbeam_utils::sync::WaitGroup;
use hashbrown::HashMap;
use pulz_bitset::BitSet;

use crate::{
    label::{CoreSystemPhase, SystemPhase, SystemPhaseId},
    resource::{ResourceAccess, Resources},
    system::{ExclusiveSystem, IntoSystemDescriptor, System, SystemDescriptor, SystemVariant},
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

const FIRST_NODE_INDEX: usize = 0;
const LAST_NODE_INDEX: usize = 1;

#[derive(Debug)]
struct DependencyNode {
    index: usize,
    parent: usize,
    dependencies: BitSet,
    sub_nodes: BitSet,
    systems: Vec<usize>,
}

impl DependencyNode {
    #[inline]
    const fn new(index: usize) -> Self {
        Self {
            index,
            parent: !0,
            dependencies: BitSet::new(),
            sub_nodes: BitSet::new(),
            systems: Vec::new(),
        }
    }
}

struct DependencyGraph {
    nodes: Vec<DependencyNode>,
    phase_labels: HashMap<SystemPhaseId, usize>,
}

impl DependencyGraph {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            phase_labels: HashMap::default(),
        }
    }

    pub fn insert_new(&mut self) -> &mut DependencyNode {
        let i = self.nodes.len();
        self.nodes.push(DependencyNode::new(i));
        &mut self.nodes[i]
    }

    pub fn insert_phase(&mut self, label: SystemPhaseId) -> &mut DependencyNode {
        if let Some(&i) = self.phase_labels.get(&label) {
            &mut self.nodes[i]
        } else {
            let i = self.nodes.len();
            self.phase_labels.insert(label, i);
            self.nodes.push(DependencyNode::new(i));
            &mut self.nodes[i]
        }
    }

    pub fn build_topological_groups(&self) -> Result<Vec<Vec<usize>>, Vec<Vec<usize>>> {
        // (lets say, a system is in group `b`, this means that there is at least one
        // dependency for this system in group `b-1`).
        // The order inside the group is the insertion order.
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut completed = BitSet::with_capacity_for(self.nodes.len());
        let mut ready = BitSet::with_capacity_for(self.nodes.len());
        let mut todo = self.nodes.len();
        assert!(todo > 2);
        // always add first group as a seperate group first
        groups.push(vec![FIRST_NODE_INDEX]);
        ready.insert(FIRST_NODE_INDEX);
        completed.insert(FIRST_NODE_INDEX);

        // mark last group[index 2], add it last
        ready.insert(LAST_NODE_INDEX);
        completed.insert(LAST_NODE_INDEX);

        todo -= 2;

        while todo > 0 {
            loop {
                let mut changed = false;
                for node in self.nodes.iter() {
                    // a node becomes READY, its parent is also READY, and if all its dependencies are COMPLETED.
                    if !ready.contains(node.index)
                        && (node.parent == !0 || ready.contains(node.parent))
                        && completed.contains_all(&node.dependencies)
                    {
                        ready.insert(node.index);
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
            let mut new_group = Vec::new();
            loop {
                let mut changed = false;
                for node in self.nodes.iter() {
                    // a node becomes COMPLETED, when it is READY and all its children are COMPLETED
                    if !completed.contains(node.index)
                        && ready.contains(node.index)
                        && completed.contains_all(&node.sub_nodes)
                    {
                        completed.insert(node.index);
                        new_group.push(node.index);
                        todo -= 1;
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }

            if new_group.is_empty() {
                return Err(groups);
            }

            groups.push(new_group);
        }

        // append the last group

        groups.push(vec![LAST_NODE_INDEX]);

        Ok(groups)
    }
}

#[derive(Clone)]
struct ResourceMutTrackerEntry {
    last_exclusive: usize, // index if the group, where exclusive access was requested last
    last_shared: usize,    // index if the group, where shared access was requested last
    systems: Vec<usize>,   // index of the system, that had the last access.
}

impl Default for ResourceMutTrackerEntry {
    #[inline]
    fn default() -> Self {
        Self {
            last_exclusive: !0,
            last_shared: !0,
            systems: Vec::new(),
        }
    }
}
struct ResourceMutTracker(Vec<ResourceMutTrackerEntry>);

#[derive(Clone, Debug)]
enum ResourceConflict {
    #[allow(unused)] // used for Debug
    ExclusiveExclusive {
        resource: usize,
        system_a: usize,
        system_b: usize,
    },
    #[allow(unused)] // used for Debug
    SharedExclusive {
        resource: usize,
        system_shared: Vec<usize>,
        system_exclusive: usize,
    },
}

impl ResourceMutTracker {
    #[inline]
    fn new() -> Self {
        Self(Vec::new())
    }

    fn get_entry_mut(&mut self, resource: usize) -> &mut ResourceMutTrackerEntry {
        if self.0.len() <= resource {
            self.0
                .resize(resource + 1, ResourceMutTrackerEntry::default());
        }
        &mut self.0[resource]
    }

    fn mark_exclusive(
        &mut self,
        resource: usize,
        current_group: usize,
        system: usize,
    ) -> Result<(usize, &[usize]), ResourceConflict> {
        let entry = self.get_entry_mut(resource);
        if entry.last_exclusive == current_group {
            Err(ResourceConflict::ExclusiveExclusive {
                resource,
                system_a: *entry.systems.first().unwrap(),
                system_b: system,
            })
        } else if entry.last_shared == current_group {
            Err(ResourceConflict::SharedExclusive {
                resource,
                system_shared: std::mem::take(&mut entry.systems),
                system_exclusive: system,
            })
        } else {
            let old = entry.last_exclusive;
            entry.last_exclusive = current_group;
            entry.systems.clear();
            entry.systems.push(system);
            Ok((old, &entry.systems))
        }
    }

    fn mark_shared(
        &mut self,
        resource: usize,
        current_group: usize,
        system: usize,
    ) -> Result<(usize, &[usize]), ResourceConflict> {
        let entry = self.get_entry_mut(resource);
        if entry.last_exclusive == current_group {
            Err(ResourceConflict::SharedExclusive {
                resource,
                system_exclusive: *entry.systems.first().unwrap(),
                system_shared: vec![system],
            })
        } else if entry.last_shared == current_group {
            entry.systems.push(system);
            Ok((current_group, &entry.systems))
        } else {
            let old = entry.last_exclusive;
            entry.last_shared = current_group;
            entry.systems.clear();
            entry.systems.push(system);
            Ok((old, &entry.systems))
        }
    }

    fn mark_access<F: FnMut(usize, &[usize])>(
        &mut self,
        access: &ResourceAccess,
        current_group: usize,
        system: usize,
        mut handle: F,
    ) -> Result<(), ResourceConflict> {
        for resource in access.exclusive.iter() {
            let (old_group, systems) = self.mark_exclusive(resource, current_group, system)?;
            if old_group < current_group {
                handle(old_group, systems);
            }
        }
        for resource in access.shared.iter() {
            let (old_group, systems) = self.mark_shared(resource, current_group, system)?;
            if old_group < current_group {
                handle(old_group, systems);
            }
        }
        Ok(())
    }
}

pub struct Schedule {
    systems: Vec<SystemDescriptor>,
    graph: DependencyGraph,
    ordered_task_groups: Vec<TaskGroup>,
    dirty: bool,
}

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
        let i = self.add_system_inner(system.into_system_descriptor());
        SystemEntryBuilder {
            graph: &mut self.graph,
            id: SystemId(i),
            dependency_node: !0,
            phase: CoreSystemPhase::Update.as_label(), // The default phase
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

    fn add_system_inner(&mut self, system: SystemDescriptor) -> usize {
        self.dirty = true;
        let index = self.systems.len();
        self.systems.push(system);
        index
    }

    fn get_system_accesses<'a>(
        &'a self,
        group: &'a [usize],
    ) -> impl Iterator<Item = (usize, &'a ResourceAccess)> + 'a {
        group
            .iter()
            .flat_map(|&n| self.graph.nodes[n].systems.iter().copied())
            .filter_map(|s| self.systems[s].access().map(|a| (s, a)))
    }

    fn mark_system_resource_dependencies_and_check_conflicts(
        &self,
        result: &mut [usize],
        groups: &[Vec<usize>],
    ) {
        let mut resources = ResourceMutTracker::new();
        for (g, group) in groups.iter().enumerate() {
            for (s, access) in self.get_system_accesses(group) {
                let result = resources.mark_access(access, g, s, |_old_group, old_systems| {
                    // resource `r` was used in `old_systems` and is now used in `g`
                    for &s in old_systems {
                        if result[s] > g {
                            result[s] = g;
                        }
                    }
                });
                if let Err(e) = result {
                    let _ = self.debug_dump_if_env_ext(Some(groups), None);
                    panic!("resource conflict ({:?})\nuse PULZ_DUMP_SCHEDULE=[path] to dump a .dot file of the schedule.", e);
                }
            }
        }
    }

    fn mark_system_dependencies_from_graph(&self, result: &mut [usize], groups: &[Vec<usize>]) {
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

    fn get_conflict_groups_for_systems(&self, groups: &[Vec<usize>]) -> Vec<usize> {
        // `groups` define, when a system/node can be schedules FIRST.
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
        groups: &mut Vec<Vec<usize>>,
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
                panic!("unable to build topological order: probbably cycles in systems.\nuse PULZ_DUMP_SCHEDULE=[path] to dump a .dot file of the schedule.");
            }
        };

        // add implicit dependencies, and check conflicts
        let system_conflict_groups = self.get_conflict_groups_for_systems(&groups);

        // map dependency-nodes to systems
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

        // self.debug_dump_if_env_ext(Some(&groups), Some(&system_conflict_groups)).unwrap();

        // build final
        self.ordered_task_groups.clear();
        let mut current_concurrent_group: Vec<(usize, usize)> = Vec::new();
        for s in groups.into_iter().flatten() {
            if self.systems[s].is_exclusive() {
                if !current_concurrent_group.is_empty() {
                    self.ordered_task_groups
                        .push(TaskGroup::Concurrent(std::mem::take(
                            &mut current_concurrent_group,
                        )));
                }
                self.ordered_task_groups.push(TaskGroup::Exclusive(s));
            } else {
                current_concurrent_group.push((s, system_conflict_groups[s]));
            }
        }
        if !current_concurrent_group.is_empty() {
            self.ordered_task_groups
                .push(TaskGroup::Concurrent(current_concurrent_group));
        }
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
            writeln!(f, "/*\n  Backtrace\n  =========\n{:?}\n*/", backtrace)?;
            self.write_dot(&mut f, Some(module_path!()))?;

            writeln!(f, "/*\n  Schedule\n  =========\n{:#?}", self)?;
            if let Some(groups) = groups {
                writeln!(f, "\n  Groups\n  =========\n{:#?}", groups)?;
            }
            if let Some(conflict_groups) = conflict_groups {
                writeln!(f, "  Conflict Groups\n  =========\n{:#?}", conflict_groups)?;
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

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Debug)]
pub struct SystemId(usize);

fn insert_sorted(vec: &mut Vec<usize>, value: usize) {
    if let Err(pos) = vec.binary_search(&value) {
        vec.insert(pos, value);
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

struct TGDebugItem<'s>(&'s SystemDescriptor, usize, usize);
impl std::fmt::Debug for TGDebugItem<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("System");
        s.field("index", &self.1);
        s.field("type", &self.0.type_name());
        s.field("exclusive", &self.0.is_exclusive());
        s.field("send", &self.0.is_send());
        if self.2 != !0 {
            s.field("next", &self.2);
        }
        s.finish()
    }
}

struct TGDebug<'s>(&'s [SystemDescriptor], &'s TaskGroup);
impl std::fmt::Debug for TGDebug<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_list();
        match &self.1 {
            TaskGroup::Exclusive(i) => {
                s.entry(&TGDebugItem(&self.0[*i], *i, !0));
            }
            TaskGroup::Concurrent(group) => {
                for &(i, next) in group {
                    s.entry(&TGDebugItem(&self.0[i], i, next));
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
        s.field("nodes", &self.graph.nodes);
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
        assert!(!self.0.has_exclusive_systems());
        // make resources mut:
        // SAFETY: resources is not accessed through-mut references, because
        // there are no exclusive-systems in the schedule.
        let resources: *const Resources = resources;
        let resources = unsafe { &mut *(resources as *mut _) };
        self.0.run(resources)
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
                scratch.extend(access);
            }
        }
        scratch.shared.remove_bitset(&scratch.exclusive);
        access.extend(&scratch);
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
    /// # Safety
    /// This function may spawn tasks in a thread-pool. These spawned tasks
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
                                system.run_send(resources, ());
                                drop(signal_wait_group);
                            });
                        } else {
                            // execute local
                            current_wait_group.wait();
                            system.run(resources, ());
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
        #[cfg(not(target_os = "unknown"))]
        {
            self.join();
        }
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
