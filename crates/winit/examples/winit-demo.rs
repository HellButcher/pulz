use pulz_schedule::prelude::Resources;
use pulz_winit::app::{App, AppExit};

fn create() -> App {
    let resources = Resources::new();
    App::new(resources)
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> AppExit {
    pulz_app::init_tracing_subscriber_defaults();
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let app = create();
    app.run(event_loop)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(main)]
fn main() {
    pulz_app::init_tracing_subscriber_defaults();
    use winit::event_loop::EventLoopExtWebSys;
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    let app = create();
    event_loop.spawn_app(app);
}
