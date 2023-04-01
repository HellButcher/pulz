use std::{
    borrow::Cow,
    hash::{Hash, Hasher},
};

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

use crate::{
    pipeline::{GraphicsPass, PipelineLayout, SpecializationInfo},
    shader::ShaderModule,
    texture::TextureFormat,
};

crate::backend::define_gpu_resource!(GraphicsPipeline, GraphicsPipelineDescriptor<'l>);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GraphicsPipelineDescriptor<'a> {
    pub label: Option<&'a str>,
    #[serde(with = "crate::utils::serde_slots::option")]
    pub layout: Option<PipelineLayout>,
    pub vertex: VertexState<'a>,
    pub primitive: PrimitiveState,
    pub depth_stencil: Option<DepthStencilState>,
    pub fragment: Option<FragmentState<'a>>,
    pub samples: u32,
    pub specialization: SpecializationInfo<'a>,
    #[serde(with = "crate::utils::serde_slots")]
    pub graphics_pass: GraphicsPass,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VertexState<'a> {
    #[serde(with = "crate::utils::serde_slots")]
    pub module: ShaderModule,
    pub entry_point: &'a str,
    pub buffers: Cow<'a, [VertexBufferLayout<'a>]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VertexBufferLayout<'a> {
    pub array_stride: usize,
    pub attributes: Cow<'a, [VertexAttribute]>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VertexAttribute {
    pub format: VertexFormat,
    pub offset: usize,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
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

impl Eq for PrimitiveState {}

impl PartialEq for PrimitiveState {
    fn eq(&self, other: &Self) -> bool {
        self.topology.eq(&other.topology)
            && self.polygon_mode.eq(&other.polygon_mode)
            && self.front_face.eq(&other.front_face)
            && self.cull_mode.eq(&other.cull_mode)
            && self.line_width.eq(&other.line_width)
    }
}

impl Hash for PrimitiveState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.topology.hash(state);
        self.polygon_mode.hash(state);
        self.front_face.hash(state);
        self.cull_mode.hash(state);
        state.write_u32(self.line_width.to_bits());
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DepthStencilState {
    pub format: TextureFormat,
    pub depth: DepthState,
    pub stencil: StencilState,
}

impl DepthStencilState {
    pub const DEFAULT: Self = Self {
        format: TextureFormat::Depth24PlusStencil8,
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

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
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

impl Eq for DepthState {}

impl PartialEq for DepthState {
    fn eq(&self, other: &Self) -> bool {
        self.write_enabled.eq(&other.write_enabled)
            && self.compare.eq(&other.compare)
            && self.bias.eq(&other.bias)
            && self.bias_slope_scale.eq(&other.bias_slope_scale)
            && self.bias_clamp.eq(&other.bias_clamp)
    }
}

impl Hash for DepthState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.write_enabled.hash(state);
        self.compare.hash(state);
        state.write_i32(self.bias);
        state.write_u32(self.bias_slope_scale.to_bits());
        state.write_u32(self.bias_clamp.to_bits());
    }
}

impl Default for DepthState {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FragmentState<'a> {
    #[serde(with = "crate::utils::serde_slots")]
    pub module: ShaderModule,
    pub entry_point: &'a str,
    pub targets: Cow<'a, [ColorTargetState]>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlendState {
    pub color: BlendComponent,
    pub alpha: BlendComponent,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(
    Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize,
)]
pub enum PolygonMode {
    #[default]
    Fill,
    Line,
    Point,
}

#[derive(
    Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize,
)]
pub enum PrimitiveTopology {
    PointList,
    LineList,
    LineStrip,
    #[default]
    TriangleList,
    TriangleStrip,
}

#[derive(
    Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize,
)]
pub enum VertexStepMode {
    #[default]
    Vertex,
    Instance,
}

#[derive(
    Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize,
)]
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

#[derive(
    Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize,
)]
pub enum IndexFormat {
    Uint16,
    #[default]
    Uint32,
}

#[derive(
    Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize,
)]
pub enum FrontFace {
    #[default]
    CounterClockwise,
    Clockwise,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum Face {
    Front,
    Back,
}

#[derive(
    Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize,
)]
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

#[derive(
    Copy, Clone, Default, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize,
)]
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

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum BlendOperation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
