use bevy::prelude::Reflect;
use egui::emath;
use serde::{Deserialize, Serialize};
use std::ops;

/// A tick. Each tick is 10ms
#[derive(
    Reflect, Debug, Deserialize, Serialize, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct Tick(pub i64);

impl Tick {
    pub const ZERO: Self = Self(0);
    pub const TICKS_PER_SECOND: i64 = 100;
    pub const TICKS_PER_DAY: i64 = 24 * 3600 * Self::TICKS_PER_SECOND;

    pub fn to_timetable_time(self) -> TimetableTime {
        TimetableTime((self.0 / 100) as i32)
    }
    pub fn from_timetable_time(time: TimetableTime) -> Self {
        Tick(time.0 as i64 * 100)
    }
    pub fn as_seconds_f64(self) -> f64 {
        let ticks_per_second = Self::from_timetable_time(TimetableTime(1)).0 as f64;
        self.0 as f64 / ticks_per_second
    }

    #[inline]
    pub fn normalized_with(self, cycle: Tick) -> Self {
        if cycle.0 <= 0 {
            return self;
        }
        Self(self.0.rem_euclid(cycle.0))
    }

    #[inline]
    pub fn normalized(self) -> Self {
        self.normalized_with(Tick(Self::TICKS_PER_DAY))
    }
}

impl From<TimetableTime> for Tick {
    fn from(value: TimetableTime) -> Self {
        Self::from_timetable_time(value)
    }
}

impl Into<TimetableTime> for Tick {
    fn into(self) -> TimetableTime {
        self.to_timetable_time()
    }
}

impl From<f64> for Tick {
    fn from(value: f64) -> Self {
        Tick(value as i64)
    }
}

impl Into<f64> for Tick {
    fn into(self) -> f64 {
        self.0 as f64
    }
}

/// The timetable timepoint in seconds from midnight
#[derive(
    Reflect, Debug, Deserialize, Serialize, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct TimetableTime(pub i32);

