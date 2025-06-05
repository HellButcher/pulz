use crate::{
    label::SystemPhase,
    prelude::Schedule,
    schedule::{
        TaskGroup,
        graph::{DependencyGraph, DependencyNode},
    },
    system::SystemDescriptor,
};

struct TGDebugItem<'s>(&'s SystemDescriptor, usize, usize, usize);
impl std::fmt::Debug for TGDebugItem<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("System");
        s.field("index", &self.1);
        s.field("type", &self.0.type_name());
        s.field("exclusive", &self.0.is_exclusive());
        s.field("send", &self.0.is_send());
        s.field("tg", &self.2);
        if self.3 != !0 {
            s.field("next", &self.3);
        }
        s.finish()
    }
}

struct TGDebug<'s>(&'s [SystemDescriptor], &'s [TaskGroup]);
impl std::fmt::Debug for TGDebug<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_list();
        for (j, tg) in self.1.iter().enumerate() {
            match tg {
                TaskGroup::Exclusive(i) => {
                    s.entry(&TGDebugItem(&self.0[*i], *i, j, !0));
                }
                TaskGroup::Concurrent(group) => {
                    for &(i, next) in group {
                        s.entry(&TGDebugItem(&self.0[i], i, j, next));
                    }
                }
            }
        }
        s.finish()
    }
}

struct DNDebugItem<'s>(&'s DependencyNode, &'s str);
impl std::fmt::Debug for DNDebugItem<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut t = f.debug_list();
        t.entry(&self.1);
        t.entry(&self.0);
        t.finish()
    }
}
impl std::fmt::Debug for DependencyGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_list();
        for n in &self.nodes {
            let name = self
                .phase_labels
                .iter()
                .find_map(|(p, i)| {
                    if *i == n.index {
                        Some(p.as_str())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();
            s.entry(&DNDebugItem(n, name));
        }
        s.finish()
    }
}

impl std::fmt::Debug for Schedule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Schedule");
        s.field("dirty", &self.dirty);
        s.field("nodes", &self.graph);
        if self.dirty {
            s.field("systems", &self.systems);
        } else {
            s.field("order", &TGDebug(&self.systems, &self.ordered_task_groups));
        }
        s.finish()
    }
}
