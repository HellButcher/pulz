use crate::pipeline::{IndexFormat, PrimitiveTopology, VertexFormat};

pub struct Mesh {
    primitive_topology: PrimitiveTopology,
    indices: Option<Indices>,
}

impl Mesh {
    pub const ATTRIBUTE_POSITION: MeshVertexAttribute =
        MeshVertexAttribute("Vertex_Position", 0, VertexFormat::Float32x3);
    pub const ATTRIBUTE_NORMAL: MeshVertexAttribute =
        MeshVertexAttribute("Vertex_Normal", 1, VertexFormat::Float32x3);
    pub const ATTRIBUTE_UV_0: MeshVertexAttribute =
        MeshVertexAttribute("Vertex_Uv0", 2, VertexFormat::Float32x2);
    pub const ATTRIBUTE_UV_1: MeshVertexAttribute =
        MeshVertexAttribute("Vertex_Uv1", 3, VertexFormat::Float32x2);
    pub const ATTRIBUTE_TANGENT: MeshVertexAttribute =
        MeshVertexAttribute("Vertex_Tangent", 4, VertexFormat::Float32x4);
    pub const ATTRIBUTE_COLOR_0: MeshVertexAttribute =
        MeshVertexAttribute("Vertex_Color0", 5, VertexFormat::Float32x4);
    pub const ATTRIBUTE_COLOR_1: MeshVertexAttribute =
        MeshVertexAttribute("Vertex_Color1", 6, VertexFormat::Float32x4);
    pub const ATTRIBUTE_JOINT_WEIGHT: MeshVertexAttribute =
        MeshVertexAttribute("Vertex_JointWeight", 7, VertexFormat::Float32x4);
    pub const ATTRIBUTE_JOINT_INDEX: MeshVertexAttribute =
        MeshVertexAttribute("Vertex_JointIndex", 8, VertexFormat::Uint16x4);

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

    #[inline]
    pub fn indices(&self) -> Option<&Indices> {
        self.indices.as_ref()
    }
}

#[derive(Debug, Clone)]
pub enum Indices {
    U16(Vec<u16>),
    U32(Vec<u32>),
}

impl Indices {
    pub fn format(&self) -> IndexFormat {
        match self {
            Self::U16(_) => IndexFormat::Uint16,
            Self::U32(_) => IndexFormat::Uint32,
        }
    }
}

pub struct MeshVertexAttribute {
    pub name: &'static str,
    pub id: u32,
    pub format: VertexFormat,
}

#[allow(non_snake_case)]
#[inline]
pub const fn MeshVertexAttribute(
    name: &'static str,
    id: u32,
    format: VertexFormat,
) -> MeshVertexAttribute {
    MeshVertexAttribute { name, id, format }
}

impl MeshVertexAttribute {
    #[inline]
    pub const fn new(name: &'static str, id: u32, format: VertexFormat) -> Self {
        Self { name, id, format }
    }

    #[inline]
    pub const fn at(&self, shader_location: u32) -> MeshVertexAttributeLocation {
        MeshVertexAttributeLocation {
            attribute_id: self.id,
            shader_location,
        }
    }
}

pub struct MeshVertexAttributeLocation {
    pub attribute_id: u32,
    pub shader_location: u32,
}

impl VertexFormat {
    pub fn size(self) -> usize {
        match self {
            Self::Uint8x2 => 2,
            Self::Uint8x4 => 4,
            Self::Sint8x2 => 2,
            Self::Sint8x4 => 4,
            Self::Unorm8x2 => 2,
            Self::Unorm8x4 => 4,
            Self::Snorm8x2 => 2,
            Self::Snorm8x4 => 4,
            Self::Uint16x2 => 4,
            Self::Uint16x4 => 8,
            Self::Sint16x2 => 4,
            Self::Sint16x4 => 8,
            Self::Unorm16x2 => 4,
            Self::Unorm16x4 => 8,
            Self::Snorm16x2 => 4,
            Self::Snorm16x4 => 8,
            Self::Float16 => 2,
            Self::Float16x2 => 4,
            Self::Float16x4 => 8,
            Self::Float32 => 4,
            Self::Float32x2 => 8,
            Self::Float32x3 => 12,
            Self::Float32x4 => 16,
            Self::Float64 => 8,
            Self::Float64x2 => 16,
            Self::Float64x3 => 24,
            Self::Float64x4 => 32,
            Self::Uint32 => 4,
            Self::Uint32x2 => 8,
            Self::Uint32x3 => 12,
            Self::Uint32x4 => 16,
            Self::Sint32 => 4,
            Self::Sint32x2 => 8,
            Self::Sint32x3 => 12,
            Self::Sint32x4 => 16,
        }
    }
}
