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

use std::{ops::Deref, rc::Rc};

use fnv::FnvHashMap;
use pulz_ecs::prelude::*;
use pulz_window::{
    listener::WindowSystemListener, Size2, WindowDescriptor, WindowId, Windows, WindowsMirror,
};
use tracing::{debug, info, warn};
pub use winit;
use winit::{
    dpi::PhysicalSize,
    error::{EventLoopError, OsError},
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
    fn insert(&mut self, window_id: WindowId, winit_window: Rc<WinitWindow>) {
        self.windows.insert(window_id, winit_window.clone());
        self.window_id_map.insert(winit_window.id(), window_id);
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

    fn close(&mut self, window_id: WindowId) -> bool {
        let Some(window) = self.windows.remove(window_id) else {
            return false;
        };
        self.window_id_map.remove(&window.id());
        window.set_visible(false);
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

fn update_window_descriptor(
    window_descriptor: &mut WindowDescriptor,
    winit_window: &winit::window::Window,
) {
    window_descriptor.scale_factor = winit_window.scale_factor();
    let phys_size: [u32; 2] = winit_window.inner_size().into();
    window_descriptor.size = phys_size.into();
}

fn descriptor_from_window(winit_window: &WinitWindow) -> WindowDescriptor {
    let mut descriptor = WindowDescriptor::new();
    descriptor.title = winit_window.title().into();
    update_window_descriptor(&mut descriptor, winit_window);
    descriptor
}

fn builder_for_descriptor(descriptor: &WindowDescriptor) -> winit::window::WindowBuilder {
    let mut builder = winit::window::WindowBuilder::new().with_title(descriptor.title.to_owned());

    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::WindowBuilderExtWindows;
        builder = builder.with_drag_and_drop(false);
    }

    if descriptor.size != Size2::ZERO {
        builder = builder.with_inner_size(PhysicalSize::new(descriptor.size.x, descriptor.size.y));
    }

    builder
}

pub struct WinitWindowSystem {
    windows_id: ResourceId<Windows>,
    winit_windows_id: ResourceId<WinitWindows>,
    schedule_id: ResourceId<Schedule>,
    active: bool,
}

pub struct WinitWindowSystemMut<'l> {
    windows: ResMut<'l, Windows>,
    winit_windows: ResMut<'l, WinitWindows>,
}

pub struct WinitWindowModule {
    descriptor: WindowDescriptor,
    window: Rc<WinitWindow>,
}

impl WinitWindowModule {
    pub fn new<T>(
        mut descriptor: WindowDescriptor,
        event_loop: &EventLoopWindowTarget<T>,
    ) -> Result<Self, OsError> {
        let builder = builder_for_descriptor(&descriptor);
        let window = Rc::new(builder.build(event_loop)?);
        update_window_descriptor(&mut descriptor, &window);
        Ok(Self { descriptor, window })
    }

    pub fn from_window(window: impl Into<Rc<WinitWindow>>) -> Self {
        let window: Rc<WinitWindow> = window.into();
        let descriptor = descriptor_from_window(&window);
        Self { descriptor, window }
    }
}

impl ModuleWithOutput for WinitWindowModule {
    type Output<'l> = (WinitWindowSystem, WindowId, Rc<WinitWindow>);
    fn install_resources(self, resources: &mut Resources) -> Self::Output<'_> {
        let sys = WinitWindowSystem::install(resources);
        let mut sys_mut = sys.as_mut(resources);
        let Self { descriptor, window } = self;
        let window_id = sys_mut.add_winit_window_with_descriptor(descriptor, window.clone());
        (sys, window_id, window)
    }
}

impl WinitWindowSystemMut<'_> {
    pub fn add_window<T>(
        &mut self,
        mut descriptor: WindowDescriptor,
        event_loop: &EventLoopWindowTarget<T>,
    ) -> Result<(WindowId, Rc<WinitWindow>), OsError> {
        let builder = builder_for_descriptor(&descriptor);
        let window = Rc::new(builder.build(event_loop)?);
        update_window_descriptor(&mut descriptor, &window);
        let window_id = self.add_winit_window_with_descriptor(descriptor, window.clone());
        Ok((window_id, window))
    }

    pub fn add_winit_window(&mut self, window: impl Into<Rc<WinitWindow>>) -> WindowId {
        let window: Rc<WinitWindow> = window.into();
        let descriptor = descriptor_from_window(&window);
        self.add_winit_window_with_descriptor(descriptor, window)
    }

    fn add_winit_window_with_descriptor(
        &mut self,
        descriptor: WindowDescriptor,
        window: Rc<WinitWindow>,
    ) -> WindowId {
        let window_id = self.windows.create(descriptor);
        debug!(
            "Added window {:?} with {:?}, {:?}",
            window_id,
            window.id(),
            window.inner_size()
        );
        self.winit_windows.insert(window_id, window);
        window_id
    }

    fn handle_window_event(
        &mut self,
        res: &Resources,
        window_id: WinitWindowId,
        event: WindowEvent,
    ) {
        if let Some(window_id) = self.winit_windows.window_id_map.get(&window_id).copied() {
            if matches!(event, WindowEvent::Destroyed) {
                debug!("Window {:?} destroyed", window_id);
                self.close(res, window_id);
            } else if let Some(window) = self.windows.get_mut(window_id) {
                match event {
                    WindowEvent::CloseRequested => {
                        debug!("Window {:?} close requested", window_id);
                        window.close_requested = true;
                    }
                    WindowEvent::Resized(size) => {
                        let phys_size: [u32; 2] = size.into();
                        window.size = phys_size.into();
                    }
                    WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                        window.scale_factor = scale_factor;
                    }
                    _ => {}
                }
            }
        }
    }

    fn handle_created<T>(&mut self, res: &Resources, event_loop: &EventLoopWindowTarget<T>) {
        while let Some((window_id, window_descr)) = self.windows.pop_next_created_window() {
            let winit_window = if let Some(w) = self.winit_windows.windows.get(window_id) {
                Rc::clone(w)
            } else {
                let builder = builder_for_descriptor(window_descr);
                let winit_window =
                    Rc::new(builder.build(event_loop).expect("unable to create window"));
                update_window_descriptor(window_descr, &winit_window);
                self.winit_windows.insert(window_id, winit_window.clone());
                winit_window
            };
            res.foreach_meta_mut(move |l: &mut dyn WindowSystemListener| {
                l.on_created(window_id, window_descr, winit_window.clone());
            });
        }
    }

    // close all windows where the close_requested flag is not cleared
    fn handle_close(&mut self, res: &Resources) -> bool {
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
                self.close(res, window_id);
            }
        }
        self.winit_windows.windows.is_empty() // all windows closed
    }

    fn close(&mut self, res: &Resources, window_id: WindowId) -> bool {
        if self.winit_windows.get(window_id).is_some() {
            debug!("Window {:?} closing", window_id);
            res.foreach_meta_mut(|l: &mut dyn WindowSystemListener| l.on_closed(window_id));
            self.windows.close(window_id);
            self.winit_windows.close(window_id);
            true
        } else {
            false
        }
    }
}

