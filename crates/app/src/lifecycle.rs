use pulz_schedule::{
    event::Events,
    prelude::{FromResourcesMut, ResourceId, Resources, Schedule},
};
use tracing as log;

use crate::{
    AppExit, AppModule,
    schedules::{MainSchedule, ResumeSchedule, StartupSchedule, StopSchedule, SuspendSchedule},
};

/// Enum describing the lifecycle state of the application.
///  ```txt
/// [CREATED] -> STARTING --> RESUMING --> [RUNNING]
///     |                         ^            |
///     v                         |            v
/// [STOPPED] <- STOPPING <- [SUSPENDED] <- SUSPENDING
/// ```
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AppLifecycle {
    /// The application has been created and is not started yet.
    #[default]
    Created,

    /// The application is starting up. (running the StartupSchedule)
    Starting,

    /// The application is resuming from a suspended state or starting up. (running the ResumeSchedule)
    Resuming,

    /// The application is running. (running Schedule, MainSchedule and FixedMainSchedule)
    Running,

    /// The application is suspending. (running the SuspendSchedule)
    Suspending,

    /// The application is suspended. (running the Schedule)
    Suspended,

    /// The application is stopping. (running the StopSchedule)
    Stopping(AppExit),

    /// The application has Stopped.
    Stopped(AppExit),
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum AppState {
    #[default]
    Created,
    Running,
    Suspended,
    Stopped(AppExit),
}

impl From<AppState> for AppLifecycle {
    fn from(state: AppState) -> Self {
        match state {
            AppState::Created => Self::Created,
            AppState::Running => Self::Running,
            AppState::Suspended => Self::Suspended,
            AppState::Stopped(exit) => Self::Stopped(exit),
        }
    }
}

impl AppLifecycle {
    /// Returns `true` if the lifecycle is exactly [`Running`](AppLifecycle::Running).
    #[inline]
    pub fn is_running(self) -> bool {
        matches!(self, Self::Running)
    }

    /// Returns `true` if the lifecycle is exactly [`Suspended`](AppLifecycle::Suspended).
    #[inline]
    pub fn is_suspended(self) -> bool {
        matches!(self, Self::Suspended)
    }

    /// Returns `true` if startup has completed (any state after `Created`).
    #[inline]
    pub fn is_started(self) -> bool {
        matches!(
            self,
            Self::Running | Self::Resuming | Self::Suspending | Self::Suspended
        )
    }

    /// Returns `true` if the lifecycle has reached [`Stopped`](AppLifecycle::Stopped).
    #[inline]
    pub fn is_stopped(self) -> bool {
        matches!(self, Self::Stopped(_))
    }
}

/// Drives the application lifecycle state machine and owns all schedule resource ids.
///
/// Obtained via [`FromResourcesMut`]; stores resource ids resolved at construction time
/// to avoid repeated lookups during each `update` call.
pub struct AppLifecycleController {
    state: AppState,
    lifecycle_id: ResourceId<AppLifecycle>,
    events_id: ResourceId<Events<AppLifecycle>>,
    exit_events_id: ResourceId<Events<AppExit>>,
    schedule_startup_id: ResourceId<StartupSchedule>,
    schedule_resume_id: ResourceId<ResumeSchedule>,
    schedule_id: ResourceId<Schedule>,
    schedule_main_id: ResourceId<MainSchedule>,
    schedule_suspend_id: ResourceId<SuspendSchedule>,
    schedule_stop_id: ResourceId<StopSchedule>,
}

impl FromResourcesMut for AppLifecycleController {
    fn from_resources_mut(res: &mut Resources) -> Self {
        res.install(AppModule);
        Self {
            state: AppState::Created,
            lifecycle_id: res.expect_id(),
            events_id: res.expect_id(),
            exit_events_id: res.expect_id(),
            schedule_startup_id: res.expect_id(),
            schedule_resume_id: res.expect_id(),
            schedule_id: res.expect_id(),
            schedule_main_id: res.expect_id(),
            schedule_suspend_id: res.expect_id(),
            schedule_stop_id: res.expect_id(),
        }
    }
}

impl AppLifecycleController {
    /// Returns the current lifecycle state as a public [`AppLifecycle`] enum value.
    pub fn lifecycle(&self) -> AppLifecycle {
        self.state.into()
    }

    /// Returns `true` if the application is in the `Running` state.
    #[inline]
    pub fn is_running(&self) -> bool {
        matches!(self.state, AppState::Running)
    }

    /// Returns `true` if the application has started (either running or suspended).
    #[inline]
    pub fn is_started(&self) -> bool {
        matches!(self.state, AppState::Running | AppState::Suspended)
    }

    fn change_lifecycle(&self, lifecycle: AppLifecycle, res: &mut Resources) {
        let var = res.get_mut_id(self.lifecycle_id).unwrap();
        if *var != lifecycle {
            log::info!("Changed application lifecycle to: {:?}", lifecycle);
            *var = lifecycle;
            res.get_mut_id(self.events_id).unwrap().send(lifecycle);
        }
    }

    /// Returns `Some(exit)` if the application should stop, either because it reached `Stopped` or an exit event was sent.
    pub fn should_exit(&self, res: &Resources) -> Option<AppExit> {
        if let AppState::Stopped(exit) = self.state {
            return Some(exit);
        }
        res.borrow_res_id(self.exit_events_id)?.last().copied()
    }

    /// Advances one tick: starts/resumes if needed, runs the appropriate schedules, and handles exit events.
    pub fn update(&mut self, res: &mut Resources) -> Option<AppExit> {
        if matches!(self.state, AppState::Created) {
            self.resume(res);
        }
        match self.state {
            AppState::Created => unreachable!(),
            AppState::Suspended => {
                self.change_lifecycle(AppLifecycle::Suspended, res);
                res.run_schedule_id(self.schedule_id);
            }
            AppState::Running => {
                self.change_lifecycle(AppLifecycle::Running, res);
                res.run_schedule_id(self.schedule_id);
                res.run_schedule_id(self.schedule_main_id);
            }
            AppState::Stopped(app_exit) => return Some(app_exit),
        }
        res.get_mut_id(self.exit_events_id)?
            .last()
            .copied()
            .map(|app_exit| self.stop(res, app_exit))
    }

    /// Transitions from `Created` to `Suspended` by running the startup schedule.
    /// Returns `false` if the application is not in the `Created` state.
    pub fn start(&mut self, res: &mut Resources) -> bool {
        if matches!(self.state, AppState::Created) {
            self.change_lifecycle(AppLifecycle::Starting, res);
            res.run_schedule_id(self.schedule_startup_id);
            self.state = AppState::Suspended;
            true
        } else {
            log::warn!("Cannot start application in state: {:?}", self.state);
            false
        }
    }

    /// Transitions from `Suspended` to `Running` by running the resume schedule.
    /// Returns `false` if the application cannot be resumed in its current state.
    pub fn resume(&mut self, res: &mut Resources) -> bool {
        if matches!(self.state, AppState::Created) {
            self.start(res);
        }
        if matches!(self.state, AppState::Suspended) {
            self.change_lifecycle(AppLifecycle::Resuming, res);
            res.run_schedule_id(self.schedule_resume_id);
            self.state = AppState::Running;
            true
        } else {
            log::warn!("Cannot resume application in state: {:?}", self.state);
            false
        }
    }

    /// Transitions from `Running` to `Suspended` by running the suspend schedule.
    /// Returns `false` if the application is not `Running`.
    pub fn suspend(&mut self, res: &mut Resources) -> bool {
        if matches!(self.state, AppState::Running) {
            self.change_lifecycle(AppLifecycle::Suspending, res);
            res.run_schedule_id(self.schedule_suspend_id);
            self.state = AppState::Suspended;
            true
        } else {
            log::warn!("Cannot suspend application in state: {:?}", self.state);
            false
        }
    }

    /// Suspends if running, then runs the stop schedule and transitions to `Stopped`.
    pub fn stop(&mut self, res: &mut Resources, app_exit: AppExit) -> AppExit {
        if matches!(self.state, AppState::Running) {
            self.suspend(res);
        }
        match self.state {
            AppState::Created => {
                self.change_lifecycle(AppLifecycle::Stopped(app_exit), res);
                self.state = AppState::Stopped(app_exit);
                app_exit
            }
            AppState::Running => unreachable!(),
            AppState::Suspended => {
                self.change_lifecycle(AppLifecycle::Stopping(app_exit), res);
                res.run_schedule_id(self.schedule_stop_id);
                self.state = AppState::Stopped(app_exit);
                self.change_lifecycle(AppLifecycle::Stopped(app_exit), res);
                app_exit
            }
            AppState::Stopped(app_exit) => app_exit,
        }
    }
}

impl AppModule {
    pub(crate) fn init_lifecycle(self, res: &mut Resources) {
        res.init::<AppLifecycle>();
        res.init_event::<AppLifecycle>();
        res.init_event::<AppExit>();
        res.init_unsend::<StartupSchedule>();
        res.init_unsend::<ResumeSchedule>();
        res.init_unsend::<Schedule>();
        res.init_unsend::<MainSchedule>();
        res.init_unsend::<SuspendSchedule>();
        res.init_unsend::<StopSchedule>();
    }
}
