use std::ops::{Deref, DerefMut};

use ecs::Entity;
use slotmap::Key;
use window::WindowId;

use super::{
    graph::RenderGraph,
    node::NodeEntry,
    slot::{SlotAccess, SlotBinding, SlotLabel, SlotType},
    GraphError,
};
use crate::{
    backend::CommandEncoder,
    render_resource::{BufferId, TextureId},
    view::surface::{SurfaceTarget, SurfaceTargets},
};

pub struct RenderGraphContext<'a> {
    graph: &'a RenderGraph,
    surface_targets: &'a SurfaceTargets,
    node: &'a NodeEntry,
    slot_bindings: &'a mut [Option<SlotBinding>],
    encoder: &'a mut dyn CommandEncoder,
}

impl<'a> RenderGraphContext<'a> {
    pub(super) fn new(
        graph: &'a RenderGraph,
        surface_targets: &'a SurfaceTargets,
        node: &'a NodeEntry,
        slot_bindings: &'a mut [Option<SlotBinding>],
        encoder: &'a mut dyn CommandEncoder,
    ) -> Self {
        debug_assert_eq!(
            slot_bindings.len(),
            node.slots.len(),
            "not all slots binded"
        );
        let debug_label = format!("node: {} ({})", node.name, node.type_name);
        encoder.push_debug_group(&debug_label);
        Self {
            graph,
            surface_targets,
            node,
            slot_bindings,
            encoder,
        }
    }

    // TODO: better mechanism to access swapchain images from graph
    pub fn surface_target(&self, window_id: WindowId) -> Option<SurfaceTarget> {
        if window_id.is_null() {
            // get first
            self.surface_targets.values().next().copied()
        } else {
            self.surface_targets.get(window_id).copied()
        }
    }

    pub fn input<'l>(
        &self,
        label: impl Into<SlotLabel<'l>>,
    ) -> Result<&SlotBinding, GraphError<'l>> {
        let label = label.into();
        let (index, slot_access, _slot_type) = self
            .node
            .slot(label)
            .ok_or(GraphError::InvalidSlot(self.node.id, label))?;
        if slot_access == SlotAccess::Output {
            return Err(GraphError::AccessMismatch(self.node.id, label));
        }
        if let Some(binding) = &self.slot_bindings[index as usize] {
            Ok(binding)
        } else {
            Err(GraphError::UndefinedValue(self.node.id, label))
        }
    }

    pub fn input_buffer<'l>(
        &self,
        label: impl Into<SlotLabel<'l>>,
    ) -> Result<BufferId, GraphError<'l>> {
        let label = label.into();
        match self.input(label) {
            Err(e) => Err(e),
            Ok(SlotBinding::Buffer(buffer)) => Ok(*buffer),
            Ok(value) => Err(GraphError::TypeMismatch {
                label,
                expected: SlotType::Buffer,
                actual: value.slot_type(),
            }),
        }
    }

    pub fn input_texture<'l>(
        &self,
        label: impl Into<SlotLabel<'l>>,
    ) -> Result<TextureId, GraphError<'l>> {
        let label = label.into();
        match self.input(label)? {
            SlotBinding::Texture(texture) => Ok(*texture),
            value => Err(GraphError::TypeMismatch {
                label,
                expected: SlotType::Texture,
                actual: value.slot_type(),
            }),
        }
    }

    pub fn input_entity<'l>(
        &self,
        label: impl Into<SlotLabel<'l>>,
    ) -> Result<Entity, GraphError<'l>> {
        let label = label.into();
        match self.input(label)? {
            SlotBinding::Entity(entity) => Ok(*entity),
            value => Err(GraphError::TypeMismatch {
                label,
                expected: SlotType::Entity,
                actual: value.slot_type(),
            }),
        }
    }

    pub fn output<'l>(
        &mut self,
        label: impl Into<SlotLabel<'l>>,
        value: impl Into<SlotBinding>,
    ) -> Result<(), GraphError<'l>> {
        let label = label.into();
        let (index, slot_access, slot_type) = self
            .node
            .slot(label)
            .ok_or(GraphError::InvalidSlot(self.node.id, label))?;
        if slot_access == SlotAccess::Input {
            return Err(GraphError::AccessMismatch(self.node.id, label));
        }
        let value = value.into();
        let value_type = value.slot_type();
        if slot_type != value_type {
            return Err(GraphError::TypeMismatch {
                label,
                expected: slot_type,
                actual: value_type,
            });
        }
        self.slot_bindings[index as usize] = Some(value);
        Ok(())
    }
}

impl<'a> Drop for RenderGraphContext<'a> {
    fn drop(&mut self) {
        self.encoder.pop_debug_group();
    }
}

impl<'a> Deref for RenderGraphContext<'a> {
    type Target = dyn CommandEncoder + 'a;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.encoder
    }
}

impl DerefMut for RenderGraphContext<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.encoder
    }
}
