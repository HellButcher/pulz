use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use super::{Graphics, PipelineType};
use crate::{
    backend::CommandEncoder,
    draw::{DrawContext, DrawPhases, PhaseItem},
};

pub trait PassRun<Q>: Send + Sync + 'static {
    fn run(&self, _ctx: PassContext<'_, Q>);
}

trait PassRunAny: Send + Sync + 'static {
    #[inline]
    fn import(&self) {}
    fn run(&self, draw_context: DrawContext<'_>, draw_phases: &DrawPhases);
}

struct PassRunAnyNoop;
struct PassRunAnyWrapper<Q, R>(R, PhantomData<fn(Q)>);

impl PassRunAny for PassRunAnyNoop {
    #[inline]
    fn run(&self, _draw_context: DrawContext<'_>, _draw_phases: &DrawPhases) {}
}

impl<Q, R> PassRunAny for PassRunAnyWrapper<Q, R>
where
    R: PassRun<Q>,
    Q: PipelineType,
{
    #[inline]
    fn run(&self, draw_context: DrawContext<'_>, draw_phases: &DrawPhases) {
        let ctx = PassContext::<'_, Q> {
            draw_context,
            draw_phases,
            _pipeline_type: PhantomData,
        };
        PassRun::<Q>::run(&self.0, ctx);
    }
}

pub struct PassExec<Q = Graphics> {
    run: Box<dyn PassRunAny>,
    _phantom: PhantomData<Q>,
}

impl<Q> PassExec<Q> {
    pub fn noop() -> Self {
        Self {
            run: Box::new(PassRunAnyNoop),
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub(crate) fn execute(&self, draw_context: DrawContext<'_>, draw_phases: &DrawPhases) {
        self.run.run(draw_context, draw_phases)
    }
}

impl<Q: PipelineType> PassExec<Q> {
    #[inline]
    pub fn new_fn<F>(run: F) -> Self
    where
        F: Fn(PassContext<'_, Q>) + Send + Sync + 'static,
    {
        Self::new(run)
    }

    pub fn new<R>(run: R) -> Self
    where
        R: PassRun<Q>,
    {
        let boxed = Box::new(PassRunAnyWrapper::<Q, R>(run, PhantomData));
        Self {
            run: boxed,
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub(crate) fn erased(self) -> PassExec<()> {
        PassExec {
            run: self.run,
            _phantom: PhantomData,
        }
    }
}

impl<Q, F> PassRun<Q> for F
where
    F: Fn(PassContext<'_, Q>) + Send + Sync + 'static,
{
    #[inline]
    fn run(&self, ctx: PassContext<'_, Q>) {
        self(ctx)
    }
}

pub struct PassContext<'a, Q = Graphics> {
    draw_context: DrawContext<'a>,
    draw_phases: &'a DrawPhases,
    _pipeline_type: PhantomData<fn(Q)>,
}

impl PassContext<'_, Graphics> {
    pub fn draw_phase_items<I>(&mut self, target_key: I::TargetKey)
    where
        I: PhaseItem,
    {
        if let Some(phase) = self.draw_phases.get::<I>(target_key) {
            phase.draw(self.draw_context);
        }
    }
}

// TODO: ambassador
impl<'a, Q> Deref for PassContext<'a, Q> {
    type Target = dyn CommandEncoder + 'a;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.draw_context
    }
}
impl<Q> DerefMut for PassContext<'_, Q> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.draw_context
    }
}
