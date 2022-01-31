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
#![doc(html_logo_url = "https://raw.githubusercontent.com/HellButcher/pulz/master/docs/logo.png")]
#![doc(html_no_source)]
#![doc = include_str!("../README.md")]

use std::{ops::Deref, rc::Rc};

use fnv::FnvHashMap;
use pulz_ecs::prelude::*;
use pulz_window::{
    RawWindow, RawWindowHandles, Size2, WindowDescriptor, WindowId, Windows, WindowsMirror,
};
use tracing::{debug, info, warn};
pub use winit;
use winit::{
    dpi::PhysicalSize,
    error::OsError,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    window::{Window as WinitWindow, WindowId as WinitWindowId},
};

#[derive(Default)]
pub struct WinitWindows {
    windows: WindowsMirror<Rc<WinitWindow>>,
    window_id_map: FnvHashMap<WinitWindowId, WindowId>,
}

impl WinitWindows {
    fn builder_for_descriptor(descriptor: &WindowDescriptor) -> winit::window::WindowBuilder {
        let mut builder =
            winit::window::WindowBuilder::new().with_title(descriptor.title.to_owned());

        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::WindowBuilderExtWindows;
            builder = builder.with_drag_and_drop(false);
        }

        if descriptor.size != Size2::ZERO {
            builder =
                builder.with_inner_size(PhysicalSize::new(descriptor.size.x, descriptor.size.y));
        }

        builder
    }

    fn create<T>(
        &mut self,
        window_id: WindowId,
        window: &mut WindowDescriptor,
        event_loop: &EventLoopWindowTarget<T>,
    ) -> Result<Rc<WinitWindow>, OsError> {
        let builder = Self::builder_for_descriptor(window);
        let winit_window = builder.build(event_loop)?;
        Self::update_window_descriptor(window, &winit_window);
        debug!(
            "created window {:?} with {:?}, {:?}",
            window_id,
            winit_window.id(),
            winit_window.inner_size(),
        );

        Ok(self.insert(window_id, winit_window))
    }

    fn insert(&mut self, window_id: WindowId, winit_window: WinitWindow) -> Rc<WinitWindow> {
        let winit_window = Rc::new(winit_window);
        self.windows.insert(window_id, winit_window.clone());
        self.window_id_map.insert(winit_window.id(), window_id);
        winit_window
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    #[inline]
    pub fn get(&self, id: WindowId) -> Option<&WinitWindow> {
        self.windows.get(id).map(Deref::deref)
    }

    fn update_window_descriptor(
        window_descriptor: &mut WindowDescriptor,
        winit_window: &winit::window::Window,
    ) {
        window_descriptor.scale_factor = winit_window.scale_factor();
        let phys_size: [u32; 2] = winit_window.inner_size().into();
        window_descriptor.size = phys_size.into();
    }

    fn close(&mut self, window_id: WindowId) -> bool {
        let Some(window) = self.windows.remove(window_id) else {
            return false;
        };
        self.window_id_map.remove(&window.id());
        window.set_visible(false);
        drop(window);
        true
    }
}

impl std::ops::Index<WindowId> for WinitWindows {
    type Output = WinitWindow;
    #[inline]
    fn index(&self, id: WindowId) -> &Self::Output {
        &self.windows[id]
    }
}

pub struct WinitWindowSystem {
    windows_id: ResourceId<Windows>,
    winit_windows_id: ResourceId<WinitWindows>,
    raw_window_handles_id: ResourceId<RawWindowHandles>,
    active: bool,
}

pub struct WinitWindowSystemMut<'l> {
    windows: ResMut<'l, Windows>,
    winit_windows: ResMut<'l, WinitWindows>,
    raw_window_handles: ResMut<'l, RawWindowHandles>,
}

pub struct WinitWindowModule {
    descriptor: WindowDescriptor,
    window: WinitWindow,
}

impl WinitWindowModule {
    pub fn new<T>(
        mut descriptor: WindowDescriptor,
        event_loop: &EventLoopWindowTarget<T>,
    ) -> Result<Self, OsError> {
        let builder = WinitWindows::builder_for_descriptor(&descriptor);
        let window = builder.build(event_loop)?;
        WinitWindows::update_window_descriptor(&mut descriptor, &window);
        Ok(Self { descriptor, window })
    }

    pub fn from_window(window: WinitWindow) -> Self {
        let mut descriptor = WindowDescriptor::default();
        WinitWindows::update_window_descriptor(&mut descriptor, &window);
        Self { descriptor, window }
    }
}

