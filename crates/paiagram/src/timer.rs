use jiff::Zoned;
use paiagram_core::time::{Tick, TimetableTime};

/// Token used to unlock the timer.
pub(crate) struct TimerLockKey(());

pub(crate) struct GlobalTimer {
    /// Progress of the timer
    seconds: f64,
    locked: bool,
    /// Ignored when synched to real time.
    pub animation_speed: f32,
    pub animation_playing: bool,
    /// Whether to sync the timer with the current time.
    pub sync_to_real_time: bool,
}

impl GlobalTimer {
    pub fn new() -> Self {
        Self {
            seconds: TimetableTime::from_hms(8, 0, 0).0 as f64,
            locked: false,
            animation_speed: 1.0,
            animation_playing: false,
            sync_to_real_time: false,
        }
    }
}

impl GlobalTimer {
    /// Advance the timer by `delta` seconds of wall-clock time.
    ///
    /// When synchronized to real time the timer ignores `delta` and derives its
    /// value from the system clock directly. Otherwise it only advances while
    /// animation is playing, scaled by [`Self::animation_speed`].
    pub fn march(&mut self, delta: f64) {
        if self.locked {
            return;
        }
        if self.sync_to_real_time {
            self.seconds = Self::current_real_time_seconds();
            return;
        }
        if !self.animation_playing {
            return;
        }
        let speed = self.animation_speed as f64;
        let seconds_delta = delta * speed;
        self.seconds += seconds_delta
    }

    pub fn seconds(&self) -> f64 {
        self.seconds
    }

    pub fn update_seconds(&mut self, new_seconds: f64, _key: &TimerLockKey) {
        self.seconds = new_seconds
    }

    /// Acquire the lock and return a key used to release it.
    pub fn try_lock(&mut self) -> Option<TimerLockKey> {
        if self.locked {
            None
        } else {
            self.locked = true;
            Some(TimerLockKey(()))
        }
    }

    /// Release the lock associated with `key`.
    pub fn unlock(&mut self, _key: TimerLockKey) {
        debug_assert_eq!(self.locked, true);
        self.locked = false;
    }

    /// The current time of day as seconds in f64 since midnight.
    fn current_real_time_seconds() -> f64 {
        let now = Zoned::now();
        let time = now.datetime().time();
        time.hour() as f64 * 3600.0
            + time.minute() as f64 * 60.0
            + time.second() as f64
            + time.subsec_nanosecond() as f64 / 1e9
    }
}
