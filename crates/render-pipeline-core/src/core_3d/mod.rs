use pulz_ecs::prelude::*;
use pulz_render::{
    camera::{Camera, RenderTarget},
    graph::{
        pass::{builder::PassBuilder, run::PassExec, Graphics, Pass},
        resources::{Texture, WriteSlot},
        RenderGraphBuilder,
    },
    math::Mat4,
    RenderSystemPhase,
};

pub use crate::common::*;

pub struct CoreShadingModule;

impl CoreShadingModule {
    fn build_graph_system(
        mut builder: ResMut<'_, RenderGraphBuilder>,
        cams_qry: Query<'_, (&Camera, &RenderTarget, Entity)>,
    ) {
        for (camera, render_target, entity) in cams_qry {
            let output = builder.add_pass(CoreShadingPass {
                view_camera: entity,
                projection: camera.projection_matrix,
            });

            builder.export_texture(output.read(), render_target);
        }
    }
}

impl Module for CoreShadingModule {
    fn install_modules(&self, res: &mut Resources) {
        res.install(CorePipelineCommonModule);
    }

    fn install_systems(schedule: &mut Schedule) {
        schedule
            .add_system(Self::build_graph_system)
            .into_phase(RenderSystemPhase::BuildGraph);
    }
}
pub struct CoreShadingPass {
    view_camera: Entity,
    projection: Mat4,
}

impl Pass for CoreShadingPass {
    type Output = WriteSlot<Texture>;

    fn build(self, mut builder: PassBuilder<'_, Graphics>) -> (Self::Output, PassExec<Graphics>) {
        let color = builder.creates_color_attachment();
        builder.creates_depth_stencil_attachment();

        (
            color,
            PassExec::new_fn(move |mut ctx| {
                ctx.draw_phase_items::<Opaque>(self.view_camera);
                ctx.draw_phase_items::<OpaqueAlpha>(self.view_camera);
                ctx.draw_phase_items::<Transparent>(self.view_camera);
            }),
        )
    }
}
