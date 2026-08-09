//! Time tracking resources: wall-clock, virtual, and fixed-step time.
//!
//! [`RealTime`] measures actual elapsed time; [`VirtTime`] applies a speed factor and supports
//! pausing; [`FixedTime`] accumulates virtual delta and steps at a fixed interval.
//! A system updates all three each frame and stores a snapshot in the shared [`Time`] resource.

use std::time::{Duration, Instant};

use pulz_schedule::{
    custom_schedule_type,
    prelude::{ResourceId, Resources},
    system::{ExclusiveSystem, System, SystemInit, system},
};

use crate::{AppModule, schedules::MainSchedule};

/// Snapshot of elapsed and per-frame delta time, stored as nanosecond integers plus an `f64` second delta.
#[derive(Debug, Clone, Copy)]
pub struct Time {
    /// Total elapsed time in nanoseconds since this timer was created or last cleared.
    pub duration_nanos: u64,
    /// Duration of the last frame in nanoseconds.
    pub delta_nanos: u64,
    /// Duration of the last frame in seconds (`f64` for use in physics/animation).
    pub delta_sec: f64,
}

/// Wall-clock time tracker that measures real elapsed time using [`Instant`].
#[derive(Debug, Clone)]
pub struct RealTime {
    /// The current time snapshot.
    pub time: Time,
    last_update: Option<Instant>,
}

/// Virtual (game) time that can be paused or scaled relative to real time.
pub struct VirtTime {
    /// The current virtual time snapshot.
    pub time: Time,
    speed_factor: f64,
    paused: bool,
}

/// Fixed-step time accumulator for deterministic simulation updates.
///
/// Call [`accumulate`](FixedTime::accumulate) with the virtual delta, then call [`step`](FixedTime::step)
/// in a loop until it returns `false` to consume full fixed intervals.
pub struct FixedTime {
    /// The current fixed time snapshot, advanced by `step_nanos` each step.
    pub time: Time,
    step_nanos: u64,
    remaining_nanos: u64,
}

impl Time {
    /// Creates a zeroed-out time snapshot.
    pub const fn new() -> Self {
        Self {
            duration_nanos: 0,
            delta_nanos: 0,
            delta_sec: 0.0,
        }
    }

    fn update(&mut self, delta: Duration) {
        self.delta_nanos = delta.as_nanos() as u64;
        self.delta_sec = delta.as_secs_f64();
        self.duration_nanos = self.duration_nanos.wrapping_add(self.delta_nanos);
    }

    /// Returns the total elapsed time as a [`Duration`].
    #[inline]
    pub fn duration(&self) -> Duration {
        Duration::from_nanos(self.duration_nanos)
    }

    /// Returns the per-frame delta time as a [`Duration`].
    #[inline]
    pub fn delta(&self) -> Duration {
        Duration::from_nanos(self.delta_nanos)
    }
}

impl Default for Time {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl RealTime {
    /// Creates a new `RealTime` with no prior measurement.
    pub const fn new() -> Self {
        Self {
            time: Time::new(),
            last_update: None,
        }
    }

    /// Measures the time since the last call and advances the snapshot. Returns the delta.
    #[inline]
    pub fn update(&mut self) -> Duration {
        let now = Instant::now();
        self.update_with(now)
    }

    /// Like [`update`](Self::update) but accepts an externally provided `now` timestamp.
    pub fn update_with(&mut self, now: Instant) -> Duration {
        let delta = if let Some(last_update) = self.last_update {
            let delta = now.saturating_duration_since(last_update);
            self.time.update(delta);
            delta
        } else {
            Duration::ZERO
        };
        self.last_update = Some(now);
        delta
    }
}

impl Default for RealTime {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for RealTime {
    type Target = Time;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.time
    }
}

impl VirtTime {
    /// Creates a new `VirtTime` running at 1× speed.
    pub fn new() -> Self {
        Self {
            time: Time::new(),
            speed_factor: 1.0,
            paused: false,
        }
    }