impl ModuleWithOutput for WinitWindowModule {
    type Output<'l> = (WinitWindowSystem, WindowId, Rc<WinitWindow>);
    fn install_resources(self, resources: &mut Resources) -> Self::Output<'_> {
        let sys = WinitWindowSystem::install(resources);
        let mut sys_mut = sys.as_mut(resources);
        let (window_id, window) =
            sys_mut.add_winit_window_with_descriptor(self.descriptor, self.window);
        (sys, window_id, window)
    }
}

impl WinitWindowSystemMut<'_> {
    pub fn add_window<T>(
        &mut self,
        descriptor: WindowDescriptor,
        event_loop: &EventLoopWindowTarget<T>,
    ) -> Result<(WindowId, Rc<WinitWindow>), OsError> {
        let window_id = self.windows.create(descriptor);
        let winit_window =
            self.winit_windows
                .create(window_id, &mut self.windows[window_id], event_loop)?;
        let raw_window: Rc<dyn RawWindow> = winit_window.clone();
        self.raw_window_handles
            .insert(window_id, Rc::downgrade(&raw_window));
        Ok((window_id, winit_window))
    }

    pub fn add_winit_window(&mut self, window: WinitWindow) -> WindowId {
        let mut descriptor = WindowDescriptor::default();
        WinitWindows::update_window_descriptor(&mut descriptor, &window);
        self.add_winit_window_with_descriptor(descriptor, window).0
    }

    fn add_winit_window_with_descriptor(
        &mut self,
        descriptor: WindowDescriptor,
        window: WinitWindow,
    ) -> (WindowId, Rc<WinitWindow>) {
        let window_id = self.windows.create(descriptor);
        let window = self.winit_windows.insert(window_id, window);
        let raw_window: Rc<dyn RawWindow> = window.clone();
        self.raw_window_handles
            .insert(window_id, Rc::downgrade(&raw_window));
        debug!(
            "Added window {:?} with {:?}, {:?}",
            window_id,
            window.id(),
            window.inner_size()
        );
        (window_id, window)
    }

    pub fn update_windows<T>(
        &mut self,
        event_loop: &EventLoopWindowTarget<T>,
    ) -> Result<(), OsError> {
        for (window_id, window) in self.windows.iter_mut() {
            if self.winit_windows.windows.get(window_id).is_none() {
                // create missing window
                let winit_window: Rc<dyn RawWindow> =
                    self.winit_windows.create(window_id, window, event_loop)?;
                self.raw_window_handles
                    .insert(window_id, Rc::downgrade(&winit_window));
            }

            // handle commands
            // TODO
        }
        Ok(())
    }

    fn handle_window_event(&mut self, window_id: WinitWindowId, event: WindowEvent<'_>) {
        if let Some(&window_id) = self.winit_windows.window_id_map.get(&window_id) {
            if matches!(event, WindowEvent::Destroyed) {
                self.windows.close(window_id);
                self.winit_windows.close(window_id);
                self.raw_window_handles.remove(window_id);
            } else if let Some(window) = self.windows.get_mut(window_id) {
                match event {
                    WindowEvent::CloseRequested => {
                        window.close_requested = true;
                    }
                    WindowEvent::Resized(size) => {
                        let phys_size: [u32; 2] = size.into();
                        window.size = phys_size.into();
                    }
                    WindowEvent::ScaleFactorChanged {
                        scale_factor,
                        new_inner_size,
                    } => {
                        let phys_size: [u32; 2] = (*new_inner_size).into();
                        window.scale_factor = scale_factor;
                        window.size = phys_size.into();
                    }
                    _ => {}
                }
            }
        }
    }

    fn handle_close(&mut self) -> bool {
        let mut to_close = Vec::new();
        for (window_id, _) in self.winit_windows.windows.iter() {
            match self.windows.get(window_id) {
                None => to_close.push(window_id),
                Some(w) if w.close_requested => to_close.push(window_id),
                _ => {}
            }
        }
        if !to_close.is_empty() {
            debug!("Closing {} windows", to_close.len());
            for window_id in to_close {
                self.windows.close(window_id);
                self.raw_window_handles.remove(window_id);
                self.winit_windows.close(window_id);
            }
        }
        self.winit_windows.windows.is_empty() // all windows closed
    }
}

