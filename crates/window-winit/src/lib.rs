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

use std::{
    ops::{Deref, DerefMut},
    rc::Rc,
};

use tracing::{info, warn};

use ecs::{
    resource::{ResourceId, Resources},
    schedule::Schedule,
};
use fnv::FnvHashMap;
use window::{
    HasRawWindowHandle, Point2, RawWindowHandles, Size2, Window, WindowDescriptor, WindowId,
    Windows, WindowsMirror,
};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    error::OsError,
    event::Event,
    event::WindowEvent,
    window::{Window as WinitWindow, WindowId as WinitWindowId},
};

pub type WinitEventLoop = winit::event_loop::EventLoop<()>;
pub type WinitEventLoopWindowTarget = winit::event_loop::EventLoopWindowTarget<()>;

#[derive(Default)]
pub struct WinitWindows {
    windows: WindowsMirror<Rc<WinitWindow>>,
    window_id_map: FnvHashMap<WinitWindowId, WindowId>,
}

impl WinitWindows {
    fn create(
        &mut self,
        window_id: WindowId,
        window: &mut Window,
        event_loop: &WinitEventLoopWindowTarget,
    ) -> Result<Rc<WinitWindow>, OsError> {
        let mut builder = winit::window::WindowBuilder::new().with_title(window.title.to_owned());

        #[cfg(target_os = "windows")]
        {
            use winit::platform::windows::WindowBuilderExtWindows;
            builder = builder.with_drag_and_drop(false);
        }

        if window.size != Size2::ZERO {
            builder = builder.with_inner_size(PhysicalSize::new(window.size.x, window.size.y));
        }
        if window.position != Point2::new(i32::MIN, i32::MIN) {
            builder =
                builder.with_position(PhysicalPosition::new(window.position.x, window.position.y));
        }

        let winit_window = builder.build(event_loop)?;
        Self::update_window_descriptor(window, &winit_window);

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

        if let Ok(pos) = winit_window.inner_position() {
            let phys_pos: [i32; 2] = pos.into();
            window_descriptor.position = phys_pos.into();
        } else if window_descriptor.position == Point2::new(i32::MIN, i32::MIN) {
            window_descriptor.position = Point2::ZERO;
        }
    }

    fn close(&mut self, window_id: WindowId) -> bool {
        if let Some(window) = self.windows.remove(window_id) {
            self.window_id_map.remove(&window.id());
            window.set_visible(false);
            drop(window);
            true
        } else {
            false
        }
    }
}

impl std::ops::Index<WindowId> for WinitWindows {
    type Output = WinitWindow;
    #[inline]
    fn index(&self, id: WindowId) -> &Self::Output {
        &self.windows[id]
    }
}

