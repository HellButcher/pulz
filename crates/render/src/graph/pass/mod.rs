use self::{
    builder::{PassBuilder, PassGroupBuilder},
    run::PassExec,
};
use super::{PassIndex, SubPassDescription};

pub mod builder;
pub mod run;

impl SubPassDescription {
    const fn new(pass_index: PassIndex, name: &'static str) -> Self {
        Self {
            pass_index,
            name,
            color_attachments: Vec::new(),
            depth_stencil_attachment: None,
            input_attachments: Vec::new(),
        }
    }
}

pub trait PipelineType: 'static {
    const BIND_POINT: PipelineBindPoint;
}
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum PipelineBindPoint {
    Graphics,
    Compute,
    RayTracing,
}

// use the types Graphics, Compute and RayTracing also as a PipelineBindPoint Value
pub use self::PipelineBindPoint::*;

pub enum Graphics {}
impl PipelineType for Graphics {
    const BIND_POINT: PipelineBindPoint = Graphics;
}

pub enum Compute {}
impl PipelineType for Compute {
    const BIND_POINT: PipelineBindPoint = Compute;
}

pub enum RayTracing {}
impl PipelineType for RayTracing {
    const BIND_POINT: PipelineBindPoint = RayTracing;
}

pub trait Pass<Q = Graphics> {
    type Output: 'static;
    fn build(self, builder: PassBuilder<'_, Q>) -> (Self::Output, PassExec<Q>);

    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

pub trait PassGroup<Q = Graphics> {
    type Output;
    fn build(self, builder: PassGroupBuilder<'_, Q>) -> Self::Output;

    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

impl<Q: PipelineType, P: Pass<Q>> PassGroup<Q> for P {
    type Output = P::Output;
    #[inline]
    fn build(self, mut builder: PassGroupBuilder<'_, Q>) -> Self::Output {
        builder.push_sub_pass(self)
    }

    fn type_name(&self) -> &'static str {
        Pass::type_name(self)
    }
}

impl<Q, F, O> Pass<Q> for F
where
    F: FnOnce(PassBuilder<'_, Q>) -> (O, PassExec<Q>),
    O: PipelineType,
{
    type Output = O;
    #[inline]
    fn build(self, builder: PassBuilder<'_, Q>) -> (Self::Output, PassExec<Q>) {
        self(builder)
    }
}
