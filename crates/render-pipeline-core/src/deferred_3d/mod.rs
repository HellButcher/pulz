use pulz_ecs::prelude::*;
use pulz_render::{
    camera::{Camera, RenderTarget},
    graph::{
        pass::{
            builder::{PassBuilder, PassGroupBuilder},
            run::PassExec,
            Graphics, Pass, PassGroup,
        },
        resources::{Slot, WriteSlot},
        RenderGraphBuilder,
    },
    math::Mat4,
    texture::Texture,
    RenderSystemPhase,
};

pub use crate::common::*;
pub struct DeferredShadingModule;

impl DeferredShadingModule {
    fn build_graph_system(
        builder: &mut RenderGraphBuilder,
        cams_qry: Query<'_, (&Camera, &RenderTarget, Entity)>,
    ) {
        for (camera, render_target, entity) in cams_qry {
            let output = builder.add_pass(DeferredShadingPass {
                view_camera: entity,
                projection: camera.projection_matrix,
            });

            builder.export_texture(output.read(), render_target);
        }
    }
}
impl Module for DeferredShadingModule {
    fn install_modules(&self, res: &mut Resources) {
        res.install(CorePipelineCommonModule);
    }

    fn install_systems(schedule: &mut Schedule) {
        schedule
            .add_system(Self::build_graph_system)
            .into_phase(RenderSystemPhase::BuildGraph);
    }
}

pub struct DeferredShadingPass {
    view_camera: Entity,
    projection: Mat4,
}

impl PassGroup for DeferredShadingPass {
    type Output = WriteSlot<Texture>;

    fn build(self, mut build: PassGroupBuilder<'_, Graphics>) -> Self::Output {
        let gbuffer = build.sub_pass(GBuffer {
            view_camera: self.view_camera,
        });
        let output = build.sub_pass(Composition {
            albedo: gbuffer.albedo.read(),
            position: gbuffer.position.read(),
            normal: gbuffer.normal.read(),
        });

        build.sub_pass(Transparency {
            view_camera: self.view_camera,
            output,
            depth: gbuffer.depth,
        })
    }
}

struct GBuffer {
    view_camera: Entity,
}

struct GBufferOutput {
    albedo: WriteSlot<Texture>,
    position: WriteSlot<Texture>,
    normal: WriteSlot<Texture>,
    depth: WriteSlot<Texture>,
}
struct Composition {
    albedo: Slot<Texture>,
    position: Slot<Texture>,
    normal: Slot<Texture>,
}
struct Transparency {
    view_camera: Entity,
    output: WriteSlot<Texture>,
    depth: WriteSlot<Texture>,
}

impl Pass for GBuffer {
    type Output = GBufferOutput;

    fn build(self, mut build: PassBuilder<'_, Graphics>) -> (Self::Output, PassExec<Graphics>) {
        let albedo = build.creates_color_attachment();
        let position = build.creates_color_attachment();
        let normal = build.creates_color_attachment();
        let depth = build.creates_depth_stencil_attachment();
        (
            GBufferOutput {
                albedo,
                position,
                normal,
                depth,
            },
            PassExec::new_fn(move |mut ctx| {
                ctx.draw_phase_items::<Opaque>(self.view_camera);
                ctx.draw_phase_items::<OpaqueAlpha>(self.view_camera);
            }),
        )
    }
}

impl Pass for Composition {
    type Output = WriteSlot<Texture>;

    fn build(self, mut build: PassBuilder<'_, Graphics>) -> (Self::Output, PassExec<Graphics>) {
        build.color_input_attachment(self.albedo);
        build.color_input_attachment(self.position);
        build.color_input_attachment(self.normal);
        let output = build.creates_color_attachment();
        (output, PassExec::noop())
    }
}

impl Pass for Transparency {
    type Output = WriteSlot<Texture>;

    fn build(
        self,
        mut build: PassBuilder<'_, Graphics>,
    ) -> (WriteSlot<Texture>, PassExec<Graphics>) {
        build.depth_stencil_attachment(self.depth);
        let output = build.color_attachment(self.output);
        (
            output,
            PassExec::new_fn(move |mut ctx| {
                ctx.draw_phase_items::<Transparent>(self.view_camera);
            }),
        )
    }
}
