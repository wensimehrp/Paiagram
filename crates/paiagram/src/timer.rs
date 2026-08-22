use jiff::Zoned;
use paiagram_core::time::{Tick, TimetableTime};

/// Token used to unlock the timer.
pub(crate) struct TimerLockKey {
    _marker: (),
}

pub(crate) struct GlobalTimer {
    /// Progress of the timer
    ticks: Tick,
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
            ticks: TimetableTime::from_hms(8, 0, 0).to_ticks(),
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
            self.ticks = Self::current_real_time_ticks();
            return;
        }
        if !self.animation_playing {
            return;
        }
        let speed = self.animation_speed as f64;
        let tick_delta = delta * speed * Tick::TICKS_PER_SECOND as f64;
        self.ticks = Tick(self.ticks.0 + tick_delta.round() as i64);
    }

    pub fn ticks(&self) -> Tick {
        self.ticks
    }

    pub fn ticks_mut(&mut self, _key: &TimerLockKey) -> &mut Tick {
        &mut self.ticks
    }

    /// Acquire the lock and return a key used to release it.
    pub fn try_lock(&mut self) -> Option<TimerLockKey> {
        if self.locked {
            None
        } else {
            self.locked = true;
            Some(TimerLockKey { _marker: () })
        }
    }

    /// Release the lock associated with `key`.
    pub fn unlock(&mut self, _key: TimerLockKey) {
        debug_assert_eq!(self.locked, true);
        self.locked = false;
    }

    /// The current time of day as [`Tick`]s since midnight.
    fn current_real_time_ticks() -> Tick {
        let now = Zoned::now();
        let time = now.datetime().time();
        Tick(
            time.hour() as i64 * 3600 * Tick::TICKS_PER_SECOND
                + time.minute() as i64 * 60 * Tick::TICKS_PER_SECOND
                + time.second() as i64 * Tick::TICKS_PER_SECOND
                + time.subsec_nanosecond() as i64 / 10_000_000,
        )
    }
}
