use bevy::prelude::Reflect;
use derive_more::{Add, AddAssign, Sub, SubAssign};
#[derive(Reflect, Debug, Clone, Copy, Add, AddAssign, Sub, SubAssign)]
pub struct Distance(pub i32);

impl Distance {
    #[inline]
    pub fn from_km(km: f32) -> Self {
        Distance((km * 1000.0).round() as i32)
    }
    #[inline]
    pub fn from_m(m: i32) -> Self {
        Distance(m)
    }
}

impl std::fmt::Display for Distance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 <= 1000 {
            write!(f, "{}m", self.0)
        } else {
            write!(f, "{}.{:03}km", self.0 / 1000, self.0 % 1000)
        }
    }
}

impl std::ops::Mul<i32> for Distance {
    type Output = Self;
    fn mul(self, rhs: i32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl std::ops::MulAssign<i32> for Distance {
    fn mul_assign(&mut self, rhs: i32) {
        self.0 *= rhs;
    }
}

impl std::ops::Mul<f32> for Distance {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        Self((self.0 as f32 * rhs).round() as i32)
    }
}

impl std::ops::MulAssign<f32> for Distance {
    fn mul_assign(&mut self, rhs: f32) {
        self.0 = (self.0 as f32 * rhs).round() as i32;
    }
}

impl std::ops::Div<i32> for Distance {
    type Output = Self;
    fn div(self, rhs: i32) -> Self::Output {
        Self(self.0 / rhs)
    }
}

impl std::ops::DivAssign<i32> for Distance {
    fn div_assign(&mut self, rhs: i32) {
        self.0 /= rhs
    }
}

impl std::ops::Div<f32> for Distance {
    type Output = Self;
    fn div(self, rhs: f32) -> Self::Output {
        Self((self.0 as f32 / rhs).round() as i32)
    }
}

impl std::ops::DivAssign<f32> for Distance {
    fn div_assign(&mut self, rhs: f32) {
        self.0 = (self.0 as f32 / rhs).round() as i32;
    }
}
