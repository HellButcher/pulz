use std::sync::atomic::{AtomicUsize, Ordering};

use pulz_schedule::{
    event::EventWriter,
    module::{Module, system_module},
};
use tracing as log;

use crate::AppExit;

static SHOULD_EXIT: AtomicUsize = AtomicUsize::new(0);

/// Sends an [`AppExit::Success`] event to the application, indicating that it should exit.
pub fn gracefully_exit() {
    gracefully_exit_with_code(AppExit::Success);
}

/// Sends an [`AppExit`] event to the application, indicating that it should exit with the given code.
pub fn gracefully_exit_with_code(code: AppExit) {
    let intern_value = u8::from(code) as usize + 1;
    SHOULD_EXIT.store(intern_value, Ordering::Relaxed);
}

fn should_exit() -> Option<AppExit> {
    let value = SHOULD_EXIT.load(Ordering::Relaxed);
    if value == 0 {
        None
    } else {
        let code = (value - 1) as u8;
        Some(AppExit::from_code(code))
    }
}
fn install_ctrlc_handler() {
    match ctrlc::try_set_handler(gracefully_exit) {
        Ok(()) => {}
        Err(err) => log::warn!("Failed to set `Ctrl+C` handler: {err}"),
    }
}

#[derive(Default)]
pub struct CtrlCHandlerModule;

#[system_module(install_fn = install_systems_impl)]
impl CtrlCHandlerModule {
    #[system]
    fn fire_event_on_ctrlc(mut events: EventWriter<'_, AppExit>) {
        if let Some(exit_code) = should_exit() {
            log::info!("Received `Ctrl+C`, exiting with code: {exit_code:?}");
            events.send(exit_code);
        }
    }
}

impl Module for CtrlCHandlerModule {
    fn install_once(&self, _resources: &mut pulz_schedule::prelude::Resources) {
        install_ctrlc_handler();
    }
    fn install_systems(schedule: &mut pulz_schedule::prelude::Schedule) {
        Self::install_systems_impl(schedule);
    }
}
