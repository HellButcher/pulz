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
    listener::WindowSystemListeners, Size2, WindowDescriptor, WindowId, Windows, WindowsMirror,
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
    listeners_id: ResourceId<WindowSystemListeners>,
    winit_windows_id: ResourceId<WinitWindows>,
    active: bool,
}

pub struct WinitWindowSystemMut<'l> {
    windows: ResMut<'l, Windows>,
    listeners: ResMut<'l, WindowSystemListeners>,
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
        event: WindowEvent<'_>,
    ) {
        if let Some(window_id) = self.winit_windows.window_id_map.get(&window_id).copied() {
            if matches!(event, WindowEvent::Destroyed) {
                self.close(res, window_id);
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

    fn handle_created<T>(
        &mut self,
        res: &Resources,
        event_loop: &EventLoopWindowTarget<T>,
    ) -> Result<(), OsError> {
        while let Some((window_id, window_descr)) = self.windows.pop_next_created_window() {
            if let Some(winit_window) = self.winit_windows.get(window_id) {
                self.listeners
                    .call_on_created(res, window_id, window_descr, winit_window);
            } else {
                let builder = builder_for_descriptor(window_descr);
                let winit_window = Rc::new(builder.build(event_loop)?);
                update_window_descriptor(window_descr, &winit_window);
                self.winit_windows.insert(window_id, winit_window.clone());
                self.listeners
                    .call_on_created(res, window_id, window_descr, &winit_window);
            };
        }
        Ok(())
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
            self.listeners.call_on_closed(res, window_id);
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
        let listeners_id = res.init_unsend::<WindowSystemListeners>();
        let winit_windows_id = res.init_unsend::<WinitWindows>();
        Self {
            windows_id,
            listeners_id,
            winit_windows_id,
            active: false,
        }
    }

    fn as_mut<'l>(&self, res: &'l Resources) -> WinitWindowSystemMut<'l> {
        WinitWindowSystemMut {
            windows: res.borrow_res_mut_id(self.windows_id).unwrap(),
            listeners: res.borrow_res_mut_id(self.listeners_id).unwrap(),
            winit_windows: res.borrow_res_mut_id(self.winit_windows_id).unwrap(),
        }
    }

    fn listeners_mut<'l>(&self, res: &'l Resources) -> ResMut<'l, WindowSystemListeners> {
        res.borrow_res_mut_id(self.listeners_id).unwrap()
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
            }
            Event::Resumed => {
                info!("resumed");
                self.active = true;
                let mut s = self.as_mut(resources);
                s.handle_created(resources, event_loop).unwrap();
                s.listeners.call_on_resumed(resources);
                *control_flow = winit::event_loop::ControlFlow::Poll;
            }
            Event::WindowEvent { window_id, event } => {
                self.as_mut(resources)
                    .handle_window_event(resources, window_id, event);
            }
            Event::Suspended => {
                info!("suspended");
                self.active = false;
                self.listeners_mut(resources).call_on_suspended(resources);
                *control_flow = winit::event_loop::ControlFlow::Wait;
            }
            Event::MainEventsCleared => {
                if self.active {
                    self.as_mut(resources)
                        .handle_created(resources, event_loop)
                        .unwrap();
                    schedule.run(resources);
                    if self.as_mut(resources).handle_close(resources) {
                        // all windows closed
                        *control_flow = winit::event_loop::ControlFlow::Exit;
                    }
                }
            }
            Event::LoopDestroyed => {
                info!("event loop ended");
                if self.active {
                    self.active = false;
                    self.listeners_mut(resources).call_on_suspended(resources);
                }
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
        target_os = "openbsd",
        doc
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

    #[cfg(any(target_arch = "wasm32", doc))]
    pub fn spawn<T>(mut self, mut resources: Resources, event_loop: EventLoop<T>) {
        #[cfg(target_arch = "wasm32")]
        {
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
}

impl ModuleWithOutput for WinitWindowSystem {
    type Output<'l> = Self;

    fn install_resources(self, resources: &mut Resources) -> Self {
        Self::install(resources)
    }
}
