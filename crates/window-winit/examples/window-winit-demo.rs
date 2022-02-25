use ecs::{executor::Executor, resource::Resources, schedule::Schedule};
use pulz_window_winit::WinitWindowSystem;
use tracing::*;
use window::WindowDescriptor;

fn run<E: Executor>(executor: E) {
    info!("Starting...");
    let mut resources = Resources::new();
    let schedule = Schedule::new().with_executor(executor);

    #[allow(unused_variables)]
    let window_id = resources.install(WindowDescriptor::new());

    let window_system = WinitWindowSystem::init(&mut resources).unwrap();

    #[cfg(target_arch = "wasm32")]
    {
        wasm::show_window(&resources, window_id);
    }

    window_system.run(resources, schedule)
}

#[cfg(not(target_os = "unknown"))]
#[async_std::main]
async fn main() {
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .init();

    run(ecs::executor::AsyncStdExecutor)
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::prelude::*;

    console_error_panic_hook::set_once();
    tracing_log::LogTracer::init().expect("unable to create log-tracer");
    tracing_wasm::set_as_global_default();

    run(ecs::executor::ImmediateExecutor);
}

#[cfg(target_arch = "wasm32")]
pub mod wasm {
    use ecs::resource::Resources;
    use pulz_window_winit::WinitWindows;
    use wasm_bindgen::prelude::*;
    use window::{WindowId, Windows};
    use winit::platform::web::WindowExtWebSys;

    pub fn show_window(resources: &Resources, window_id: WindowId) {
        let windows = resources.borrow_res::<Windows>().unwrap();
        let window = windows.get(window_id).unwrap();

        let winit_windows = resources.borrow_res::<WinitWindows>().unwrap();
        let winit_window = winit_windows.get(window_id).unwrap();

        attach_canvas(winit_window.canvas(), &window.title)
    }

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = ["window", "demo"])]
        fn attach_canvas(canvas: web_sys::HtmlCanvasElement, title: &str);
    }
}
