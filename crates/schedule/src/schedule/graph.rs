use fnv::FnvHashMap as HashMap;
use pulz_bitset::BitSet;

use crate::label::SystemPhaseId;

pub const FIRST_NODE_INDEX: usize = 0;
pub const LAST_NODE_INDEX: usize = 1;

#[derive(Debug)]
pub struct DependencyNode {
    pub index: usize,
    pub parent: usize,
    pub dependencies: BitSet,
    pub sub_nodes: BitSet,
    pub systems: Vec<usize>,
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

pub struct DependencyGraph {
    pub nodes: Vec<DependencyNode>,
    pub phase_labels: HashMap<SystemPhaseId, usize>,
}

impl DependencyGraph {
    pub fn new() -> Self {
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
                    // a node becomes READY, if its parent is also READY, and if all its dependencies are COMPLETED.
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
