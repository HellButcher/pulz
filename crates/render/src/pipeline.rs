#[derive(Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum PolygonMode {
    #[default]
    Fill,
    Line,
    Point,
}

#[derive(Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum PrimitiveTopology {
    PointList,
    LineList,
    LineStrip,
    #[default]
    TriangleList,
    TriangleStrip,
}

#[derive(Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[non_exhaustive]
pub enum VertexFormat {
    Uint8x2,
    Uint8x4,
    Sint8x2,
    Sint8x4,
    Unorm8x2,
    Unorm8x4,
    Snorm8x2,
    Snorm8x4,
    Uint16x2,
    Uint16x4,
    Sint16x2,
    Sint16x4,
    Unorm16x2,
    Unorm16x4,
    Snorm16x2,
    Snorm16x4,
    Float16,
    Float16x2,
    Float16x4,
    Float32,
    Float32x2,
    Float32x3,
    #[default]
    Float32x4,
    Float64,
    Float64x2,
    Float64x3,
    Float64x4,
    Uint32,
    Uint32x2,
    Uint32x3,
    Uint32x4,
    Sint32,
    Sint32x2,
    Sint32x3,
    Sint32x4,
}

#[derive(Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum IndexFormat {
    Uint16,
    #[default]
    Uint32,
}

#[derive(Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum FrontFace {
    #[default]
    CounterClockwise,
    Clockwise,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum Face {
    Front,
    Back,
}
