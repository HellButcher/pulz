use downcast_rs::{impl_downcast, Downcast};
use slotmap::{new_key_type, Key};
use std::borrow::Cow;

use ecs::resource::Resources;

use super::{
    add_all_to_vecset,
    context::RenderGraphContext,
    slot::{SlotAccess, SlotDescriptor, SlotLabel, SlotType},
    GraphError,
};

mod swapchain;
pub use swapchain::*;

mod simple;
pub use simple::SimpleTriangleRenderNode;

new_key_type! {
    pub struct NodeId;
}

#[derive(Copy, Clone, Debug)]
pub enum NodeLabel<'a> {
    Id(NodeId),
    Name(&'a str),
}

pub trait Node: Downcast {
    fn slots(&self) -> Cow<'static, [SlotDescriptor]> {
        Cow::Borrowed(&[])
    }

    fn is_active(&self) -> bool {
        false
    }
    fn update(&mut self, _res: &mut Resources) {}

    fn run<'c>(&self, _graph: &'c mut RenderGraphContext<'_>) -> Result<(), GraphError<'c>> {
        Ok(())
    }
}

impl_downcast!(Node);

pub(crate) struct NodeEntry {
    pub id: NodeId,
    pub name: Cow<'static, str>,
    pub type_name: &'static str,
    pub node: Box<dyn Node>,
    pub slots: Cow<'static, [SlotDescriptor]>,
    pub node_dependencies: Vec<NodeId>,
    pub slot_dependencies: Vec<(NodeId, u32)>,
    pub node_dependents: Vec<NodeId>,
    pub active: bool,
}

impl std::fmt::Debug for NodeEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeEntry")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("type_name", &self.type_name)
            .field("active", &self.active)
            .field("node_dependencies", &self.node_dependencies)
            .field("node_dependents", &self.node_dependents)
            .finish_non_exhaustive()
    }
}

fn resolve_slot_label(
    slots: &[SlotDescriptor],
    label: SlotLabel<'_>,
) -> Option<(u32, SlotAccess, SlotType)> {
    match label {
        SlotLabel::Index(index) => {
            let slot = slots.get(index as usize)?;
            Some((index, slot.access, slot.slot_type))
        }
        SlotLabel::Name(name) => {
            for (index, slot) in slots.iter().enumerate() {
                if slot.name == name {
                    return Some((index as u32, slot.access, slot.slot_type));
                }
            }
            None
        }
    }
}

impl NodeEntry {
    pub fn new<N: Node>(id: NodeId, name: Cow<'static, str>, node: N) -> Self {
        let slots = node.slots();
        Self {
            id,
            name,
            type_name: std::any::type_name::<N>(),
            node: Box::new(node),
            slots,
            node_dependencies: Vec::new(),
            slot_dependencies: Vec::new(),
            node_dependents: Vec::new(),
            active: false,
        }
    }

    #[inline]
    pub fn slot(&self, label: SlotLabel<'_>) -> Option<(u32, SlotAccess, SlotType)> {
        resolve_slot_label(&self.slots, label)
    }

    pub fn add_dependency(&mut self, node_id: NodeId) -> &mut Self {
        add_all_to_vecset(&mut self.node_dependencies, &[node_id]);
        self
    }

    pub fn add_dependent(&mut self, node_id: NodeId) -> &mut Self {
        add_all_to_vecset(&mut self.node_dependents, &[node_id]);
        self
    }

    pub fn set_input(
        &mut self,
        input_slot: u32,
        source_node: NodeId,
        source_slot: u32,
    ) -> &mut Self {
        let input_slot = input_slot as usize;
        assert!(input_slot < self.slots.len(), "invalid slot {}", input_slot);
        self.add_dependency(source_node);
        if self.slot_dependencies.len() <= input_slot {
            self.slot_dependencies
                .resize(self.slots.len(), (NodeId::null(), u32::MAX));
        }
        // SAFETY: we have resized the vector, when it was too small
        let entry = unsafe { self.slot_dependencies.get_unchecked_mut(input_slot) };
        entry.0 = source_node;
        entry.1 = source_slot;
        self
    }

    pub fn get_input(&self, input_slot: u32) -> Option<(NodeId, u32)> {
        let entry = self.slot_dependencies.get(input_slot as usize)?;
        if entry.0.is_null() {
            None
        } else {
            Some(*entry)
        }
    }
}

impl From<NodeId> for NodeLabel<'_> {
    #[inline]
    fn from(value: NodeId) -> Self {
        Self::Id(value)
    }
}

impl<'a> From<&'a str> for NodeLabel<'a> {
    #[inline]
    fn from(value: &'a str) -> Self {
        Self::Name(value)
    }
}
