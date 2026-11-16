use std::debug_assert_matches;

use jiff::ZonedDateTime;
use paiagram_core::time::TimetableTime;

/// Token used to unlock the timer.
#[derive(Debug)]
pub(crate) struct TimerLockKey(());

pub(crate) struct GlobalTimer {
    /// Progress of the timer
    seconds: f64,
    key: Option<TimerLockKey>,
    /// Ignored when synced to real time.
    pub animation_speed: f32,
    pub animation_playing: bool,
    /// Whether to sync the timer with the current time.
    pub sync_to_real_time: bool,
}

impl GlobalTimer {
    pub fn new() -> Self {
        Self {
            seconds: TimetableTime::from_hms(8, 0, 0).0 as f64,
            key: Some(TimerLockKey(())),
            animation_speed: 1.0,
            animation_playing: false,
            sync_to_real_time: false,
        }
    }
}

impl GlobalTimer {
    pub fn seconds(&self) -> f64 {
        self.seconds
    }

    pub fn update_seconds(&mut self, new_seconds: f64, _key: &TimerLockKey) {
        self.seconds = new_seconds
    }

    /// Acquire the lock and return a key used to release it.
    pub fn try_lock(&mut self) -> Option<TimerLockKey> {
        self.key.take()
    }

    /// Release the lock associated with `key`.
    pub fn unlock(&mut self, key: TimerLockKey) {
        debug_assert_matches!(self.key, None);
        self.key = Some(key);
    }

    /// The current time of day as seconds in f64 since midnight.
    pub fn current_real_time_seconds() -> f64 {
        let now = ZonedDateTime::now();
        let time = now.datetime().time();
        time.hour() as f64 * 3600.0
            + time.minute() as f64 * 60.0
            + time.second() as f64
            + time.subsec_nanosecond() as f64 / 1e9
    }
}
