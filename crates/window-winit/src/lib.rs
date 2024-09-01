#![warn(
    // missing_docs,
    // rustdoc::missing_doc_code_examples,
    future_incompatible,
    rust_2018_idioms,
    unused,
    trivial_casts,
    trivial_numeric_casts,
    unused_lifetimes,
    unused_qualifications,
    unused_crate_dependencies,
    clippy::cargo,
    clippy::multiple_crate_versions,
    clippy::empty_line_after_outer_attr,
    clippy::fallible_impl_from,
    clippy::redundant_pub_crate,
    clippy::use_self,
    clippy::suspicious_operation_groupings,
    clippy::useless_let_if_seq,
    // clippy::missing_errors_doc,
    // clippy::missing_panics_doc,
    clippy::wildcard_imports
)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(html_logo_url = "https://raw.githubusercontent.com/HellButcher/pulz/master/docs/logo.png")]
#![doc(html_no_source)]
#![doc = include_str!("../README.md")]

use std::collections::HashMap;

use pulz_ecs::{prelude::*, resource::RemovedResource};
use pulz_window::{
    listener::WindowSystemListener, Window, WindowAttributes, WindowId, Windows, WindowsMirror,
};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use tracing::{debug, info, warn};
pub use winit;
use winit::{
    application::ApplicationHandler,
    error::OsError,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow},
    window::{
        Icon, Window as WinitWindow, WindowAttributes as WinitWindowAttributes,
        WindowId as WinitWindowId,
    },
};

struct WindowState {
    window: WinitWindow,
    id: WindowId,
}

struct WinitWindowFactory {
    icon: Icon,
}
struct WinitWindowMap {
    ids: WindowsMirror<WinitWindowId>,
    state: HashMap<WinitWindowId, WindowState>,
}

pub struct Application {
    resources: Resources,
    window_factory: WinitWindowFactory,
    window_map: WinitWindowMap,
    active: bool,
    schedule: RemovedResource<Schedule>,
    windows_resource_id: ResourceId<Windows>,
}

impl WinitWindowFactory {
    pub const DEFAULT_TITLE: &'static str =
        concat!(env!("CARGO_PKG_NAME"), ": ", env!("CARGO_PKG_VERSION"));
    fn create_winit_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        mut attributes: WinitWindowAttributes,
    ) -> Result<WinitWindow, OsError> {
        if attributes.title.is_empty() {
            attributes.title = Self::DEFAULT_TITLE.to_owned();
        }
        if attributes.window_icon.is_none() {
            attributes.window_icon = Some(self.icon.clone());
        }

        #[cfg(all(
            any(feature = "x11", feature = "wayland"),
            unix,
            not(target_vendor = "apple"),
            not(target_os = "android"),
            not(target_os = "emscripten"),
            not(target_os = "redox"),
        ))]
        {
            use winit::platform::startup_notify::{
                EventLoopExtStartupNotify, WindowAttributesExtStartupNotify,
            };
            if let Some(token) = event_loop.read_token_from_env() {
                winit::platform::startup_notify::reset_activation_token_env();
                info!({ ?token }, "Using token to activate a window");
                attributes = attributes.with_activation_token(token);
            }
        }

        event_loop.create_window(attributes)
    }
}

impl WinitWindowMap {
    fn insert_winit_window(&mut self, id: WindowId, winit_window: WinitWindow) {
        let winit_window_id = winit_window.id();
        self.ids.insert(id, winit_window_id);
        self.state.insert(
            winit_window_id,
            WindowState {
                window: winit_window,
                id,
            },
        );
        info!({ ?id, ?winit_window_id }, "new window");
    }

    fn contains_id(&self, id: WindowId) -> bool {
        self.ids.contains_key(id)
    }
    fn get_mut_by_winit_id(&mut self, id: WinitWindowId) -> Option<&mut WindowState> {
        self.state.get_mut(&id)
    }

