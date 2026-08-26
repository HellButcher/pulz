use std::{
    cell::OnceCell,
    ops::{Index, IndexMut},
};

use bit_set::BitSet;

use crate::schedule::{Layer, SystemId};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct NodeId(usize);

impl NodeId {
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

impl std::fmt::Debug for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_undefined() {
            write!(f, "NodeId(-)")
        } else {
            write!(f, "NodeId({})", self.0)
        }
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    dependencies: Vec<NodeId>,
    pub parent: NodeId,
    pub system: SystemId,
}

#[derive(Clone)]
pub struct Graph {
    nodes: Vec<Node>,
}

pub struct WorkGraph<'a> {
    pub graph: &'a Graph,
    children: Vec<Vec<NodeId>>,
    topo_order: OnceCell<Vec<Vec<NodeId>>>,
}

#[derive(thiserror::Error, Debug)]
pub enum GraphError {
    #[error("Cycle detected in the graph with {0} nodes remaining.")]
    CycleDetected(usize),
}

impl Node {
    #[inline]
    const fn new(system: SystemId) -> Self {
        Self {
            dependencies: Vec::new(),
            parent: NodeId::UNDEFINED,
            system,
        }
    }

    #[inline]
    fn is_parent_ready(&self, ready_set: &BitSet) -> bool {
        self.parent.is_undefined() || ready_set.contains(self.parent.0)
    }

    #[inline]
    fn are_dependencies_complete(&self, completed_set: &BitSet) -> bool {
        self.dependencies
            .iter()
            .all(|dep| completed_set.contains(dep.0))
    }

    #[inline]
    pub fn add_dependency(&mut self, dependency: NodeId) {
        insert_sorted(&mut self.dependencies, dependency);
    }
}

impl std::ops::Deref for WorkGraph<'_> {
    type Target = Graph;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.graph
    }
}

impl Graph {
    #[inline]
    pub const fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[inline]
    pub fn get(&self, node_id: NodeId) -> Option<&Node> {
        self.nodes.get(node_id.0)
    }

    #[inline]
    pub fn get_mut(&mut self, node_id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(node_id.0)
    }

    #[inline]
    pub fn insert(&mut self, system_id: SystemId) -> (NodeId, &mut Node) {
        let index = self.nodes.len();
        self.nodes.push(Node::new(system_id));
        (NodeId(index), &mut self.nodes[index])
    }

    #[inline]
    pub fn add_dependency(&mut self, dependency: NodeId, dependent: NodeId) {
        self[dependent].add_dependency(dependency);
    }

    #[inline]
    pub fn set_parent(&mut self, parent: NodeId, child: NodeId) {
        self[child].parent = parent;
    }

    #[inline]
    fn iter(&self) -> impl Iterator<Item = (NodeId, &Node)> {
        self.nodes.iter().enumerate().map(|(i, n)| (NodeId(i), n))
    }

    #[inline]
    pub fn work_graph(&self) -> WorkGraph<'_> {
        WorkGraph::new(self)
    }

    pub fn systems_len(&self) -> usize {
        self.nodes
            .iter()
            .filter_map(|n| {
                if n.system.is_defined() {
                    Some(n.system.0)
                } else {
                    None
                }
            })
            .max()
            .map_or(0, |max| max + 1)
    }
}

impl Index<NodeId> for Graph {
    type Output = Node;

    #[inline]
    fn index(&self, node_id: NodeId) -> &Self::Output {
        self.get(node_id).expect("NodeId out of bounds")
    }
}

impl IndexMut<NodeId> for Graph {
    #[inline]
    fn index_mut(&mut self, node_id: NodeId) -> &mut Self::Output {
        self.get_mut(node_id).expect("NodeId out of bounds")
    }
}

impl<'a> WorkGraph<'a> {
    pub fn new(graph: &'a Graph) -> Self {
        let mut children = Vec::new();
        children.resize(graph.len(), Vec::new());
        for (node_id, node) in graph.iter() {
            if node.parent.is_defined() {
                children[node.parent.0].push(node_id);
            }
        }
        Self {
            graph,
            children,
            topo_order: OnceCell::new(),
        }
    }

    #[inline]
    pub fn get_children(&self, node_id: NodeId) -> &[NodeId] {
        self.children
            .get(node_id.0)
            .map(|c| c.as_slice())
            .unwrap_or(&[])
    }

