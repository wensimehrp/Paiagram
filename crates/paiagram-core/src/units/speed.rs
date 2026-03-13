use super::distance::Distance;
use super::time::Duration;
use bevy::prelude::Reflect;
use derive_more::{Add, AddAssign, Sub, SubAssign};
use std::ops;

/// The speed represented in m/s
#[derive(Reflect, Debug, Clone, Copy, Add, AddAssign, Sub, SubAssign)]
pub struct Velocity(pub f32);

impl std::fmt::Display for Velocity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}m/s", self.0)
    }
}

impl ops::Mul<f32> for Velocity {
    type Output = Velocity;
    fn mul(self, rhs: f32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl ops::MulAssign<f32> for Velocity {
    fn mul_assign(&mut self, rhs: f32) {
        self.0 *= rhs
    }
}

impl ops::Div<f32> for Velocity {
    type Output = Self;
    fn div(self, rhs: f32) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl ops::DivAssign<f32> for Velocity {
    fn div_assign(&mut self, rhs: f32) {
        self.0 /= rhs
    }
}

impl ops::Mul<Duration> for Velocity {
    type Output = Distance;
    fn mul(self, rhs: Duration) -> Self::Output {
        Distance((self.0 * rhs.0 as f32).round() as i32)
    }
}

impl ops::Div<Velocity> for Distance {
    type Output = Duration;
    fn div(self, rhs: Velocity) -> Self::Output {
        if rhs.0 == 0.0 {
            return Duration(0);
        }
        Duration((self.0 as f32 / rhs.0).round() as i32)
    }
}

impl ops::Div<Duration> for Distance {
    type Output = Velocity;
    fn div(self, rhs: Duration) -> Self::Output {
        if rhs.0 == 0 {
            return Velocity(0.0);
        }
        Velocity(self.0 as f32 / rhs.0 as f32)
    }
}
