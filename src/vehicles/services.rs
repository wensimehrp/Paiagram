use bevy::prelude::*;
use egui::{Color32, Stroke};
use serde::{Deserialize, Serialize};

#[derive(Debug, Component)]
#[require(Name)]
pub struct VehicleService {
    pub class: Option<Entity>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum StrokeStyle {
    Filled,
    Dotted { spacing: f32, radius: f32 },
    Dashed { dash_length: f32, gap_length: f32 },
}

#[derive(Debug, Component, Serialize, Deserialize)]
#[require(Name)]
pub struct VehicleClass {
    pub stroke: (Stroke, StrokeStyle),
}

impl Default for VehicleClass {
    fn default() -> Self {
        Self {
            stroke: (
                Stroke {
                    width: 1.0,
                    color: Color32::GRAY,
                },
                StrokeStyle::Filled,
            ),
        }
    }
}