impl WinitWindowSystem {
    fn install(res: &mut Resources) -> Self {
        let windows_id = res.init::<Windows>();
        let winit_windows_id = res.init_unsend::<WinitWindows>();
        let raw_window_handles_id = res.init_unsend::<RawWindowHandles>();
        Self {
            windows_id,
            winit_windows_id,
            raw_window_handles_id,
            active: false,
        }
    }

    fn as_mut<'l>(&self, res: &'l mut Resources) -> WinitWindowSystemMut<'l> {
        WinitWindowSystemMut {
            windows: res.borrow_res_mut_id(self.windows_id).unwrap(),
            winit_windows: res.borrow_res_mut_id(self.winit_windows_id).unwrap(),
            raw_window_handles: res.borrow_res_mut_id(self.raw_window_handles_id).unwrap(),
        }
    }

    pub fn handle_event<T>(
        &mut self,
        resources: &mut Resources,
        schedule: &mut Schedule,
        event: Event<'_, T>,
        event_loop: &EventLoopWindowTarget<T>,
        control_flow: &mut ControlFlow,
    ) {
        *control_flow = winit::event_loop::ControlFlow::Poll;

        match event {
            Event::NewEvents(StartCause::Init) => {
                info!("event loop started...");
                self.as_mut(resources).update_windows(event_loop).unwrap();
                self.active = true;
            }
            Event::WindowEvent { window_id, event } => {
                self.as_mut(resources).handle_window_event(window_id, event);
            }
            Event::Suspended => {
                info!("suspended");
                self.active = false;
                // TODO: ON ANDROID: all surfaces need to be destroyed, and re-created on RESUME
                *control_flow = winit::event_loop::ControlFlow::Wait;
            }
            Event::Resumed => {
                info!("resumed");
                self.active = true;
                // TODO: ON ANDROID: surface-creation needs to be delayed until this Event
                // TODO: clever way how to link that to the render-system, and delay creation of device
                *control_flow = winit::event_loop::ControlFlow::Poll;
            }
            Event::MainEventsCleared => {
                self.as_mut(resources).update_windows(event_loop).unwrap();
                if self.active {
                    schedule.run(resources);
                }
                if self.as_mut(resources).handle_close() {
                    // all windows closed
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                }
            }
            Event::LoopDestroyed => {
                info!("event loop ended");
                self.active = false;
            }
            _ => {}
        }
    }

    pub fn run<T>(mut self, mut resources: Resources, event_loop: EventLoop<T>) -> ! {
        let schedule_id = resources.init_unsend::<Schedule>();
        let mut schedule = resources.remove_id(schedule_id).unwrap();

        let event_loop_span = tracing::trace_span!("EventLoop");

        event_loop.run(move |event, event_loop, control_flow| {
            let span = event_loop_span.enter();
            self.handle_event(
                &mut resources,
                &mut schedule,
                event,
                event_loop,
                control_flow,
            );
            drop(span);
        })
    }

    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "android",
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub fn run_return<T>(
        &mut self,
        resources: &mut Resources,
        event_loop: &mut EventLoop<T>,
    ) -> i32 {
        use winit::platform::run_return::EventLoopExtRunReturn;

        let schedule_id = resources.init_unsend::<Schedule>();
        let mut schedule = resources.remove_id(schedule_id).unwrap();

        let event_loop_span = tracing::trace_span!("EventLoop");

        let result = event_loop.run_return(|event, event_loop, control_flow| {
            let span = event_loop_span.enter();
            self.handle_event(resources, &mut schedule, event, event_loop, control_flow);
            drop(span);
        });

        resources.insert_again(schedule);

        result
    }

    #[cfg(target_arch = "wasm32")]
    pub fn spawn<T>(mut self, mut resources: Resources, event_loop: EventLoop<T>) {
        use winit::platform::web::EventLoopExtWebSys;

        let schedule_id = resources.init_unsend::<Schedule>();
        let mut schedule = resources.remove_id(schedule_id).unwrap();

        let event_loop_span = tracing::trace_span!("EventLoop");

        event_loop.spawn(move |event, event_loop, control_flow| {
            let span = event_loop_span.enter();
            self.handle_event(
                &mut resources,
                &mut schedule,
                event,
                event_loop,
                control_flow,
            );
            drop(span);
        })
    }
}

impl ModuleWithOutput for WinitWindowSystem {
    type Output<'l> = Self;

    fn install_resources(self, resources: &mut Resources) -> Self {
        Self::install(resources)
    }
}
