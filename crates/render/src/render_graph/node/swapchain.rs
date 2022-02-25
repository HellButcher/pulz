use std::borrow::Cow;

use window::WindowId;

use crate::render_graph::{
    context::RenderGraphContext,
    node::Node,
    slot::{SlotAccess, SlotDescriptor, SlotType},
    GraphError,
};

// pub struct SwapchainAquireNode(pub WindowId);

// impl Node for SwapchainAquireNode {
//     fn outputs(&self) -> Cow<'static, [SlotDescriptor]> {
//         Cow::Borrowed(&[SlotDescriptor {
//             name: Cow::Borrowed("surface"),
//             slot_type: SlotType::Texture,
//             optional: false
//         },
//         SlotDescriptor {
//             name: Cow::Borrowed("surface_sampled"),
//             slot_type: SlotType::Texture,
//             optional: true
//         }])
//     }

//     fn run<'c>(&self, graph: &'c mut RenderGraphContext<'_>) -> Result<(), GraphError<'c>> {
//         let target = graph
//             .surface_target(self.0)
//             .ok_or(GraphError::SurfaceTextureNotAquired(self.0))?;
//         graph.output(0, SlotBinding::Texture(target.texture))?;
//         if let Some(sampled) = target.sampled {
//             graph.output(1, SlotBinding::Texture(sampled))?;
//         }
//         Ok(())
//     }
// }

pub struct Acquire(pub WindowId);

impl Node for Acquire {
    fn slots(&self) -> Cow<'static, [SlotDescriptor]> {
        Cow::Borrowed(&[SlotDescriptor {
            name: Cow::Borrowed("acquire"),
            access: SlotAccess::Output,
            slot_type: SlotType::Texture,
            optional: false,
        }])
    }

    fn run<'c>(&self, graph: &'c mut RenderGraphContext<'_>) -> Result<(), GraphError<'c>> {
        if let Some(target) = graph.surface_target(self.0) {
            graph.output(0, target.texture)?;
        }
        Ok(())
    }
}

pub struct Present;

impl Node for Present {
    fn slots(&self) -> Cow<'static, [SlotDescriptor]> {
        Cow::Borrowed(&[SlotDescriptor {
            name: Cow::Borrowed("present"),
            access: SlotAccess::Input,
            slot_type: SlotType::Texture,
            optional: false,
        }])
    }

    fn is_active(&self) -> bool {
        true
    }

    fn run<'c>(&self, _graph: &'c mut RenderGraphContext<'_>) -> Result<(), GraphError<'c>> {
        Ok(())
    }
}
