//! Application framework built on top of [`pulz_schedule`].
//!
//! [`App`] drives the lifecycle state machine and ticks all schedules.
//! [`AppModule`] registers time resources and lifecycle schedules.
//! Integrate by calling `App::new(resources)` and looping on `app.update()` until `app.should_exit()` is `Some`.
#![warn(
    missing_docs,
    rustdoc::missing_doc_code_examples,
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
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::wildcard_imports
)]
#![doc(html_logo_url = "https://raw.githubusercontent.com/HellButcher/pulz/master/docs/logo.png")]
#![doc(html_no_source)]
#![doc = include_str!("../README.md")]

mod app_exit;
mod graceful_exit;
mod lifecycle;

/// Built-in schedule types for each phase of the application lifecycle.
pub mod schedules;
/// Time tracking resources and the fixed-timestep schedule system.
pub mod time;

pub use app_exit::AppExit;
pub use graceful_exit::{CtrlCHandlerModule, gracefully_exit};
pub use lifecycle::{AppLifecycle, AppLifecycleController};
use pulz_schedule::{
    module::Module,
    prelude::{FromResourcesMut, Resources},
};

/// The top-level application that owns all resources and drives the lifecycle state machine.
///
/// Create with [`App::new`], then call [`App::update`] each frame and exit when
/// [`App::should_exit`] returns `Some`.
pub struct App {
    resources: Resources,
    lifecycle: AppLifecycleController,
}

impl App {
    /// Creates a new standalone application with the given resources.
    pub fn new(mut resources: Resources) -> Self {
        let lifecycle = AppLifecycleController::from_resources_mut(&mut resources);
        Self {
            resources,
            lifecycle,
        }
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

    /// Returns `Some(exit)` if the application has been requested to stop.
    pub fn should_exit(&self) -> Option<AppExit> {
        self.lifecycle.should_exit(&self.resources)
    }

    /// Advances the application by one tick, running the appropriate schedules for the current lifecycle state.
    pub fn update(&mut self) {
        self.lifecycle.update(&mut self.resources);
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

/// A [`Module`] that registers the application's time resources and lifecycle schedules.
#[derive(Copy, Clone, Debug, Default)]
pub struct AppModule;

impl Module for AppModule {
    fn init(self, res: &mut Resources) {
        self.init_lifecycle(res);
        self.init_time(res);
    }
}

/// Initialises `tracing-subscriber` with sensible defaults (env-filter + span events).
#[cfg(all(not(target_arch = "wasm32"), feature = "tracing-subscriber-init"))]
pub fn init_tracing_subscriber_defaults() {
    use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan};

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .init();
}

#[cfg(all(target_arch = "wasm32", feature = "tracing-subscriber-init"))]
pub fn init_tracing_subscriber_defaults() {
    console_error_panic_hook::set_once();
    tracing_log::LogTracer::init().expect("unable to create log-tracer");
    tracing_wasm::set_as_global_default();
}
