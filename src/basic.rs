use bevy::prelude::*;
use derive_more::{Add, AddAssign, Sub, SubAssign};

/// Time in seconds from point of origin (e.g. midnight)
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
    Default,
    Reflect,
    Add,
    Sub,
    AddAssign,
    SubAssign,
)]
pub struct TimetableTime(pub i32);

impl TimetableTime {
    #[inline]
    pub fn from_hms<T: Into<i32>>(h: T, m: T, s: T) -> Self {
        TimetableTime(h.into() * 3600 + m.into() * 60 + s.into())
    }
    #[inline]
    pub fn from_str(s: &str) -> Option<Self> {
        let positive = s.contains("+");
        let parts: Vec<&str> = s.trim().split(|c| c == '+' || c == '-').collect();
        let time_part = parts.get(0)?;
        let time_parts: Vec<&str> = time_part.split(':').collect();
        let base_time = match time_parts.len() {
            2 => {
                let h = time_parts[0].parse::<i32>().ok()?;
                let m = time_parts[1].parse::<i32>().ok()?;
                Some(TimetableTime::from_hms(h, m, 0))
            }
            3 => {
                let h = time_parts[0].parse::<i32>().ok()?;
                let m = time_parts[1].parse::<i32>().ok()?;
                let s = time_parts[2].parse::<i32>().ok()?;
                Some(TimetableTime::from_hms(h, m, s))
            }
            _ => None,
        };
        match (base_time, parts.get(1)) {
            (Some(t), Some(day_str)) => {
                let day_offset: i32 = day_str.trim().parse().ok()?;
                Some(if positive {
                    t + TimetableTime(day_offset * 86400)
                } else {
                    t - TimetableTime(day_offset * 86400)
                })
            }
            (Some(t), None) => Some(t),
            _ => None,
        }
    }
    #[inline]
    pub fn from_oud2_str(s: &str) -> Option<Self> {
        match s.len() {
            3 => {
                let h = s[0..1].parse::<i32>().ok()?;
                let m = s[1..3].parse::<i32>().ok()?;
                Some(TimetableTime::from_hms(h, m, 0))
            }
            4 => {
                let h = s[0..2].parse::<i32>().ok()?;
                let m = s[2..4].parse::<i32>().ok()?;
                Some(TimetableTime::from_hms(h, m, 0))
            }
            5 => {
                let h = s[0..1].parse::<i32>().ok()?;
                let m = s[1..3].parse::<i32>().ok()?;
                let sec = s[3..5].parse::<i32>().ok()?;
                Some(TimetableTime::from_hms(h, m, sec))
            }
            6 => {
                let h = s[0..2].parse::<i32>().ok()?;
                let m = s[2..4].parse::<i32>().ok()?;
                let sec = s[4..6].parse::<i32>().ok()?;
                Some(TimetableTime::from_hms(h, m, sec))
            }
            _ => None,
        }
    }
    #[inline]
    pub fn to_hhmm_string_no_colon(self) -> String {
        let total_seconds = self.0;
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        if hours < 0 || minutes < 0 {
            warn!("Negative time encountered in to_hhmm_string_no_colon()");
        }
        format!("{:>2}{:02}", hours, minutes)
    }
    #[inline]
    pub fn to_mmss_string(self) -> String {
        let total_seconds = self.0;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        if minutes < 0 || seconds < 0 {
            warn!("Negative time encountered in to_mmss_string()");
        }
        format!("{:02}:{:02}", minutes, seconds)
    }
    // #[inline(always)]
    // pub fn to_hhmmssd_string(self) -> String {
    //     format!("{}", self)
    // }
    #[inline]
    pub fn to_hhmmss_string(self) -> String {
        let total_seconds = self.0;
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        if hours < 0 || minutes < 0 || seconds < 0 {
            warn!("Negative time encountered in to_hhmmss_string()");
        }
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }
    #[inline]
    pub fn to_hmsd_parts(self) -> (i32, i32, i32, i32) {
        let days = self.0.div_euclid(24 * 3600);
        let seconds_of_day = self.0.rem_euclid(24 * 3600);

        let hours = seconds_of_day / 3600;
        let minutes = (seconds_of_day % 3600) / 60;
        let seconds = seconds_of_day % 60;

        (hours, minutes, seconds, days)
    }
    #[inline]
    pub fn to_ms_parts(self) -> (i32, i32) {
        let total_seconds = self.0;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        (minutes, seconds)
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

/// Distance between two points on a real-world map, in metres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Default, Reflect)]
pub struct TrackDistance(i32);

impl TrackDistance {
    #[inline(always)]
    pub fn from_km<T: Into<f64>>(km: T) -> Self {
        TrackDistance((km.into() * 1000.0).round() as i32)
    }
    #[inline(always)]
    pub fn from_m<T: Into<i32>>(m: T) -> Self {
        TrackDistance(m.into())
    }
    // #[inline(always)]
    // pub fn from_mi<T: Into<f64>>(mi: T) -> Self {
    //     TrackDistance((mi.into() * 1609.344).round() as i32)
    // }
    #[inline(always)]
    pub fn as_m(&self) -> i32 {
        self.0
    }
}

impl std::fmt::Display for TrackDistance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 >= 1000 {
            write!(f, "{:.2} km", self.0 as f64 / 1000.0)
        } else {
            write!(f, "{} m", self.0)
        }
    }
}

/// Distance between two points on the canvas, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Reflect)]
pub struct CanvasDistance(f32);
impl CanvasDistance {
    #[inline(always)]
    pub fn from_mm<T: Into<f32>>(mm: T, factor: T) -> Self {
        CanvasDistance(mm.into() * factor.into())
    }
    // #[inline(always)]
    // pub fn from_cm<T: Into<f32>>(cm: T, factor: T) -> Self {
    //     CanvasDistance(cm.into() * 10.0 * factor.into())
    // }
    // #[inline(always)]
    // pub fn from_m<T: Into<f32>>(m: T, factor: T) -> Self {
    //     CanvasDistance(m.into() * 1000.0 * factor.into())
    // }
    #[inline(always)]
    pub fn as_mm(&self) -> f32 {
        self.0
    }
}

impl From<TrackDistance> for CanvasDistance {
    fn from(distance: TrackDistance) -> Self {
        CanvasDistance(distance.0 as f32)
    }
}

/// Speed in km/h
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Reflect)]
pub struct Speed(f32);
impl Speed {
    // #[inline(always)]
    // pub fn from_kmh<T: Into<f32>>(kmh: T) -> Self {
    //     Speed(kmh.into())
    // }
    // #[inline(always)]
    // pub fn from_ms<T: Into<f32>>(ms: T) -> Self {
    //     Speed(ms.into() * 3.6)
    // }
    // #[inline(always)]
    // pub fn from_mph<T: Into<f32>>(mph: T) -> Self {
    //     Speed(mph.into() * 1.609344)
    // }
}

impl std::fmt::Display for Speed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2} km/h", self.0)
    }
}

impl std::ops::Div<TimetableTime> for TrackDistance {
    type Output = Speed;

    fn div(self, rhs: TimetableTime) -> Self::Output {
        Speed(self.0 as f32 / 1000.0 / rhs.0 as f32 * 3600.0)
    }
}

#[derive(Component, Debug)]
pub struct Note {
    pub text: String,
    pub modified_at: i64,
}
