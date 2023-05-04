use std::rc::Rc;

use log::info;
use pulz_ecs::prelude::*;
use pulz_render::camera::{Camera, RenderTarget};
use pulz_render_ash::AshRenderer;
use pulz_render_pipeline_core::core_3d::CoreShadingModule;
use pulz_window::{WindowDescriptor, WindowId};
use pulz_window_winit::{winit, WinitWindowModule, WinitWindowSystem};
use winit::{
    event_loop::{EventLoop, EventLoopBuilder, EventLoopWindowTarget},
    window::Window,
};

#[cfg(target_os = "android")]
use platform::android::activity::AndroidApp;

fn init(event_loop: &EventLoopWindowTarget<()>) -> (Resources, Rc<Window>, WinitWindowSystem) {
    info!("Initializing...");
    let mut resources = Resources::new();
    resources.install(CoreShadingModule);
    resources.install(AshRenderer::new().unwrap());

    let (window_system, window_id, window) =
        WinitWindowModule::new(WindowDescriptor::default(), event_loop)
            .unwrap()
            .install(&mut resources);

    setup_demo_scene(&mut resources, window_id);

    (resources, window, window_system)
}

fn setup_demo_scene(resources: &mut Resources, window: WindowId) {
    let mut world = resources.world_mut();

    world
        .spawn()
        .insert(Camera::new())
        .insert(RenderTarget::Window(window));
}

#[cfg(target_os = "android")]
#[no_mangle]
pub fn android_main(app: AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;
    // #[cfg(debug_assertions)]
    // std::env::set_var("RUST_BACKTRACE", "1");
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    let event_loop = EventLoopBuilder::new().with_android_app(app).build();
    let (resources, _window, window_system) = init(&event_loop);
    window_system.run(resources, event_loop);
}