impl TimetableTime {
    #[inline]
    pub fn as_duration(self) -> Duration {
        Duration(self.0)
    }
    #[inline]
    pub fn to_ticks(self) -> Tick {
        Tick::from_timetable_time(self)
    }
    #[inline]
    pub fn from_hms<T: Into<i32>>(h: T, m: T, s: T) -> Self {
        TimetableTime(h.into() * 3600 + m.into() * 60 + s.into())
    }
    #[inline]
    pub fn hour(&self) -> i32 {
        self.to_hmsd().0
    }
    #[inline]
    pub fn minute(&self) -> i32 {
        self.to_hmsd().1
    }
    #[inline]
    pub fn second(&self) -> i32 {
        self.to_hmsd().2
    }
    #[inline]
    pub fn day(&self) -> i32 {
        self.to_hmsd().3
    }
    #[inline]
    pub fn hours(&self) -> i32 {
        self.0 / 3600
    }
    #[inline]
    pub fn minutes(&self) -> i32 {
        self.0 / 60
    }
    #[inline]
    pub fn seconds(&self) -> i32 {
        self.0
    }
    #[inline]
    pub fn days(&self) -> i32 {
        self.0 / 86400
    }
    #[inline]
    pub fn to_hmsd(self) -> (i32, i32, i32, i32) {
        let days = self.0.div_euclid(24 * 3600);
        let seconds_of_day = self.0.rem_euclid(24 * 3600);

        let hours = seconds_of_day / 3600;
        let minutes = (seconds_of_day % 3600) / 60;
        let seconds = seconds_of_day % 60;

        (hours, minutes, seconds, days)
    }
    /// Parses a string in the following forms to [`TimetableTime`]:
    /// - HH:MM:SS
    /// - HH:MM
    /// - HH:MM:SS+D
    /// - HH:MM:SS-D
    /// - HH:MM+D
    /// - HH:MM-D
    #[inline]
    pub fn from_str(s: &str) -> Option<Self> {
        let (time_part, day_offset_seconds) = if let Some(idx) = s.rfind(['+', '-']) {
            let (time, offset_str) = s.split_at(idx);
            // offset_str is "+1" or "-1", parse handles the sign for us
            let days = offset_str.parse::<i32>().ok()?;
            (time, days * 86400)
        } else {
            (s, 0)
        };

        let mut parts = time_part.split(':');
        let h = parts.next()?.parse::<i32>().ok()?;
        let m = parts.next()?.parse::<i32>().ok()?;
        let s = parts
            .next()
            .map(|s| s.parse::<i32>().ok())
            .flatten()
            .unwrap_or(0);

        if parts.next().is_some() {
            return None;
        }

        Some(TimetableTime::from_hms(h, m, s + day_offset_seconds))
    }
    /// Parses strings in HMM, HHMM, HMMSS, HHMMSS
    /// and with or without +D or -D
    /// This format is commonly seen in Japanese timetables.
    /// The +/-D is an extension.
    #[inline]
    pub fn from_oud2_str(s: &str) -> Option<Self> {
        let (time_part, day_offset_seconds) = if let Some(idx) = s.rfind(['+', '-']) {
            let (time, offset_str) = s.split_at(idx);
            // offset_str is "+1" or "-1", parse handles the sign for us
            let days = offset_str.parse::<i32>().ok()?;
            (time, days * 86400)
        } else {
            (s, 0)
        };
        match time_part.len() {
            3 => {
                let h = time_part[0..1].parse::<i32>().ok()?;
                let m = time_part[1..3].parse::<i32>().ok()?;
                Some(TimetableTime::from_hms(h, m, day_offset_seconds))
            }
            4 => {
                let h = time_part[0..2].parse::<i32>().ok()?;
                let m = time_part[2..4].parse::<i32>().ok()?;
                Some(TimetableTime::from_hms(h, m, day_offset_seconds))
            }
            5 => {
                let h = time_part[0..1].parse::<i32>().ok()?;
                let m = time_part[1..3].parse::<i32>().ok()?;
                let s = time_part[3..5].parse::<i32>().ok()?;
                Some(TimetableTime::from_hms(h, m, s + day_offset_seconds))
            }
            6 => {
                let h = time_part[0..2].parse::<i32>().ok()?;
                let m = time_part[2..4].parse::<i32>().ok()?;
                let s = time_part[4..6].parse::<i32>().ok()?;
                Some(TimetableTime::from_hms(h, m, s + day_offset_seconds))
            }
            _ => None,
        }
    }
    /// Parses the current time to a oud2 formatted string and drop the date offset.
    #[inline]
    pub fn to_oud2_str(&self, show_seconds: bool) -> String {
        let (h, m, s, _) = self.to_hmsd();
        if show_seconds {
            format!("{:2}{:02}{:02}", h, m, s)
        } else {
            format!("{:2}{:02}", h, m)
        }
    }
    /// Return the normalized time that is in 24 hour range
    #[inline]
    pub fn normalized(self) -> Self {
        Self(self.0.rem_euclid(86400))
    }
    /// Return the normalized time that is always within 24 hours ahead of the
    /// current time
    #[inline]
    pub fn normalized_ahead(&self, other: TimetableTime) -> Self {
        let diff = other.0 - self.0;
        Self(self.0 + diff.rem_euclid(86400))
    }
}

impl emath::Numeric for TimetableTime {
    const INTEGRAL: bool = true;
    const MIN: Self = Self(i32::MIN);
    const MAX: Self = Self(i32::MAX);

    fn from_f64(num: f64) -> Self {
        Self(num as i32)
    }

    fn to_f64(self) -> f64 {
        self.0 as f64
    }
}

impl std::fmt::Display for TimetableTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let days = self.0.div_euclid(24 * 3600);
        let seconds_of_day = self.0.rem_euclid(24 * 3600);

        let hours = seconds_of_day / 3600;
        let minutes = (seconds_of_day % 3600) / 60;
        let seconds = seconds_of_day % 60;

        write!(f, "{:02}:{:02}:{:02}", hours, minutes, seconds)?;

        if days != 0 {
            let sign = if days > 0 { '+' } else { '-' };
            write!(f, "{}{}", sign, days.abs())?;
        }

        Ok(())
    }
}

impl ops::Sub<TimetableTime> for TimetableTime {
    type Output = Duration;
    fn sub(self, rhs: TimetableTime) -> Self::Output {
        Duration(self.0 - rhs.0)
    }
}