impl WinitWindowSystem {
    fn install(res: &mut Resources) -> Self {
        let windows_id = res.init::<Windows>();
        let winit_windows_id = res.init_unsend::<WinitWindows>();
        let schedule_id = res.init_unsend::<Schedule>();
        Self {
            windows_id,
            winit_windows_id,
            schedule_id,
            active: false,
        }
    }

    fn as_mut<'l>(&self, res: &'l Resources) -> WinitWindowSystemMut<'l> {
        WinitWindowSystemMut {
            windows: res.borrow_res_mut_id(self.windows_id).unwrap(),
            winit_windows: res.borrow_res_mut_id(self.winit_windows_id).unwrap(),
        }
    }

    pub fn handle_event<T>(
        &mut self,
        res: &mut Resources,
        event: Event<T>,
        event_loop: &EventLoopWindowTarget<T>,
    ) {
        match event {
            Event::NewEvents(StartCause::Init) => {
                info!("event loop started...");
            }
            Event::Resumed => {
                info!("resumed");
                if !self.active {
                    self.active = true;
                    let mut s = self.as_mut(res);
                    s.handle_created(res, event_loop);
                    res.foreach_meta_mut(|l: &mut dyn WindowSystemListener| l.on_resumed());
                }
                event_loop.set_control_flow(ControlFlow::Poll);
            }
            Event::WindowEvent { window_id, event } => {
                self.as_mut(res).handle_window_event(res, window_id, event);
            }
            Event::Suspended => {
                info!("suspended");
                if self.active {
                    self.active = false;
                    res.foreach_meta_mut(|l: &mut dyn WindowSystemListener| l.on_suspended());
                }
                event_loop.set_control_flow(ControlFlow::Wait);
            }
            Event::AboutToWait => {
                if self.active {
                    self.as_mut(res).handle_created(res, event_loop);
                    let mut schedule = res.remove_id(self.schedule_id).unwrap();
                    schedule.run(res);
                    res.insert_again(schedule);
                    if self.as_mut(res).handle_close(res) {
                        // all windows closed
                        event_loop.exit();
                    }
                }
            }
            Event::LoopExiting => {
                info!("event loop ended");
                if self.active {
                    self.active = false;
                    res.foreach_meta_mut(|l: &mut dyn WindowSystemListener| l.on_suspended());
                }
            }
            _ => {}
        }
    }

    pub fn event_handler<'a, T>(
        &'a mut self,
        res: &'a mut Resources,
    ) -> impl FnMut(Event<T>, &EventLoopWindowTarget<T>) + 'a {
        let event_loop_span = tracing::trace_span!("EventLoop");
        move |event, event_loop| {
            let span = event_loop_span.enter();
            self.handle_event(res, event, event_loop);
            drop(span);
        }
    }

    #[cfg(any(not(target_arch = "wasm32"), doc))]
    pub fn run<T>(
        mut self,
        resources: &mut Resources,
        event_loop: EventLoop<T>,
    ) -> Result<(), EventLoopError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            event_loop.run(self.event_handler(resources))
        }
        #[cfg(target_arch = "wasm32")]
        {
            Ok(())
        }
    }

    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "android",
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        doc
    ))]
    pub fn pump_events<T>(
        &mut self,
        resources: &mut Resources,
        event_loop: &mut EventLoop<T>,
        timeout: Option<std::time::Duration>,
    ) -> winit::platform::pump_events::PumpStatus {
        use winit::platform::pump_events::EventLoopExtPumpEvents;
        let result = event_loop.pump_events(timeout, self.event_handler(resources));
        result
    }

    #[cfg(any(target_arch = "wasm32", doc))]
    pub fn spawn<T>(mut self, mut resources: Resources, event_loop: EventLoop<T>) {
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::EventLoopExtWebSys;
            event_loop.spawn(self.handler(&mut resources))
        }
    }
}

impl ModuleWithOutput for WinitWindowSystem {
    type Output<'l> = Self;

    fn install_resources(self, resources: &mut Resources) -> Self {
        Self::install(resources)
    }
}
