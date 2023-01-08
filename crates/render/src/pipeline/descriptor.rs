use bitflags::bitflags;

use super::PipelineLayout;
use crate::{shader::ShaderModule, texture::TextureFormat};

#[derive(Debug, Clone, PartialEq)]
pub struct ComputePipelineDescriptor<'a> {
    pub label: Option<&'a str>,
    pub layout: Option<PipelineLayout>,
    pub module: ShaderModule,
    pub entry_point: &'a str,
    pub specialization: SpecializationInfo<'a>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphicsPipelineDescriptor<'a> {
    pub label: Option<&'a str>,
    pub layout: Option<PipelineLayout>,
    pub vertex: VertexState<'a>,
    pub primitive: PrimitiveState,
    pub depth_stencil: Option<DepthStencilState>,
    pub fragment: Option<FragmentState<'a>>,
    pub samples: u32,
    pub specialization: SpecializationInfo<'a>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RayTracingPipelineDescriptor<'a> {
    pub label: Option<&'a str>,
    pub layout: Option<PipelineLayout>,
    pub modules: Vec<RayTracingShaderModule<'a>>,
    pub groups: Vec<RayTracingShaderGroup>,
    pub max_recursion_depth: u32,
    pub specialization: SpecializationInfo<'a>,
}

pub type SpecializationInfo<'a> = Vec<SpecializationMapEntry<'a>>;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct SpecializationMapEntry<'a> {
    pub constant_id: u32,
    pub name: &'a str,
    pub value: PipelineConstantValue,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PipelineConstantValue {
    Bool(bool),
    Float(f32),
    Sint(i32),
    Uint(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VertexState<'a> {
    pub module: ShaderModule,
    pub entry_point: &'a str,
    pub buffers: &'a [VertexBufferLayout<'a>],
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VertexBufferLayout<'a> {
    pub array_stride: usize,
    pub attributes: &'a [VertexAttribute],
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct VertexAttribute {
    pub format: VertexFormat,
    pub offset: usize,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PrimitiveState {
    pub topology: PrimitiveTopology,
    pub polygon_mode: PolygonMode,
    pub front_face: FrontFace,
    pub cull_mode: Option<Face>,
    pub line_width: f32,
}

impl PrimitiveState {
    pub const DEFAULT: Self = Self {
        topology: PrimitiveTopology::TriangleList,
        polygon_mode: PolygonMode::Fill,
        front_face: FrontFace::CounterClockwise,
        cull_mode: None,
        line_width: 0.0,
    };
}

impl Default for PrimitiveState {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct DepthStencilState {
    pub format: TextureFormat,
    pub depth: DepthState,
    pub stencil: StencilState,
}

impl DepthStencilState {
    pub const DEFAULT: Self = Self {
        format: TextureFormat::DEFAULT,
        depth: DepthState::DEFAULT,
        stencil: StencilState::DEFAULT,
    };

    pub fn is_depth_enabled(&self) -> bool {
        self.depth.compare != CompareFunction::Always || self.depth.write_enabled
    }
    /// Returns true if the state doesn't mutate either depth or stencil of the target.
    pub fn is_read_only(&self) -> bool {
        !self.depth.write_enabled && self.stencil.is_read_only()
    }
}

impl Default for DepthStencilState {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct DepthState {
    pub write_enabled: bool,
    pub compare: CompareFunction,

    pub bias: i32,
    pub bias_slope_scale: f32,
    pub bias_clamp: f32,
}

impl DepthState {
    pub const DEFAULT: Self = Self {
        write_enabled: false,
        compare: CompareFunction::Always,

        bias: 0,
        bias_slope_scale: 0.0,
        bias_clamp: 0.0,
    };
}

impl Default for DepthState {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct StencilState {
    pub front: StencilFaceState,
    pub back: StencilFaceState,

    pub read_mask: u32,
    pub write_mask: u32,
}

impl StencilState {
    pub const DEFAULT: Self = Self {
        front: StencilFaceState::IGNORE,
        back: StencilFaceState::IGNORE,

        read_mask: u32::MAX,
        write_mask: u32::MAX,
    };

    pub fn is_enabled(&self) -> bool {
        (self.front != StencilFaceState::IGNORE || self.back != StencilFaceState::IGNORE)
            && (self.read_mask != 0 || self.write_mask != 0)
    }
    /// Returns true if the state doesn't mutate the target values.
    pub fn is_read_only(&self) -> bool {
        self.write_mask == 0
    }
}

impl Default for StencilState {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct StencilFaceState {
    pub compare: CompareFunction,
    pub fail_op: StencilOperation,
    pub depth_fail_op: StencilOperation,
    pub pass_op: StencilOperation,
}

impl StencilFaceState {
    pub const IGNORE: Self = Self {
        compare: CompareFunction::Always,
        fail_op: StencilOperation::Keep,
        depth_fail_op: StencilOperation::Keep,
        pass_op: StencilOperation::Keep,
    };
}

impl Default for StencilFaceState {
    fn default() -> Self {
        Self::IGNORE
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FragmentState<'a> {
    pub module: ShaderModule,
    pub entry_point: &'a str,
    pub targets: &'a [ColorTargetState],
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ColorTargetState {
    pub format: TextureFormat,
    pub blend: Option<BlendState>,
    pub write_mask: ColorWrite,
}

impl ColorTargetState {
    pub const DEFAULT: Self = Self {
        format: TextureFormat::DEFAULT,
        blend: None,
        write_mask: ColorWrite::ALL,
    };
}

impl Default for ColorTargetState {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl From<TextureFormat> for ColorTargetState {
    fn from(format: TextureFormat) -> Self {
        Self {
            format,
            blend: None,
            write_mask: ColorWrite::ALL,
        }
    }
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub struct BlendState {
    pub color: BlendComponent,
    pub alpha: BlendComponent,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct BlendComponent {
    pub operation: BlendOperation,
    pub src_factor: BlendFactor,
    pub dst_factor: BlendFactor,
}

impl BlendComponent {
    pub const DEFAULT: Self = Self {
        operation: BlendOperation::Add,
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::Zero,
    };
}

impl Default for BlendComponent {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Default, Copy, Clone, Debug, Eq, PartialEq)]
pub struct RayTracingShaderGroup {
    pub group_type: RayTracingGroupType,
    pub general_shader: u32,
    pub closest_hit_shader: u32,
    pub any_hit_shader: u32,
    pub intersection_shader: u32,
}

#[derive(Default, Copy, Clone, Debug, Eq, PartialEq)]
pub struct RayTracingShaderModule<'a> {
    pub stage: RayTracingStage,
    pub module: ShaderModule,
    pub entry_point: &'a str,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash)]
pub enum RayTracingStage {
    #[default]
    Raygen,
    AnyHit,
    ClosestHit,
    Miss,
    Intersection,
    Callable,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash)]
pub enum RayTracingGroupType {
    #[default]
    General,
    TrianglesHitGroup,
    ProceduralHitGroup,
}

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
pub enum VertexStepMode {
    #[default]
    Vertex,
    Instance,
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

#[derive(Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum CompareFunction {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    #[default]
    Always,
}

#[derive(Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum StencilOperation {
    #[default]
    Keep,
    Zero,
    Replace,
    Invert,
    IncrementClamp,
    DecrementClamp,
    IncrementWrap,
    DecrementWrap,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum BlendOperation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum BlendFactor {
    Zero,
    One,
    Src,
    OneMinusSrc,
    SrcAlpha,
    OneMinusSrcAlpha,
    Dst,
    OneMinusDst,
    DstAlpha,
    OneMinusDstAlpha,
    SrcAlphaSaturated,
    Constant,
    OneMinusConstant,
}

bitflags! {
    pub struct ColorWrite: u32 {
        const RED = 1;
        const GREEN = 2;
        const BLUE = 4;
        const ALPHA = 8;

        const ALL = 0xF;
    }
}

impl Default for ColorWrite {
    fn default() -> Self {
        Self::ALL
    }
}
