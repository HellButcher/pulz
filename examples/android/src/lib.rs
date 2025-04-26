use log::info;
#[cfg(target_os = "android")]
use platform::android::activity::AndroidApp;
use pulz_ecs::prelude::*;
use pulz_render::camera::{Camera, RenderTarget};
use pulz_render_ash::AshRenderer;
use pulz_render_pipeline_core::core_3d::CoreShadingModule;
use pulz_window::{WindowAttributes, WindowId, WindowModule};

fn init() -> Resources {
    info!("Initializing...");
    let mut resources = Resources::new();
    resources.install(CoreShadingModule);
    resources.install(AshRenderer::new().unwrap());

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

#[cfg(target_os = "android")]
#[no_mangle]
pub fn android_main(app: AndroidApp) {
    use pulz_window_winit::Application;
    use winit::platform::android::EventLoopBuilderExtAndroid;
    // #[cfg(debug_assertions)]
    // std::env::set_var("RUST_BACKTRACE", "1");
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    let event_loop = EventLoopBuilder::new().with_android_app(app).build();
    let resources = init();
    let mut app = Application::new(resources);
    event_loop.run_app(&mut app).unwrap();
}
