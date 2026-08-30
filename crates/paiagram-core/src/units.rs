pub mod coordinates;
pub mod distance;
pub mod speed;
pub mod time;

pub use coordinates::*;
pub use distance::*;
use serde::{Deserialize, Serialize};

/// The canvas' length in millimetres
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct CanvasLength(pub f64);

impl CanvasLength {
    pub fn from_mm(v: f64) -> Self {
        CanvasLength(v)
    }
    pub fn from_cm(v: f64) -> Self {
        CanvasLength(v * 10.0)
    }
    pub fn from_in(v: f64) -> Self {
        CanvasLength(v * 25.4)
    }
    /// Uses postscript points
    pub fn from_pts(v: f64) -> Self {
        CanvasLength(v * 25.4 / 72.0)
    }
    pub fn to_mm(&self) -> f64 {
        self.0
    }
    pub fn to_cm(&self) -> f64 {
        self.0 / 10.0
    }
    pub fn to_in(&self) -> f64 {
        self.0 / 25.4
    }
    /// Uses postscript points
    pub fn to_pts(&self) -> f64 {
        self.0 * 72.0 / 25.4
    }
}