fn handle_window_event(_window_id: WindowId, window: &mut Window, event: WindowEvent<'_>) {
    // TODO: redirect to event queue
    match event {
        WindowEvent::CloseRequested => {
            window.close_requested = true;
        }
        WindowEvent::Resized(size) => {
            let phys_size: [u32; 2] = size.into();
            window.size = phys_size.into();
        }
        WindowEvent::Moved(pos) => {
            let phys_pos: [i32; 2] = pos.into();
            window.position = phys_pos.into();
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

struct WinitWindowSystemInner {
    windows_id: ResourceId<Windows>,
    winit_windows_id: ResourceId<WinitWindows>,
    raw_window_handles_id: ResourceId<RawWindowHandles>,
}

pub struct WinitWindowSystem {
    event_loop: WinitEventLoop,
    inner: WinitWindowSystemInner,
}

impl Deref for WinitWindowSystem {
    type Target = WinitEventLoop;

    #[inline]
    fn deref(&self) -> &WinitEventLoop {
        &self.event_loop
    }
}

impl DerefMut for WinitWindowSystem {
    #[inline]
    fn deref_mut(&mut self) -> &mut WinitEventLoop {
        &mut self.event_loop
    }
}

impl WinitWindowSystemInner {
    fn update_windows(
        &self,
        res: &mut Resources,
        event_loop: &WinitEventLoopWindowTarget,
    ) -> Result<(), OsError> {
        let mut windows = res.borrow_res_mut_id(self.windows_id).unwrap();
        let mut winit_windows = res.borrow_res_mut_id(self.winit_windows_id).unwrap();
        for (window_id, window) in windows.iter_mut() {
            if winit_windows.windows.get(window_id).is_none() {
                // create window
                let mut raw_window_handles =
                    res.borrow_res_mut_id(self.raw_window_handles_id).unwrap();
                let winit_window: Rc<dyn HasRawWindowHandle> =
                    winit_windows.create(window_id, window, event_loop)?;
                raw_window_handles.insert(window_id, Rc::downgrade(&winit_window));
            }

            // handle commands
            // TODO
        }
        Ok(())
    }

    fn handle_window_event(
        &self,
        res: &mut Resources,
        window_id: WinitWindowId,
        event: WindowEvent<'_>,
    ) {
        let mut windows = res.borrow_res_mut_id(self.windows_id).unwrap();
        let mut winit_windows = res.borrow_res_mut_id(self.winit_windows_id).unwrap();
        if let Some(&window_id) = winit_windows.window_id_map.get(&window_id) {
            if matches!(event, WindowEvent::Destroyed) {
                let mut raw_window_handles =
                    res.borrow_res_mut_id(self.raw_window_handles_id).unwrap();
                windows.close(window_id);
                raw_window_handles.remove(window_id);
                winit_windows.close(window_id);
            } else if let Some(window) = windows.get_mut(window_id) {
                handle_window_event(window_id, window, event)
            }
        }
    }

    fn handle_close(&self, res: &mut Resources) -> bool {
        let mut windows = res.borrow_res_mut_id(self.windows_id).unwrap();
        let mut winit_windows = res.borrow_res_mut_id(self.winit_windows_id).unwrap();
        let mut to_close = Vec::new();
        for (window_id, _) in winit_windows.windows.iter() {
            if let Some(window) = windows.get(window_id) {
                if window.close_requested {
                    // close was requested, and flag was not reset
                    to_close.push(window_id);
                }
            } else {
                to_close.push(window_id);
            }
        }
        if !to_close.is_empty() {
            let mut raw_window_handles = res.borrow_res_mut_id(self.raw_window_handles_id).unwrap();
            for window_id in to_close {
                windows.close(window_id);
                raw_window_handles.remove(window_id);
                winit_windows.close(window_id);
            }
        }
        winit_windows.windows.is_empty() // all windows closed
    }
}

impl WinitWindowSystem {
    pub fn with_event_loop(
        res: &mut Resources,
        event_loop: WinitEventLoop,
    ) -> Result<Self, OsError> {
        let windows_id = res.init::<Windows>();
        let winit_windows_id = res.init_unsend::<WinitWindows>();
        let raw_window_handles_id = res.init_unsend::<RawWindowHandles>();
        res.insert_unsend(event_loop.create_proxy());

        let inner = WinitWindowSystemInner {
            windows_id,
            winit_windows_id,
            raw_window_handles_id,
        };

        inner.update_windows(res, &event_loop)?;

        Ok(Self { event_loop, inner })
    }

    pub fn init(res: &mut Resources) -> Result<Self, OsError> {
        if let Some(event_loop) = res.remove::<WinitEventLoop>() {
            Self::with_event_loop(res, event_loop.into_inner())
        } else {
            Self::with_event_loop(res, WinitEventLoop::new())
        }
    }

    pub fn run(self, mut res: Resources, mut schedule: Schedule) -> ! {
        info!("Entering event loop...");
        let event_loop_span = tracing::trace_span!("Event Loop");

        let WinitWindowSystem { inner, event_loop } = self;

        inner.update_windows(&mut res, &event_loop).unwrap();

        let mut active = true;

        event_loop.run(move |event, event_loop, control_flow| {
            let span = event_loop_span.enter();
            *control_flow = winit::event_loop::ControlFlow::Poll;

            match event {
                Event::WindowEvent { window_id, event } => {
                    inner.handle_window_event(&mut res, window_id, event);
                }
                Event::Suspended => {
                    active = false;
                    *control_flow = winit::event_loop::ControlFlow::Wait;
                }
                Event::Resumed => {
                    active = true;
                    *control_flow = winit::event_loop::ControlFlow::Poll;
                }
                Event::MainEventsCleared => {
                    inner.update_windows(&mut res, event_loop).unwrap();
                    if active {
                        schedule.run(&mut res);
                    }
                    if inner.handle_close(&mut res) {
                        // all windows closed
                        *control_flow = winit::event_loop::ControlFlow::Exit;
                    }
                }
                Event::LoopDestroyed => {
                    info!("event loop ended");
                }
                _ => {}
            }

            drop(span);
        })
    }
}
