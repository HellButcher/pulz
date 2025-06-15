use pulz_schedule::{
    event::Events,
    prelude::{FromResourcesMut, ResourceId, Resources, Schedule},
};
use tracing as log;

use crate::{AppExit, schedules::{MainSchedule, ResumeSchedule, StartupSchedule, StopSchedule, SuspendSchedule}};

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
    #[inline]
    pub fn is_running(self) -> bool {
        matches!(self, Self::Running)
    }

    #[inline]
    pub fn is_suspended(self) -> bool {
        matches!(self, Self::Suspended)
    }

    #[inline]
    pub fn is_started(self) -> bool {
        matches!(
            self,
            Self::Running | Self::Resuming | Self::Suspending | Self::Suspended
        )
    }

    #[inline]
    pub fn is_stopped(self) -> bool {
        matches!(self, Self::Stopped(_))
    }
}

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
        let lifecycle_id = res.init::<AppLifecycle>();
        let events_id = res.init::<Events<AppLifecycle>>();
        let exit_events_id = res.init::<Events<AppExit>>();
        let schedule_startup_id = res.init_unsend::<StartupSchedule>();
        let schedule_resume_id = res.init_unsend::<ResumeSchedule>();
        let schedule_id = res.init_unsend::<Schedule>();
        let schedule_main_id = res.init_unsend::<MainSchedule>();
        let schedule_suspend_id = res.init_unsend::<SuspendSchedule>();
        let schedule_stop_id = res.init_unsend::<StopSchedule>();
        Self {
            state: AppState::Created,
            lifecycle_id,
            events_id,
            exit_events_id,
            schedule_startup_id,
            schedule_resume_id,
            schedule_id,
            schedule_main_id,
            schedule_suspend_id,
            schedule_stop_id,
        }
    }
}

impl AppLifecycleController {
    pub fn lifecycle(&self) -> AppLifecycle {
        self.state.into()
    }

    #[inline]
    pub fn is_running(&self) -> bool {
        matches!(self.state, AppState::Running)
    }

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

    pub fn should_exit(&self, res: &Resources) -> Option<AppExit> {
        if let AppState::Stopped(exit) = self.state {
            return Some(exit);
        }
        res.borrow_res_id(self.exit_events_id)?.last().copied()
    }

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
        res.get_mut_id(self.exit_events_id)?.last().copied().map(|app_exit| self.stop(res, app_exit))
    }

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
