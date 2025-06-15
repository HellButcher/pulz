use pulz_schedule::custom_schedule_type;

/// A schedule, that runs in AppLifecycle::Running and also AppLifecycle::Suspended.
type Schedule = pulz_schedule::schedule::Schedule;

custom_schedule_type! {
    /// A `Schedule` that runs once at startup, before any other schedules.  Coresponds to AppLifecycle::Startup
    pub struct StartupSchedule
}

custom_schedule_type! {
    /// A `Schedule` that runs after the application is resumed from a suspended state, or directly after startup.  Coresponds to AppLifecycle::Resuming
    pub struct ResumeSchedule
}

custom_schedule_type! {
    /// The main `Schedule` that runs each time during AppLifecycle::Running.
    pub struct MainSchedule
}

custom_schedule_type! {
    /// The render `Schedule` that runs during AppLifecycle::Running .when a redraw was requested and when windows are visible.
    pub struct RenderSchedule
}

custom_schedule_type! {
    /// A `Schedule` that runs before the application is suspended. Coresponds to AppLifecycle::Suspending.
    pub struct SuspendSchedule
}

custom_schedule_type! {
    /// A `Schedule` that runs before the application is stopped. Coresponds to AppLifecycle::Stopping.
    pub struct StopSchedule
}