    #[inline]
    fn are_children_complete(&self, index: NodeId, completed_set: &BitSet) -> bool {
        self.get_children(index)
            .iter()
            .all(|dep| completed_set.contains(dep.0))
    }

    pub fn topological_order_layers(&self) -> Result<&[Vec<NodeId>], GraphError> {
        if let Some(layers) = self.topo_order.get() {
            return Ok(layers);
        }
        let value = self.build_topological_order_layers()?;
        Ok(self.topo_order.get_or_init(move || value))
    }

    // Aa algorithm for topological sorting that includes parent/child relations (similar to Kahn's algorithm).
    // This algorithm groups nodes into layers, where each layer contains
    // nodes that can be processed independently of each other.
    // The algorithm ensures that nodes are processed in a way that respects their dependencies and parent-child relationships.
    fn build_topological_order_layers(&self) -> Result<Vec<Vec<NodeId>>, GraphError> {
        let mut result_layers = Vec::new();
        let len: usize = self.len();
        if len == 0 {
            return Ok(result_layers);
        }

        // set of nodes that are completed (including their children)
        let mut completed = BitSet::with_capacity(len);
        // set of nodes that are have all dependencies met
        let mut ready = BitSet::with_capacity(len);

        let mut todo = len;
        loop {
            let mut count_new_ready = 0;
            let mut current_layer = Vec::new();
            loop {
                let mut changed = false;
                for (node_id, node) in self.iter() {
                    if completed.contains(node_id.0) {
                        continue;
                    }
                    // a node becomes READY, if its parent is also READY, and if all its dependencies are COMPLETED.
                    let mut is_ready = ready.contains(node_id.0);
                    if !is_ready
                        && node.is_parent_ready(&ready)
                        && node.are_dependencies_complete(&completed)
                    {
                        ready.insert(node_id.0);
                        count_new_ready += 1;
                        is_ready = true;
                        changed = true;
                        if node.system.is_defined() {
                            current_layer.push(node_id);
                        }
                    }
                    // virtual nodes also become COMPLETED here when they are READY
                    // and all their children are COMPLETED.
                    // For viretual nodes this is done in this loop, so they are not introducing a new layer
                    if is_ready
                        && node.system.is_undefined()
                        && self.are_children_complete(node_id, &completed)
                    {
                        completed.insert(node_id.0);
                        ready.remove(node_id.0);
                        changed = true;
                    }
                }
                if !changed {
                    break; // No more nodes can be marked as READY
                }
            }

            if count_new_ready == 0 {
                return Err(GraphError::CycleDetected(todo)); // Cycle detected or no more nodes to process
            }
            if !current_layer.is_empty() {
                result_layers.push(current_layer);
            }
            todo -= count_new_ready;
            if todo == 0 {
                break; // break the outer loop: All nodes processed
            }

            loop {
                let mut changed = false;
                for i in ready.iter() {
                    // a node becomes COMPLETED, when it is READY and all its children are COMPLETED
                    // (when it has no children, it is completed as soon as it is READY).
                    if !completed.contains(i) && self.are_children_complete(NodeId(i), &completed) {
                        completed.insert(i);
                        changed = true;
                    }
                }
                if !changed {
                    break; // No more nodes can be marked as completed
                }
            }
            ready.difference_with(&completed);
        }

        Ok(result_layers)
    }

    pub fn systems_topological_order_layers(&self) -> Result<Vec<Vec<SystemId>>, GraphError> {
        let layers = self.topological_order_layers()?;
        Ok(self.build_systems_topological_order_layers(layers))
    }

