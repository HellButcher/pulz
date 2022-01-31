use std::borrow::Cow;

use bitset::BitSet;
use ecs::{resource::Resources, schedule::Schedule, system::IntoSystem};
use slotmap::{SecondaryMap, SlotMap};
use tracing::*;

use crate::{
    backend::CommandEncoder,
    render_graph::{
        context::RenderGraphContext,
        node::{Node, NodeEntry, NodeId, NodeLabel},
        slot::{SlotBinding, SlotLabel},
        GraphError,
    },
    view::surface::SurfaceTargets,
    RenderSystemLabel,
};

use super::{add_all_to_vecset, slot::SlotAccess};

type HashMap<K, V> = hashbrown::HashMap<K, V, fnv::FnvBuildHasher>;

pub struct RenderGraph {
    pub(super) nodes: SlotMap<NodeId, NodeEntry>,
    node_names: HashMap<Cow<'static, str>, NodeId>,
    topological_order: Vec<NodeId>,
    dirty: bool,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self {
            nodes: SlotMap::with_key(),
            node_names: HashMap::default(),
            topological_order: Vec::new(),
            dirty: true,
        }
    }

    pub fn insert(&mut self, name: impl Into<Cow<'static, str>>, node: impl Node) -> NodeId {
        self.dirty = true;

        let name = name.into();
        let node_id = self
            .nodes
            .insert_with_key(|id| NodeEntry::new(id, name.clone(), node));
        if !name.is_empty() {
            self.node_names.insert(name, node_id);
        }
        node_id
    }

    #[inline]
    pub fn connect_slots<'l>(
        &mut self,
        output_node: impl Into<NodeLabel<'l>>,
        output_slot: impl Into<SlotLabel<'l>>,
        input_node: impl Into<NodeLabel<'l>>,
        input_slot: impl Into<SlotLabel<'l>>,
    ) -> Result<(), GraphError<'l>> {
        self._connect_slots(
            output_node.into(),
            output_slot.into(),
            input_node.into(),
            input_slot.into(),
        )
    }

    fn _connect_slots<'l>(
        &mut self,
        output_node: NodeLabel<'l>,
        output_slot: SlotLabel<'l>,
        input_node: NodeLabel<'l>,
        input_slot: SlotLabel<'l>,
    ) -> Result<(), GraphError<'l>> {
        self.dirty = true;

        let output_node_id = self
            .node_id(output_node)
            .ok_or(GraphError::InvalidNode(output_node))?;
        let input_node_id = self
            .node_id(input_node)
            .ok_or(GraphError::InvalidNode(input_node))?;

        if output_node_id == input_node_id {
            return Err(GraphError::CircularRef(output_node, input_node));
        }

        let (output_slot_index, output_access, output_type) = self
            .nodes
            .get(output_node_id)
            .ok_or(GraphError::InvalidNode(output_node))?
            .slot(output_slot)
            .ok_or(GraphError::InvalidSlot(output_node_id, output_slot))?;
        if output_access == SlotAccess::Input {
            return Err(GraphError::AccessMismatch(output_node_id, output_slot));
        }
        let input_entry = self
            .nodes
            .get_mut(input_node_id)
            .ok_or(GraphError::InvalidNode(input_node))?;

        let (input_slot_index, input_access, input_type) = input_entry
            .slot(input_slot)
            .ok_or(GraphError::InvalidSlot(input_node_id, input_slot))?;
        if input_access == SlotAccess::Output {
            return Err(GraphError::AccessMismatch(output_node_id, output_slot));
        }
        if output_type != input_type {
            return Err(GraphError::TypeMismatch {
                label: input_slot,
                expected: input_type,
                actual: output_type,
            });
        }

        input_entry.add_dependency(output_node_id);
        input_entry.set_input(input_slot_index, output_node_id, output_slot_index);
        self.nodes
            .get_mut(output_node_id)
            .unwrap()
            .add_dependent(input_node_id);

        Ok(())
    }

    #[inline]
    pub fn connect_nodes<'l>(
        &mut self,
        output_node: impl Into<NodeLabel<'l>>,
        input_node: impl Into<NodeLabel<'l>>,
    ) -> Result<(), GraphError<'l>> {
        self._connect_nodes(output_node.into(), input_node.into())
    }

    fn _connect_nodes<'l>(
        &mut self,
        output_node: NodeLabel<'l>,
        input_node: NodeLabel<'l>,
    ) -> Result<(), GraphError<'l>> {
        self.dirty = true;

        let output_node_id = self
            .node_id(output_node)
            .ok_or(GraphError::InvalidNode(output_node))?;
        let input_node_id = self
            .node_id(input_node)
            .ok_or(GraphError::InvalidNode(input_node))?;

        if output_node_id == input_node_id {
            return Err(GraphError::CircularRef(output_node, input_node));
        }

        self.nodes
            .get(output_node_id)
            .ok_or(GraphError::InvalidNode(output_node))?;
        let input_entry = self
            .nodes
            .get_mut(input_node_id)
            .ok_or(GraphError::InvalidNode(input_node))?;

        input_entry.add_dependency(output_node_id);
        self.nodes
            .get_mut(output_node_id)
            .unwrap()
            .add_dependent(input_node_id);

        Ok(())
    }

    fn node_id<'l>(&self, label: impl Into<NodeLabel<'l>>) -> Option<NodeId> {
        let label = label.into();
        match label {
            NodeLabel::Id(id) => Some(id),
            NodeLabel::Name(name) => self.node_names.get(name).copied(),
        }
    }

    pub fn get_node<'l, T: Node>(&self, label: impl Into<NodeLabel<'l>>) -> Option<&T> {
        let node_id = self.node_id(label)?;
        let entry = self.nodes.get(node_id)?;
        entry.node.downcast_ref()
    }

    pub fn get_node_mut<'l, T: Node>(&mut self, label: impl Into<NodeLabel<'l>>) -> Option<&mut T> {
        let node_id = self.node_id(label)?;
        let entry = self.nodes.get_mut(node_id)?;
        entry.node.downcast_mut()
    }

    fn build_topological_order(&self) -> Vec<NodeId> {
        // Kahn's algorithm
        let mut indegrees = SecondaryMap::with_capacity(self.nodes.len());
        let mut result = Vec::with_capacity(self.nodes.len());
        for (id, node) in self.nodes.iter() {
            let indegree = node.node_dependencies.len();
            indegrees.insert(id, indegree);
            if indegree == 0 {
                result.push(id);
            }
        }
        let mut next = 0usize;
        while next < result.len() {
            let id = result[next];
            let node = &self.nodes[id];
            next += 1;
            for dep in node.node_dependents.iter().copied() {
                let indegree = indegrees.get_mut(dep).unwrap();
                if *indegree > 0 {
                    *indegree -= 1;
                    if *indegree == 0 {
                        result.push(dep);
                    }
                }
            }
        }
        result
    }
    fn mark_active_nodes(&mut self) {
        let mut current: Vec<NodeId> = Vec::new();
        let mut next: Vec<NodeId> = Vec::new();
        for (id, node) in self.nodes.iter_mut() {
            node.active = node.node.is_active();
            if node.active {
                add_all_to_vecset(&mut next, &node.node_dependencies);
            }
        }
        while !next.is_empty() {
            std::mem::swap(&mut current, &mut next);
            for id in current.drain(..) {
                let node = &mut self.nodes[id];
                if !node.active {
                    node.active = true;
                    add_all_to_vecset(&mut next, &node.node_dependencies);
                }
            }
        }
    }

    fn rebuild(&mut self) {
        self.topological_order = self.build_topological_order();
        self.mark_active_nodes();
    }

    pub fn update(&mut self, res: &mut Resources) {
        // TODO: schedule in correct order and skip "inactive" nodes
        for (_, node) in self.nodes.iter_mut() {
            node.node.update(res);
        }
    }

    pub fn run(
        &mut self,
        _res: &Resources,
        encoder: &mut dyn CommandEncoder,
        surface_targets: &SurfaceTargets,
    ) {
        if self.dirty {
            self.dirty = false;
            self.rebuild();
            if level_enabled!(Level::DEBUG) {
                debug!("Topological Order {:#?}", self.topological_order);
                for node in self.nodes.values() {
                    debug!("GraphNode: {:#?}", node);
                }
            }
        }

        // reserve bindings
        let mut bindings: SecondaryMap<NodeId, Vec<Option<SlotBinding>>> =
            SecondaryMap::with_capacity(self.nodes.len());
        for (id, node) in self.nodes.iter() {
            if node.active {
                let mut slot_bindings = Vec::new();
                slot_bindings.resize(node.slots.len(), None);
                bindings.insert(id, slot_bindings);
            }
        }

        let mut slot_bindings = Vec::new();
        // run nodes
        for &id in &self.topological_order {
            let node = &self.nodes[id];
            if !node.active {
                continue;
            }

            let _ = tracing::trace_span!(
                "graph node",
                node = node.name.as_ref(),
                type_name = node.type_name
            )
            .entered();

            std::mem::swap(&mut slot_bindings, &mut bindings[id]);

            // prepare inputs
            for (i, slot) in node.slots.iter().enumerate() {
                if let Some((source_node, source_slot)) = node.get_input(i as u32) {
                    if let Some(binding) = bindings[source_node][source_slot as usize] {
                        slot_bindings[i] = Some(binding);
                    }
                }

                if slot.access != SlotAccess::Output && !slot.optional && slot_bindings[i].is_none()
                {
                    panic!(
                        "input slot for index {} of node {:?} ({}) not set",
                        i, node.name, node.type_name
                    );
                }
            }

            // run node
            let mut context =
                RenderGraphContext::new(self, surface_targets, node, &mut slot_bindings, encoder);
            node.node.run(&mut context).unwrap();
            drop(context);

            // validate outputs
            for (i, slot) in node.slots.iter().enumerate() {
                if slot.access == SlotAccess::Output && slot_bindings[i].is_none() {
                    panic!(
                        "no output set for index {} of node {:?} ({})",
                        i, node.name, node.type_name
                    );
                }
            }

            std::mem::swap(&mut slot_bindings, &mut bindings[id]);
        }
    }

    pub fn update_system(res: &mut Resources) {
        if let Some(mut graph) = res.remove::<Self>() {
            graph.update(res);
            res.insert_again(graph);
        }
    }

    pub fn install_into(res: &mut Resources, schedule: &mut Schedule) {
        if res.try_init_unsend::<Self>().is_ok() {
            schedule.add_system(Self::update_system.with_label(RenderSystemLabel::UpdateGraph));
        }
    }
}

impl Default for RenderGraph {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
