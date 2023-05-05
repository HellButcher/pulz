use std::rc::Rc;

use pulz_ecs::prelude::*;
use pulz_window::WindowDescriptor;
use pulz_window_winit::{WinitWindowModule, WinitWindowSystem};
use tracing::*;
use winit::{event_loop::EventLoop, window::Window};

fn init() -> (Resources, EventLoop<()>, Rc<Window>, WinitWindowSystem) {
    info!("Initializing...");
    let mut resources = Resources::new();

    let event_loop = EventLoop::new();

    let (window_system, _window_id, window) =
        WinitWindowModule::new(WindowDescriptor::default(), &event_loop)
            .unwrap()
            .install(&mut resources);

    (resources, event_loop, window, window_system)
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .init();

    let (resources, event_loop, _window, window_system) = init();

    window_system.run(resources, event_loop);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::prelude::*;
    use winit::platform::web::*;

    console_error_panic_hook::set_once();
    tracing_log::LogTracer::init().expect("unable to create log-tracer");
    tracing_wasm::set_as_global_default();

    let (resources, event_loop, window, window_system) = init();

    let canvas = window.canvas();
    canvas.style().set_css_text("background-color: teal;");
    web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| doc.body())
        .and_then(|body| body.append_child(&web_sys::Element::from(canvas)).ok())
        .expect("couldn't append canvas to document body");

    window_system.spawn(resources, event_loop);
}