impl ops::Add<Duration> for TimetableTime {
    type Output = TimetableTime;
    fn add(self, rhs: Duration) -> Self::Output {
        TimetableTime(self.0 + rhs.0)
    }
}

impl ops::AddAssign<Duration> for TimetableTime {
    fn add_assign(&mut self, rhs: Duration) {
        self.0 += rhs.0
    }
}

impl ops::Sub<Duration> for TimetableTime {
    type Output = TimetableTime;
    fn sub(self, rhs: Duration) -> Self::Output {
        TimetableTime(self.0 - rhs.0)
    }
}

impl ops::SubAssign<Duration> for TimetableTime {
    fn sub_assign(&mut self, rhs: Duration) {
        self.0 -= rhs.0;
    }
}

/// A duration in seconds.
#[derive(
    Reflect, Debug, Default, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct Duration(pub i32);

impl Duration {
    pub const ZERO: Self = Self(0);
    pub const MAX: Self = Self(i32::MAX);
    #[inline]
    pub fn to_hms(self) -> (i32, i32, i32) {
        let hours = self.0 / 3600;
        let minutes = (self.0 % 3600) / 60;
        let seconds = self.0 % 60;
        (hours, minutes, seconds)
    }
}

impl emath::Numeric for Duration {
    const INTEGRAL: bool = true;
    const MIN: Self = Self(i32::MIN);
    const MAX: Self = Self(i32::MAX);

    fn from_f64(num: f64) -> Self {
        Self(num as i32)
    }

    fn to_f64(self) -> f64 {
        self.0 as f64
    }
}

impl std::iter::Sum for Duration {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut s = Duration::ZERO;
        for i in iter {
            s += i
        }
        s
    }
}

impl Duration {
    pub fn from_secs(s: i32) -> Self {
        Self(s)
    }
    pub fn from_hms(h: i32, m: i32, s: i32) -> Self {
        Self(h * 3600 + m * 60 + s)
    }
    /// Parses a [`Duration`] to HH:MM:SS, without the `->` arrow
    pub fn to_string_no_arrow(&self) -> String {
        TimetableTime(self.0).to_string()
    }
    #[inline]
    pub fn from_str(s: &str) -> Option<Self> {
        let time_parts = if let Some((_, rhs)) = s.rsplit_once('→') {
            rhs
        } else {
            s
        }
        .trim();
        Some(Self(TimetableTime::from_str(time_parts)?.0))
    }
}

impl std::fmt::Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "→ {}", self.to_string_no_arrow())
    }
}

impl ops::Add<Duration> for Duration {
    type Output = Duration;
    fn add(self, rhs: Duration) -> Self::Output {
        Duration(self.0 + rhs.0)
    }
}

impl ops::AddAssign<Duration> for Duration {
    fn add_assign(&mut self, rhs: Duration) {
        self.0 += rhs.0;
    }
}

impl ops::Sub<Duration> for Duration {
    type Output = Duration;
    fn sub(self, rhs: Duration) -> Self::Output {
        Duration(self.0 - rhs.0)
    }
}

impl ops::SubAssign<Duration> for Duration {
    fn sub_assign(&mut self, rhs: Duration) {
        self.0 -= rhs.0;
    }
}

impl ops::Add<TimetableTime> for Duration {
    type Output = TimetableTime;
    fn add(self, rhs: TimetableTime) -> Self::Output {
        TimetableTime(self.0 + rhs.0)
    }
}

impl ops::Div<i32> for Duration {
    type Output = Duration;
    fn div(self, rhs: i32) -> Self::Output {
        Duration(self.0 / rhs)
    }
}

impl ops::DivAssign<i32> for Duration {
    fn div_assign(&mut self, rhs: i32) {
        self.0 /= rhs;
    }
}

impl ops::Mul<i32> for Duration {
    type Output = Duration;
    fn mul(self, rhs: i32) -> Self::Output {
        Duration(self.0 * rhs)
    }
}

impl ops::MulAssign<i32> for Duration {
    fn mul_assign(&mut self, rhs: i32) {
        self.0 *= rhs;
    }
}

impl ops::Mul<Duration> for i32 {
    type Output = Duration;
    fn mul(self, rhs: Duration) -> Self::Output {
        Duration(self * rhs.0)
    }
}
