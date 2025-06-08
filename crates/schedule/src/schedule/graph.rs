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
        for (layer_index, layer) in topological_order_layers.iter().enumerate() {
            for &node_id in layer {
                for &dep in self[node_id].dependencies.iter() {
                    let dep_node = &self.graph[dep];
                    if dep_node.system.is_defined() {
                        let entry = &mut result[dep_node.system.0];
                        if entry.0 > layer_index {
                            entry.0 = layer_index;
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
