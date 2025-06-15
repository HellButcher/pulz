use std::time::{Duration, Instant};

pub use pulz_app::AppExit;
use pulz_app::AppLifecycleController;
use pulz_schedule::prelude::{FromResourcesMut, ResourceId, Resources};
use tracing as log;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::WindowId,
};

use crate::windows::WinitWindows;
pub struct App {
    resources: Resources,
    lifecycle: AppLifecycleController,
    should_update: bool,
    active_update_mode: UpdateMode,
    inactive_update_mode: UpdateMode,
    suspended_wait: Duration,
    winit_windows_id: ResourceId<WinitWindows>,
    app_exit: Option<AppExit>,
    #[cfg(target_arch = "wasm32")]
    exit_promise_resolve_reject: Option<(js_sys::Function, js_sys::Function)>,
}

#[derive(Copy, Clone, Debug)]
enum UpdateMode {
    Poll,
    ContinousRedraw,
    Wait(Duration),
}

impl App {
    /// Creates a new standalone application with the given resources.
    pub fn new(mut resources: Resources) -> Self {
        let lifecycle = AppLifecycleController::from_resources_mut(&mut resources);
        let winit_windows_id = resources.init_unsend::<WinitWindows>();
        Self {
            resources,
            lifecycle,
            should_update: false,
            active_update_mode: UpdateMode::ContinousRedraw,
            inactive_update_mode: UpdateMode::Wait(Duration::from_millis(40)), // 40ms=25 FPS (16ms ca 60 FPS)
            suspended_wait: Duration::from_secs(5), // only update every 5 seconds when suspended
            winit_windows_id,
            app_exit: None,
            #[cfg(target_arch = "wasm32")]
            exit_promise_resolve_reject: None,
        }
    }

    pub fn run<T: 'static>(mut self, event_loop: EventLoop<T>) -> AppExit {
        if let Err(e) = event_loop.run_app(&mut self) {
            log::error!("event loop returned an error: {e}");
            AppExit::error()
        } else {
            self.app_exit.unwrap()
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn spawn<T: 'static>(mut self, event_loop: EventLoop<T>) -> js_sys::Promise {
        js_sys::Promise::new(&mut move |resolve, reject| {
            self.exit_promise_resolve = Some((resolve, reject));
            event_loop.spawn_app(self);
        })
    }

    /// Returns a reference to the application's resources.
    pub fn resources(&self) -> &Resources {
        &self.resources
    }

    /// Returns a mutable reference to the application's resources.
    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    /// returns the owned resources from the application.
    #[inline]
    pub fn into_resources(self) -> Resources {
        self.resources
    }

    pub fn should_exit(&self) -> Option<AppExit> {
        self.lifecycle.should_exit(&self.resources)
    }
}

impl std::ops::Deref for App {
    type Target = Resources;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.resources
    }
}

impl std::ops::DerefMut for App {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.resources
    }
}

impl From<Resources> for App {
    fn from(resources: Resources) -> Self {
        Self::new(resources)
    }
}

impl From<App> for Resources {
    fn from(app: App) -> Self {
        app.into_resources()
    }
}

impl App {
    fn current_update_mode(&mut self) -> UpdateMode {
        if !self.lifecycle.is_running() {
            UpdateMode::Wait(self.suspended_wait)
        } else {
            let mut has_visible = false;
            let mut has_focus = false;
            for window in self
                .resources
                .get_mut_id(self.winit_windows_id)
                .expect("WinitWindows resource not found")
                .iter()
            {
                if window.is_visible().unwrap_or(true) {
                    has_visible = true;
                    if window.has_focus() {
                        has_focus = true;
                        break; // no need to check further
                    }
                }
            }
            if !has_visible {
                // when there are no visible windows, act like suspended.
                // (ContinuesRedraw would not work, as there are no windows to redraw)
                UpdateMode::Wait(self.suspended_wait)
            } else if has_focus {
                self.active_update_mode
            } else {
                self.inactive_update_mode
            }
        }
    }

    fn request_redraw(&mut self) {
        self.resources
            .get_mut_id(self.winit_windows_id)
            .expect("WinitWindows resource not found")
            .request_redraw();
    }
}

impl<T: 'static> ApplicationHandler<T> for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if event_loop.exiting() {
            return;
        }
        match cause {
            StartCause::Init => {
                self.lifecycle.start(&mut self.resources);
                event_loop.set_control_flow(ControlFlow::Wait);
            }
            StartCause::Poll => {
                self.should_update = true;
            }
            StartCause::ResumeTimeReached { .. } => {
                self.should_update = true;
            }
            StartCause::WaitCancelled {
                requested_resume: Some(resume),
                ..
            } => {
                let now = Instant::now();
                if resume <= now {
                    self.should_update = true;
                } else {
                    // wait until the originally requested resume time, (may be overidden later)
                    event_loop.set_control_flow(ControlFlow::WaitUntil(resume));
                }
            }
            _ => {}
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.lifecycle.resume(&mut self.resources);
        self.should_update = true;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        // TODO: only redraw on specific events? (RedrawRequested, Resized, etc.)
        self.should_update = true;
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: T) {
        self.should_update = true;
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if self.lifecycle.is_running() {
            self.should_update = true;
        }
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.lifecycle.suspend(&mut self.resources);
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        let app_exit = self.app_exit.unwrap_or(AppExit::Success);
        self.lifecycle.stop(&mut self.resources, app_exit);
        self.resources.clear();
        #[cfg(target_arch = "wasm32")]
        if let Some((resolve, reject)) = self.exit_promise_resolve_reject.take() {
            match app_exit {
                AppExit::Success => resolve
                    .call0(&js_sys::wasm_bindgen::JsValue::UNDEFINED)
                    .unwrap(),
                AppExit::Error(exit_value) => reject
                    .call1(
                        &js_sys::wasm_bindgen::JsValue::UNDEFINED,
                        &js_sys::wasm_bindgen::JsValue::from(exit_value.get()),
                    )
                    .unwrap(),
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.should_update {
            let begin_frame_time = Instant::now();

            if let Some(app_exit) = self.lifecycle.update(&mut self.resources) {
                self.app_exit = Some(app_exit);
                event_loop.exit();
            }

            self.should_update = false;

            let update_mode = self.current_update_mode();
            match update_mode {
                UpdateMode::Poll => event_loop.set_control_flow(ControlFlow::Poll),
                UpdateMode::ContinousRedraw => {
                    self.request_redraw();
                    event_loop.set_control_flow(ControlFlow::Wait);
                }
                UpdateMode::Wait(wait) => {
                    if let Some(next) = begin_frame_time.checked_add(wait) {
                        event_loop.set_control_flow(ControlFlow::WaitUntil(next));
                    } else {
                        event_loop.set_control_flow(ControlFlow::Wait);
                    }
                }
            }
        }
    }
}
