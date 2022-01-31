use crate::pipeline::PrimitiveTopology;

pub struct Mesh {
    primitive_topology: PrimitiveTopology,
    indices: Option<Indices>,
}

impl Mesh {
    pub const ATTRIB_POSITION: &'static str = "Vertex_Position";
    pub const ATTRIB_NORMAL: &'static str = "Vertex_Normal";
    pub const ATTRIB_TANGENT: &'static str = "Vertex_Tangent";
    pub const ATTRIB_COLOR: &'static str = "Vertex_Color";
    pub const ATTRIB_COLOR_2: &'static str = "Vertex_Color2";
    pub const ATTRIB_TEXT_COORDS: &'static str = "Vertex_TexCoords";
    pub const ATTRIB_TEXT_COORDS_2: &'static str = "Vertex_TexCoords2";
    pub const ATTRIB_JOINT_INDICES: &'static str = "Vertex_JointIndices";
    pub const ATTRIB_JOINT_WEIGHTS: &'static str = "Vertex_JointWeights";

    pub const fn new(primitive_topology: PrimitiveTopology) -> Self {
        Self {
            primitive_topology,
            indices: None,
        }
    }

    #[inline]
    pub fn primitive_topology(&self) -> PrimitiveTopology {
        self.primitive_topology
    }
}

#[derive(Debug, Clone)]
pub enum Indices {
    U16(Vec<u16>),
    U32(Vec<u32>),
}
