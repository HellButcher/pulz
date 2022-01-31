pub mod context;
pub mod graph;
pub mod node;
pub mod slot;

use self::{
    node::{NodeId, NodeLabel},
    slot::{SlotLabel, SlotType},
};
use thiserror::Error;
use window::WindowId;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum GraphError<'l> {
    #[error("invalid node")]
    InvalidNode(NodeLabel<'l>),
    #[error("invalid slot")]
    InvalidSlot(NodeId, SlotLabel<'l>),

    #[error("access mismatch")]
    AccessMismatch(NodeId, SlotLabel<'l>),

    #[error("type mismatch")]
    TypeMismatch {
        label: SlotLabel<'l>,
        expected: SlotType,
        actual: SlotType,
    },
    #[error("circular reference")]
    CircularRef(NodeLabel<'l>, NodeLabel<'l>),
    #[error("No surface was aquired")]
    SurfaceTextureNotAquired(WindowId),

    #[error("slot value undefined")]
    UndefinedValue(NodeId, SlotLabel<'l>),
}

fn add_all_to_vecset(dest: &mut Vec<NodeId>, src: &[NodeId]) {
    for value in src {
        if let Err(index) = dest.binary_search(value) {
            dest.insert(index, *value);
        }
    }
}