    fn is_empty(&self) -> bool {
        self.state.is_empty()
    }

    fn remove_by_winit_id(&mut self, winit_window_id: WinitWindowId) -> Option<WindowState> {
        if let Some(window_state) = self.state.remove(&winit_window_id) {
            self.ids.remove(window_state.id);
            Some(window_state)
        } else {
            None
        }
    }
}

impl Application {
    pub fn new(mut resources: Resources) -> Self {
        let windows_resource_id = resources.init::<Windows>();
        let schedule = resources.remove::<Schedule>().expect("schedule");
        let icon = load_icon(include_bytes!("icon.png"));
        Self {
            resources,
            window_factory: WinitWindowFactory { icon },
            window_map: WinitWindowMap {
                ids: WindowsMirror::new(),
                state: HashMap::new(),
            },
            active: false,
            schedule,
            windows_resource_id,
        }
    }

    pub fn into_resources(self) -> Resources {
        let Self {
            mut resources,
            schedule,
            ..
        } = self;
        resources.insert_again(schedule);
        resources
    }

    #[inline]
    pub fn resources(&self) -> &Resources {
        &self.resources
    }

    #[inline]
    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    pub fn default_window_attributes() -> WinitWindowAttributes {
        let attributes = WinitWindow::default_attributes().with_transparent(true);

        #[cfg(all(target_family = "wasm", target_os = "unknown"))]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            attributes = attributes.with_append(true);
        }

        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::WindowAttributesExtWindows;
            attributes = attributes.with_drag_and_drop(false);
        }

        attributes
    }

    fn winit_window_attributes_from_attributes(
        attributes: WindowAttributes,
    ) -> WinitWindowAttributes {
        let mut winit_attributes = Self::default_window_attributes();
        if let Some(size) = attributes.size {
            winit_attributes.inner_size =
                Some(winit::dpi::PhysicalSize::new(size.x, size.y).into());
        }
        if !attributes.title.is_empty() {
            winit_attributes.title = attributes.title.into_owned();
        }
        winit_attributes
    }

    pub fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        winit_window_attributes: WinitWindowAttributes,
    ) -> Result<WindowId, OsError> {
        let winit_window = self
            .window_factory
            .create_winit_window(event_loop, winit_window_attributes)?;
        let mut windows = self
            .resources
            .borrow_res_mut_id(self.windows_resource_id)
            .expect("Windows");
        let (id, window) = windows.create_new();
        update_window_from_winit(window, &winit_window);
        let display_handle = winit_window.display_handle().unwrap();
        let window_handle = winit_window.window_handle().unwrap();
        self.resources
            .foreach_meta_mut(|l: &mut dyn WindowSystemListener| {
                l.on_created(id, window, display_handle, window_handle)
            });
        self.window_map.insert_winit_window(id, winit_window);
        Ok(id)
    }

    fn sync_create_windows(&mut self, event_loop: &ActiveEventLoop) -> Result<(), OsError> {
        let mut windows = self
            .resources
            .borrow_res_mut_id(self.windows_resource_id)
            .expect("Windows");
        while let Some((id, window, window_attributes)) = windows.pop_next_window_to_create() {
            if window.is_pending && !window.is_close_requested & &!self.window_map.contains_id(id) {
                let winit_window_attributes =
                    Self::winit_window_attributes_from_attributes(window_attributes);
                let winit_window = self
                    .window_factory
                    .create_winit_window(event_loop, winit_window_attributes)?;
                update_window_from_winit(window, &winit_window);
                let display_handle = winit_window.display_handle().unwrap();
                let window_handle = winit_window.window_handle().unwrap();
                self.resources
                    .foreach_meta_mut(|l: &mut dyn WindowSystemListener| {
                        l.on_created(id, window, display_handle, window_handle)
                    });
                self.window_map.insert_winit_window(id, winit_window);
            }
        }
        Ok(())
    }

    fn sync_close_windows(&mut self) -> bool {
        let windows = self
            .resources
            .get_mut_id(self.windows_resource_id)
            .expect("Windows");
        let mut to_close = Vec::new();
        for (window_id, window_state) in self.window_map.state.iter() {
            match windows.get(window_state.id) {
                Some(w) if !w.is_close_requested => {}
                _ => to_close.push(*window_id),
            }
        }
        if !to_close.is_empty() {
            debug!("Closing {} windows", to_close.len());
            for winit_window_id in to_close {
                self.close_window_by_winit_id(winit_window_id);
            }
        }
        self.window_map.is_empty() // all windows closed
    }

    fn close_window_by_winit_id(&mut self, winit_window_id: WinitWindowId) -> bool {
        if let Some(window_state) = self.window_map.remove_by_winit_id(winit_window_id) {
            info!({id=?window_state.id, ?winit_window_id}, "Window closing");
            self.resources
                .foreach_meta_mut(|l: &mut dyn WindowSystemListener| l.on_closed(window_state.id));
            let windows = self
                .resources
                .get_mut_id(self.windows_resource_id)
                .expect("Windows");
            windows.close(window_state.id);
            true
        } else {
            false
        }
    }

    fn get_window_with_state_by_winit_id_mut(
        &mut self,
        winit_window_id: WinitWindowId,
    ) -> Option<(&mut Window, &mut WindowState)> {
        let window_state = self.window_map.get_mut_by_winit_id(winit_window_id)?;
        let windows = self.resources.get_mut_id(self.windows_resource_id)?;
        let window = windows.get_mut(window_state.id)?;
        Some((window, window_state))
    }

    fn run_schedule(&mut self) {
        self.schedule.run(&mut self.resources);
    }
}

