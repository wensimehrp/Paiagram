// SPDX-License-Identifier: MPL-2.0
#![doc = include_str!("units/README.md")]

pub mod coordinates;
pub mod distance;
pub mod speed;
pub mod time;

pub use coordinates::*;
pub use distance::*;
use serde::{Deserialize, Serialize};

/// The canvas' length in millimetres
#[derive(Default, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct CanvasLength(pub f64);

impl From<f64> for CanvasLength {
    fn from(value: f64) -> Self {
        CanvasLength(value)
    }
}

impl From<CanvasLength> for f64 {
    fn from(value: CanvasLength) -> Self {
        value.0
    }
}

impl CanvasLength {
    const EGUI_POINTS_PER_IN: f64 = 96.0;

    /// How many egui points make up one millimetre.
    pub fn egui_pts_per_mm() -> f64 {
        Self::EGUI_POINTS_PER_IN / 25.4
    }
    pub fn from_mm(v: f64) -> Self {
        CanvasLength(v)
    }
    pub fn from_cm(v: f64) -> Self {
        CanvasLength(v * 10.0)
    }
    pub fn from_in(v: f64) -> Self {
        CanvasLength(v * 25.4)
    }
    pub fn from_postscript_pts(v: f64) -> Self {
        CanvasLength(v * 25.4 / 72.0)
    }
    pub fn from_egui_pts(v: impl Into<f64>) -> Self {
        CanvasLength(v.into() / Self::egui_pts_per_mm())
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
    pub fn to_postscript_pts(&self) -> f64 {
        self.0 * 72.0 / 25.4
    }
    pub fn to_egui_pts(&self) -> f32 {
        (self.0 * Self::egui_pts_per_mm()) as f32
    }
    pub fn to_egui_pts_f64(&self) -> f64 {
        self.0 * Self::egui_pts_per_mm()
    }
}
