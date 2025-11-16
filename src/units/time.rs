use serde::{Deserialize, Serialize};
use std::ops;

#[derive(Debug, Deserialize, Serialize, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimetableTime(pub i32);

impl TimetableTime {
    #[inline]
    pub fn from_hms<T: Into<i32>>(h: T, m: T, s: T) -> Self {
        TimetableTime(h.into() * 3600 + m.into() * 60 + s.into())
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
    #[inline]
    pub fn from_str(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        match parts.len() {
            2 => {
                let h = parts[0].parse::<i32>().ok()?;
                let m = parts[1].parse::<i32>().ok()?;
                Some(TimetableTime::from_hms(h, m, 0))
            }
            3 => {
                let h = parts[0].parse::<i32>().ok()?;
                let m = parts[1].parse::<i32>().ok()?;
                let sec = parts[2].parse::<i32>().ok()?;
                Some(TimetableTime::from_hms(h, m, sec))
            }
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

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub struct Duration(pub i32);

impl Duration {
    #[inline]
    pub fn to_hms(self) -> (i32, i32, i32) {
        let hours = self.0 / 3600;
        let minutes = (self.0 % 3600) / 60;
        let seconds = self.0 % 60;
        (hours, minutes, seconds)
    }
}

impl Duration {
    #[inline]
    pub fn from_str(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        match parts.len() {
            2 => {
                let h = parts[0].parse::<i32>().ok()?;
                let m = parts[1].parse::<i32>().ok()?;
                Some(Duration(h * 3600 + m * 60))
            }
            3 => {
                let h = parts[0].parse::<i32>().ok()?;
                let m = parts[1].parse::<i32>().ok()?;
                let sec = parts[2].parse::<i32>().ok()?;
                Some(Duration(h * 3600 + m * 60 + sec))
            }
            _ => None,
        }
    }
}

impl std::fmt::Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (h, m, s) = self.to_hms();
        write!(f, ">> {:02}:{:02}:{:02}", h, m, s)
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
