use std::error::Error;

use pulz_ecs::prelude::*;
use pulz_window_winit::Application;
use tracing::*;
use winit::event_loop::EventLoop;

fn init() -> Resources {
    info!("Initializing...");
    Resources::new()
    /*
    let window_attributes = pulz_window_winit::default_window_attributes(&event_loop);
    let (window_system, _window_id, window) =
        WinitWindowModule::new(WindowDescriptor::default(), &event_loop)
            .unwrap()
            .install(&mut resources);

    resources
    */
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
    use wasm_bindgen::prelude::*;
    use winit::platform::web::*;

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