    /// Scales `real_delta` by the speed factor, advances the snapshot, and returns the virtual delta.
    pub fn update(&mut self, real_delta: Duration) -> Duration {
        if !self.paused && self.speed_factor != 0.0 && !real_delta.is_zero() {
            let scaled_duration = if self.speed_factor == 1.0 {
                real_delta
            } else {
                real_delta.mul_f64(self.speed_factor)
            };
            self.time.update(scaled_duration);
            scaled_duration
        } else {
            self.time.delta_nanos = 0;
            self.time.delta_sec = 0.0;
            Duration::ZERO
        }
    }

    /// Pauses virtual time; subsequent `update` calls return zero delta.
    #[inline]
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resumes virtual time after a pause.
    #[inline]
    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Returns `true` if virtual time is currently paused.
    #[inline]
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Sets the speed multiplier applied to real-time deltas.
    pub fn set_speed_factor(&mut self, factor: f64) {
        self.speed_factor = factor;
    }
}

impl Default for VirtTime {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for VirtTime {
    type Target = Time;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.time
    }
}

impl FixedTime {
    /// Default step size in nanoseconds (~61 Hz, chosen as a prime to avoid resonance with common refresh rates).
    pub const DEFAULT_STEP: u64 = 16_393_453;

    /// Creates a `FixedTime` with the given step size in nanoseconds.
    pub const fn new(step_nanos: u64) -> Self {
        Self {
            time: Time::new(),
            step_nanos,
            remaining_nanos: 0,
        }
    }

    /// Adds `virt_delta` to the remaining-time accumulator.
    pub fn accumulate(&mut self, virt_delta: Duration) {
        self.remaining_nanos = self
            .remaining_nanos
            .saturating_add(virt_delta.as_nanos() as u64);
    }

    /// Consumes one step from the accumulator and advances the time snapshot.
    /// Returns `false` when the accumulator has less than one full step remaining.
    pub fn step(&mut self) -> bool {
        let step_nanos = self.step_nanos;
        let Some(new_remaining) = self.remaining_nanos.checked_sub(step_nanos) else {
            return false;
        };
        self.remaining_nanos = new_remaining;
        self.time.update(Duration::from_nanos(step_nanos));
        true
    }
}

impl Default for FixedTime {
    #[inline]
    fn default() -> Self {
        Self::new(Self::DEFAULT_STEP)
    }
}

impl std::ops::Deref for FixedTime {
    type Target = Time;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.time
    }
}

#[system]
fn update_time(
    real_time: &mut RealTime,
    virt_time: &mut VirtTime,
    fixed_time: &mut FixedTime,
    time: &mut Time,
) {
    let real_delta = real_time.update();
    let virt_delta = virt_time.update(real_delta);
    *time = virt_time.time;
    fixed_time.accumulate(virt_delta);
}

custom_schedule_type! {
    /// A `Schedule` that is part of the main `Schedule` that runs at a fixed interval, regardless of the frame rate. It may run zero or multiple times per frame.
    pub struct FixedMainSchedule
}

/// An exclusive system that drives the [`FixedMainSchedule`] at a fixed timestep.
#[derive(Clone, Copy)]
pub struct FixedMainSystem(ResourceId<FixedTime>, ResourceId<FixedMainSchedule>);

impl SystemInit for FixedMainSystem {
    #[inline]
    fn init(&mut self, resources: &mut Resources) {
        resources.take_id_and(self.1, |schedule, res| schedule.init(res));
    }
}

impl ExclusiveSystem for FixedMainSystem {
    #[inline]
    fn run_exclusive(&mut self, resources: &mut Resources) {
        while resources
            .get_mut_id(self.0)
            .map(FixedTime::step)
            .unwrap_or(false)
        {
            resources.run_schedule_id(self.1);
        }
    }
}

impl AppModule {
    pub(crate) fn init_time(self, res: &mut Resources) {
        res.init::<RealTime>();
        res.init::<VirtTime>();
        let fixed_time_id = res.init::<FixedTime>();
        res.init::<Time>();
        let main_schedule_id = res.expect_id::<MainSchedule>();
        let fixed_main_schedule_id = res.init_unsend::<FixedMainSchedule>();

        let schedule = res.get_mut_id(main_schedule_id).unwrap();
        let update_time = schedule.add_system(System!(update_time)).as_label();
        schedule
            .add_system_exclusive(FixedMainSystem(fixed_time_id, fixed_main_schedule_id))
            .after(update_time);
    }
}
