use pulz_ecs::{prelude::Module, Entity};
use pulz_render::{
    draw::{PhaseItem, PhaseModule},
    RenderModule,
};

pub struct Opaque {
    distance: f32,
    entity: Entity,
}

impl PhaseItem for Opaque {
    type TargetKey = Entity;
    fn sort<E>(items: &mut [E])
    where
        E: std::ops::Deref<Target = Self>,
    {
        // front to back
        radsort::sort_by_key(items, |item| item.distance);
    }
}

pub struct OpaqueAlpha {
    distance: f32,
    entity: Entity,
}

impl PhaseItem for OpaqueAlpha {
    type TargetKey = Entity;
    fn sort<E>(items: &mut [E])
    where
        E: std::ops::Deref<Target = Self>,
    {
        // front to back
        radsort::sort_by_key(items, |item| item.distance);
    }
}

pub struct Transparent {
    distance: f32,
    entity: Entity,
}

impl PhaseItem for Transparent {
    type TargetKey = Entity;
    fn sort<E>(items: &mut [E])
    where
        E: std::ops::Deref<Target = Self>,
    {
        // back to front
        radsort::sort_by_key(items, |item| -item.distance);
    }
}

pub struct CorePipelineCommonModule;

impl Module for CorePipelineCommonModule {
    fn install_modules(&self, res: &mut pulz_ecs::resource::Resources) {
        res.install(RenderModule);
        res.install(PhaseModule::<Opaque>::new());
        res.install(PhaseModule::<OpaqueAlpha>::new());
        res.install(PhaseModule::<Transparent>::new());
    }
}
