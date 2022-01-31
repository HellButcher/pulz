use ecs::{executor::Executor, resource::Resources, schedule::Schedule};
use pulz_render_wgpu::install_wgpu_renderer_with_window;
use render::{
    render_graph::{
        graph::RenderGraph,
        node::{Acquire, Present, SimpleTriangleRenderNode},
    },
    view::surface::Msaa,
};
use tracing::info;
use window::WindowDescriptor;
use window_winit::WinitWindowSystem;

async fn run<E: Executor>(executor: E) -> anyhow::Result<()> {
    info!("Starting...");
    let mut resources = Resources::new();
    let mut schedule = Schedule::new().with_executor(executor);

    resources.insert(Msaa { samples: 4 });

    let window_id = resources.install(WindowDescriptor::new());

    let window_system = WinitWindowSystem::init(&mut resources).unwrap();

    #[cfg(target_arch = "wasm32")]
    {
        attach_window(&resources, window_id);
    }

    {
        // TODO: SAFETY
        let renderer =
            install_wgpu_renderer_with_window(&mut resources, &mut schedule, window_id).await?;
        let simple_node = SimpleTriangleRenderNode::new(renderer.backend_mut());
        if let Some(graph) = resources.get_mut::<RenderGraph>() {
            let acquire = graph.insert("ACQUIRE", Acquire(window_id));
            let main = graph.insert("TRIANGLE", simple_node);
            let present = graph.insert("PRESENT", Present);
            graph.connect_slots(acquire, 0, main, 0)?; // connect acquire:color -> main:color
            graph.connect_slots(main, 0, present, 0)?; // connect main:color -> present:color
        }
    }

    window_system.run(resources, schedule)
}

#[cfg(not(target_os = "unknown"))]
#[async_std::main]
async fn main() -> anyhow::Result<()> {
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .init();

    run(ecs::executor::AsyncStdExecutor).await
}

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    tracing_log::LogTracer::init().expect("unable to create log-tracer");
    tracing_wasm::set_as_global_default_with_config(
        tracing_wasm::WASMLayerConfigBuilder::new()
            .set_max_level(if cfg!(debug_assertions) {
                tracing::Level::DEBUG
            } else {
                tracing::Level::INFO
            })
            .build(),
    );

    wasm_bindgen_futures::spawn_local(async move {
        run(ecs::executor::ImmediateExecutor).await.unwrap();
    })
}

#[cfg(target_arch = "wasm32")]
pub fn attach_window(resources: &Resources, window_id: window::WindowId) -> Option<web_sys::Node> {
    use window_winit::WinitWindows;
    use winit::platform::web::WindowExtWebSys;

    let winit_windows = resources.borrow_res::<WinitWindows>().unwrap();
    let winit_window = winit_windows.get(window_id).unwrap();

    web_sys::window()?
        .document()?
        .body()?
        .append_child(&web_sys::Element::from(winit_window.canvas()))
        .ok()
}
