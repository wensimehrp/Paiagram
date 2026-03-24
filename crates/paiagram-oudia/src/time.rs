use derive_more::{Add, AddAssign, Sub, SubAssign};
use std::{
    fmt::{Debug, Display},
    num::ParseIntError,
};

use crate::ast::SerializeToOud;

/// A Time
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Add, AddAssign, Sub, SubAssign, Hash)]
pub struct Time(i32);

impl Debug for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:02}:{:02}:{:02}",
            self.hour(),
            self.minute(),
            self.second()
        )
    }
}

impl Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:02}:{:02}:{:02}",
            self.hour(),
            self.minute(),
            self.second()
        )
    }
}

impl std::str::FromStr for Time {
    type Err = ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_oud_str(s)
    }
}

impl Time {
    pub fn hour(self) -> i32 {
        self.0 / 3600
    }
    pub fn minute(self) -> i32 {
        (self.0 % 3600) / 60
    }
    pub fn second(self) -> i32 {
        self.0 % 60
    }
    pub fn seconds(self) -> i32 {
        self.0
    }
}

impl SerializeToOud for Time {
    fn serialize_oud_to(&self, buf: &mut impl std::io::Write) -> std::io::Result<()> {
        buf.write_all(Time::to_oud_string(*self).as_bytes())?;
        Ok(())
    }
}

impl Time {
    pub fn to_oud_string(self) -> String {
        let s = self.second();
        if s == 0 {
            format!("{}{:02}", self.hour(), self.minute())
        } else {
            format!("{}{:02}{:02}", self.hour(), self.minute(), s)
        }
    }
    pub fn from_oud_str(s: &str) -> Result<Self, ParseIntError> {
        match s.len() {
            3 => {
                let h = s[0..1].parse::<i32>()?;
                let m = s[1..3].parse::<i32>()?;
                Ok(Time(h * 3600 + m * 60))
            }
            4 => {
                let h = s[0..2].parse::<i32>()?;
                let m = s[2..4].parse::<i32>()?;
                Ok(Time(h * 3600 + m * 60))
            }
            5 => {
                let h = s[0..1].parse::<i32>()?;
                let m = s[1..3].parse::<i32>()?;
                let sec = s[3..5].parse::<i32>()?;
                Ok(Time(h * 3600 + m * 60 + sec))
            }
            6 => {
                let h = s[0..2].parse::<i32>()?;
                let m = s[2..4].parse::<i32>()?;
                let sec = s[4..6].parse::<i32>()?;
                Ok(Time(h * 3600 + m * 60 + sec))
            }
            _ => "invalid".parse::<i32>().map(Time),
        }
    }
}
