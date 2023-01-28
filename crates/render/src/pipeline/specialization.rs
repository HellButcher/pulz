use std::hash::{Hash, Hasher};

pub type SpecializationInfo<'a> = Vec<SpecializationMapEntry<'a>>;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct SpecializationMapEntry<'a> {
    pub constant_id: u32,
    pub name: &'a str,
    pub value: PipelineConstantValue,
}

#[derive(Debug, Copy, Clone)]
pub enum PipelineConstantValue {
    Bool(bool),
    Float(f32),
    Sint(i32),
    Uint(u32),
}

impl Eq for PipelineConstantValue {}

impl PartialEq for PipelineConstantValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(v1), Self::Bool(v2)) => v1.eq(v2),
            (Self::Float(v1), Self::Float(v2)) => v1.eq(v2),
            (Self::Sint(v1), Self::Sint(v2)) => v1.eq(v2),
            (Self::Uint(v1), Self::Uint(v2)) => v1.eq(v2),
            _ => false,
        }
    }
}

impl Hash for PipelineConstantValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Bool(v) => {
                state.write_u8(2);
                v.hash(state);
            }
            Self::Float(v) => {
                state.write_u8(3);
                state.write_u32(v.to_bits());
            }
            Self::Sint(v) => {
                state.write_u8(5);
                state.write_i32(*v);
            }
            Self::Uint(v) => {
                state.write_u8(7);
                state.write_u32(*v);
            }
        }
    }
}