    fn build_systems_topological_order_layers(
        &self,
        topological_order_layers: &[Vec<NodeId>],
    ) -> Vec<Vec<SystemId>> {
        topological_order_layers
            .iter()
            .map(|layer| {
                layer
                    .iter()
                    .filter_map(|&node_id| {
                        let node = &self.graph[node_id];
                        if node.system.is_defined() {
                            Some(node.system)
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .collect()
    }

    pub fn systems_dependent_layers(&self) -> Result<Vec<Layer>, GraphError> {
        let layers = self.topological_order_layers()?;
        Ok(self.build_systems_dependent_layers(layers))
    }

    fn build_systems_dependent_layers(
        &self,
        topological_order_layers: &[Vec<NodeId>],
    ) -> Vec<Layer> {
        let mut result = Vec::new();
        result.resize(self.systems_len(), Layer::UNDEFINED);

        // Build a map from NodeId to layer index
        let mut node_to_layer: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        for (layer_index, layer) in topological_order_layers.iter().enumerate() {
            for &node_id in layer {
                node_to_layer.insert(node_id.0, layer_index);
            }
        }

        // For each system, find the maximum layer index of its dependencies
        for layer in topological_order_layers.iter() {
            for &node_id in layer {
                let node = &self[node_id];
                if !node.system.is_defined() {
                    continue;
                }
                for &dep in node.dependencies.iter() {
                    let dep_node = &self.graph[dep];
                    if dep_node.system.is_defined()
                        && let Some(&dep_layer) = node_to_layer.get(&dep.0)
                    {
                        let entry = &mut result[node.system.0];
                        // Update if entry is UNDEFINED or dep_layer is greater
                        let should_update = entry.0 == !0 || dep_layer > entry.0;
                        if should_update {
                            entry.0 = dep_layer;
                        }
                    }
                }
            }
        }
        result
    }

    pub fn systems_topological_order_with_dependent_layers(
        &self,
    ) -> Result<(Vec<Vec<SystemId>>, Vec<Layer>), GraphError> {
        Ok((
            self.systems_topological_order_layers()?,
            self.systems_dependent_layers()?,
        ))
    }
}

fn insert_sorted<T: Ord>(vec: &mut Vec<T>, value: T) {
    if let Err(pos) = vec.binary_search(&value) {
        vec.insert(pos, value);
    }
}

impl GraphError {
    pub fn panic_with_optional_dump(&self, context: &str) -> ! {
        //let _ = self.debug_dump_if_env_ext(Some(&groups), None);
        panic!(
            "{context}: probbably cycles in systems.\nuse PULZ_DUMP_SCHEDULE=[path] to dump a .dot file of the schedule."
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Graph basics ---

    #[test]
    fn graph_new_is_empty() {
        let g = Graph::new();
        assert_eq!(g.len(), 0);
    }

    #[test]
    fn graph_insert_creates_node() {
        let mut g = Graph::new();
        let (id, node) = g.insert(SystemId(0));
        assert!(id.is_defined());
        assert_eq!(node.system, SystemId(0));
        assert_eq!(g.len(), 1);
    }

    #[test]
    fn graph_get_returns_node() {
        let mut g = Graph::new();
        let (id, _) = g.insert(SystemId(42));
        assert_eq!(g.get(id).unwrap().system, SystemId(42));
        assert!(g.get(NodeId(99)).is_none());
    }

    #[test]
    fn graph_add_dependency() {
        let mut g = Graph::new();
        let (a, _) = g.insert(SystemId(0));
        let (b, _) = g.insert(SystemId(1));
        g.add_dependency(a, b);
        assert_eq!(g[b].dependencies, vec![a]);
    }

    #[test]
    fn graph_set_parent() {
        let mut g = Graph::new();
        let (p, _) = g.insert(SystemId(0));
        let (c, _) = g.insert(SystemId(1));
        g.set_parent(p, c);
        assert_eq!(g[c].parent, p);
    }

    #[test]
    fn graph_index_operator() {
        let mut g = Graph::new();
        let (id, _) = g.insert(SystemId(0));
        // Should not panic
        let _node = &g[id];
    }

    // --- WorkGraph: linear DAG ---

    #[test]
    fn workgraph_linear_dag_single_layer() {
        let mut g = Graph::new();
        let (a, _) = g.insert(SystemId(0));
        let (b, _) = g.insert(SystemId(1));
        let (c, _) = g.insert(SystemId(2));
        g.add_dependency(a, b);
        g.add_dependency(b, c);
        let wg = g.work_graph();
        let layers = wg.topological_order_layers().unwrap();
        // Linear chain: each node in its own layer
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[0], vec![a]);
        assert_eq!(layers[1], vec![b]);
        assert_eq!(layers[2], vec![c]);
    }

    #[test]
    fn workgraph_linear_dag_systems_order() {
        let mut g = Graph::new();
        let (a, _) = g.insert(SystemId(0));
        let (b, _) = g.insert(SystemId(1));
        g.add_dependency(a, b);
        let wg = g.work_graph();
        let systems = wg.systems_topological_order_layers().unwrap();
        assert_eq!(systems.len(), 2);
        assert_eq!(systems[0], vec![SystemId(0)]);
        assert_eq!(systems[1], vec![SystemId(1)]);
    }

    // --- WorkGraph: diamond dependencies ---

    #[test]
    fn workgraph_diamond_dependencies() {
        //   A
        //  / \
        // B   C
        //  \ /
        //   D
        let mut g = Graph::new();
        let (a, _) = g.insert(SystemId(0));
        let (b, _) = g.insert(SystemId(1));
        let (c, _) = g.insert(SystemId(2));
        let (d, _) = g.insert(SystemId(3));
        g.add_dependency(a, b);
        g.add_dependency(a, c);
        g.add_dependency(b, d);
        g.add_dependency(c, d);
        let wg = g.work_graph();
        let layers = wg.topological_order_layers().unwrap();
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[0], vec![a]);
        // B and C are in the same layer (both depend only on A)
        assert_eq!(layers[1].len(), 2);
        assert!(layers[1].contains(&b));
        assert!(layers[1].contains(&c));
        assert_eq!(layers[2], vec![d]);
    }

    // --- WorkGraph: parallel independent nodes ---

    #[test]
    fn workgraph_independent_nodes_same_layer() {
        let mut g = Graph::new();
        let (a, _) = g.insert(SystemId(0));
        let (b, _) = g.insert(SystemId(1));
        let (c, _) = g.insert(SystemId(2));
        // No dependencies — all independent
        let wg = g.work_graph();
        let layers = wg.topological_order_layers().unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].len(), 3);
        assert!(layers[0].contains(&a));
        assert!(layers[0].contains(&b));
        assert!(layers[0].contains(&c));
    }

    // --- WorkGraph: virtual nodes (undefined systems) ---

    #[test]
    fn workgraph_virtual_nodes_dont_create_layers() {
        // A ──virtual──> B
        // Virtual node should not appear in output layers
        let mut g2 = Graph::new();
        let (a2, _) = g2.insert(SystemId(0));
        let (_v, _) = g2.insert(SystemId::UNDEFINED);
        let (b2, _) = g2.insert(SystemId(1));
        g2.add_dependency(a2, _v);
        g2.add_dependency(_v, b2);
        let wg = g2.work_graph();
        let systems = wg.systems_topological_order_layers().unwrap();
        // Virtual node should be collapsed: A in layer 0, B in layer 1
        assert_eq!(systems.len(), 2);
        assert_eq!(systems[0], vec![SystemId(0)]);
        assert_eq!(systems[1], vec![SystemId(1)]);
    }

    // --- WorkGraph: parent/child constraints ---

    #[test]
    fn workgraph_parent_child_enforced() {
        let mut g = Graph::new();
        let (p, _) = g.insert(SystemId(0));
        let (c1, _) = g.insert(SystemId(1));
        let (c2, _) = g.insert(SystemId(2));
        // c1 and c2 are children of p — parent/child alone doesn't create ordering
        // without explicit dependency edges. Test that parent/child is tracked.
        g.set_parent(p, c1);
        g.set_parent(p, c2);
        let wg = g.work_graph();
        // All nodes have no dependencies, so they should all be in one layer
        let layers = wg.topological_order_layers().unwrap();
        assert_eq!(layers.len(), 1);
        assert!(layers[0].contains(&p));
        assert!(layers[0].contains(&c1));
        assert!(layers[0].contains(&c2));
    }

    // --- WorkGraph: cycle detection ---

    #[test]
    fn workgraph_cycle_detected() {
        let mut g = Graph::new();
        let (a, _) = g.insert(SystemId(0));
        let (b, _) = g.insert(SystemId(1));
        let (c, _) = g.insert(SystemId(2));
        // Create a cycle: a → b → c → a
        g.add_dependency(a, b);
        g.add_dependency(b, c);
        g.add_dependency(c, a);
        let wg = g.work_graph();
        let result = wg.topological_order_layers();
        assert!(result.is_err());
        match result.unwrap_err() {
            GraphError::CycleDetected(count) => {
                // Should report remaining unprocessed nodes
                assert!(count > 0);
            }
        }
    }

    #[test]
    fn workgraph_self_cycle_detected() {
        let mut g = Graph::new();
        let (a, _) = g.insert(SystemId(0));
        // Self-dependency
        g.add_dependency(a, a);
        let wg = g.work_graph();
        assert!(wg.topological_order_layers().is_err());
    }

    // --- WorkGraph: dependent layers ---

    #[test]
    fn workgraph_dependent_layers_basic() {
        let mut g = Graph::new();
        let (a, _) = g.insert(SystemId(0));
        let (b, _) = g.insert(SystemId(1));
        g.add_dependency(a, b);
        let wg = g.work_graph();
        let dependent = wg.systems_dependent_layers().unwrap();
        assert_eq!(dependent.len(), 2);
        // A has no dependencies → UNDEFINED
        assert_eq!(dependent[0], Layer::UNDEFINED);
        // B depends on A → layer 0
        assert_eq!(dependent[1], Layer(0));
    }

    // --- WorkGraph: combined output ---

    #[test]
    fn workgraph_combined_output() {
        let mut g = Graph::new();
        let (a, _) = g.insert(SystemId(0));
        let (b, _) = g.insert(SystemId(1));
        let (c, _) = g.insert(SystemId(2));
        g.add_dependency(a, b);
        g.add_dependency(a, c);

        let wg = g.work_graph();

        let (systems, dependent) = wg
            .systems_topological_order_with_dependent_layers()
            .unwrap();

        // Verify the topological order: a must come before b and c
        assert_eq!(systems.len(), 2);
        assert_eq!(systems[0], vec![SystemId(0)]);
        assert_eq!(systems[1].len(), 2);

        // A has no dependencies, so its dependent layer should be UNDEFINED
        // B and C depend on A, so their dependent layers should be >= 0
        assert!(dependent[1] != Layer::UNDEFINED || dependent[2] != Layer::UNDEFINED);
    }

    // --- WorkGraph: empty graph ---

    #[test]
    fn workgraph_empty_graph() {
        let g = Graph::new();
        let wg = g.work_graph();
        let layers = wg.topological_order_layers().unwrap();
        assert!(layers.is_empty());
        let systems = wg.systems_topological_order_layers().unwrap();
        assert!(systems.is_empty());
    }

    // --- WorkGraph: single node ---

    #[test]
    fn workgraph_single_node() {
        let mut g = Graph::new();
        let (a, _) = g.insert(SystemId(0));
        let wg = g.work_graph();
        let layers = wg.topological_order_layers().unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0], vec![a]);
    }

    // --- Node helpers ---

    #[test]
    fn node_is_parent_ready_no_parent() {
        let node = Node::new(SystemId(0));
        let ready = BitSet::new();
        assert!(node.is_parent_ready(&ready));
    }

    #[test]
    fn node_is_parent_ready_with_parent_not_ready() {
        let mut node = Node::new(SystemId(1));
        node.parent = NodeId(0);
        let mut ready = BitSet::new();
        ready.insert(1); // only node 1 is ready, not parent
        assert!(!node.is_parent_ready(&ready));
    }

    #[test]
    fn node_are_dependencies_complete() {
        let mut g = Graph::new();
        let (a, _) = g.insert(SystemId(0));
        let (b, _) = g.insert(SystemId(1));
        g.add_dependency(a, b);
        let mut completed = BitSet::new();
        assert!(!g[b].are_dependencies_complete(&completed));
        completed.insert(a.0);
        assert!(g[b].are_dependencies_complete(&completed));
    }

    // --- NodeId ---

    #[test]
    fn node_id_undefined() {
        assert!(NodeId::UNDEFINED.is_undefined());
        assert!(!NodeId(0).is_undefined());
    }

    #[test]
    fn node_id_defined() {
        assert!(NodeId(0).is_defined());
        assert!(!NodeId::UNDEFINED.is_defined());
    }

    // --- SystemId ---

    #[test]
    fn system_id_undefined() {
        assert!(SystemId::UNDEFINED.is_undefined());
        assert!(!SystemId(0).is_undefined());
    }

    #[test]
    fn system_id_defined() {
        assert!(SystemId(0).is_defined());
        assert!(!SystemId::UNDEFINED.is_defined());
    }
}
