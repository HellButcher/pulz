use std::error::Error;

use pulz_ecs::prelude::*;
use pulz_render::camera::{Camera, RenderTarget};
use pulz_render_ash::AshRenderer;
use pulz_render_pipeline_core::core_3d::CoreShadingModule;
use pulz_window::{WindowAttributes, WindowId, WindowModule};
use pulz_window_winit::{winit::event_loop::EventLoop, Application};
use tracing::*;

fn init() -> Resources {
    info!("Initializing...");
    let mut resources = Resources::new();
    resources.install(CoreShadingModule);

    /*
    let (window_system, window_id, window) =
        WinitWindowModule::new(WindowDescriptor::default(), &event_loop)
            .unwrap()
            .install(&mut resources);
    */

    resources.install(AshRenderer::new().unwrap());

    // let mut schedule = resources.remove::<Schedule>().unwrap();
    // schedule.init(&mut resources);
    // schedule.debug_dump_if_env(None).unwrap();
    // resources.insert_again(schedule);

    let windows = resources.install(WindowModule);
    let window_id = windows.create(WindowAttributes::new());

    setup_demo_scene(&mut resources, window_id);

    resources
}

fn setup_demo_scene(resources: &mut Resources, window: WindowId) {
    let mut world = resources.world_mut();

    world
        .spawn()
        .insert(Camera::new())
        .insert(RenderTarget::Window(window));
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn Error>> {
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .init();

    let event_loop = EventLoop::new().unwrap();
    let resources = init();
    let mut app = Application::new(resources);
    event_loop.run_app(&mut app).map_err(Into::into)
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use pulz_window_winit::winit::event_loop;
    use wasm_bindgen::prelude::*;
    use winit::platform::web::WindowExtWebSys;

    console_error_panic_hook::set_once();
    tracing_log::LogTracer::init().expect("unable to create log-tracer");
    tracing_wasm::set_as_global_default();

    let event_loop = EventLoop::new().unwrap();
    let resources = init();
    let app = Application::new(resources);

    /*
    let canvas = window.canvas();
    canvas.style().set_css_text("background-color: teal;");
    web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| doc.body())
        .and_then(|body| body.append_child(&web_sys::Element::from(canvas)).ok())
        .expect("couldn't append canvas to document body");
    */

    event_loop.spawn_app(app);
}