fn update_window_from_winit(window: &mut Window, winit_window: &WinitWindow) {
    window.scale_factor = winit_window.scale_factor();
    let phys_size: [u32; 2] = winit_window.inner_size().into();
    window.size = phys_size.into();
    window.is_pending = false;
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        info!("resumed");
        if !self.active {
            self.active = true;
            self.sync_create_windows(event_loop).unwrap();
            self.resources
                .foreach_meta_mut(|l: &mut dyn WindowSystemListener| l.on_resumed());
        }
        event_loop.set_control_flow(ControlFlow::Poll);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        info!("suspended");
        if self.active {
            self.active = false;
            if self.sync_close_windows() {
                // all windows closed
                event_loop.exit();
            }
            self.resources
                .foreach_meta_mut(|l: &mut dyn WindowSystemListener| l.on_suspended());
        }
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        info!("event loop ended");
        if self.active {
            self.active = false;
            self.sync_close_windows();
            self.resources
                .foreach_meta_mut(|l: &mut dyn WindowSystemListener| l.on_suspended());
        }
        self.resources.clear();
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        winit_window_id: WinitWindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Destroyed => {
                self.close_window_by_winit_id(winit_window_id);
            }
            WindowEvent::CloseRequested => {
                let Some((window, window_state)) =
                    self.get_window_with_state_by_winit_id_mut(winit_window_id)
                else {
                    return;
                };
                debug!({ id=?window_state.id, ?winit_window_id}, "close requested");
                window.is_close_requested = true;
            }
            WindowEvent::Resized(size) => {
                let Some((window, _window_state)) =
                    self.get_window_with_state_by_winit_id_mut(winit_window_id)
                else {
                    return;
                };
                let phys_size: [u32; 2] = size.into();
                window.size = phys_size.into();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let Some((window, _window_state)) =
                    self.get_window_with_state_by_winit_id_mut(winit_window_id)
                else {
                    return;
                };
                window.scale_factor = scale_factor;
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.active {
            self.run_schedule();
            self.sync_create_windows(event_loop).unwrap();
            if self.sync_close_windows() {
                // all windows closed
                event_loop.exit();
            }
        }
    }
}

fn load_icon(bytes: &[u8]) -> Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(bytes).unwrap().into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
