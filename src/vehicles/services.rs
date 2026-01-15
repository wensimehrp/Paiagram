use crate::colors::DisplayColor;
use bevy::prelude::*;
use egui::{Pos2, Shape, Stroke};
use moonshine_core::prelude::*;

#[derive(Reflect, Debug, Component, MapEntities)]
#[reflect(Component, MapEntities)]
#[require(Name)]
pub struct VehicleService {
    pub class: Option<Entity>,
}

#[derive(Debug, Clone, Copy)]
pub enum StrokeStyle {
    Filled {
        color: DisplayColor,
        width: f32,
    },
    Dotted {
        color: DisplayColor,
        radius: f32,
        spacing: f32,
    },
    Dashed {
        color: DisplayColor,
        length: f32,
        spacing: f32,
        width: f32,
    },
}

impl StrokeStyle {
    pub fn to_shape(self, light: bool, points: &[Pos2]) -> Vec<Shape> {
        match self {
            Self::Filled { color, width } => {
                vec![Shape::line(
                    points.to_vec(),
                    Stroke::new(width, color.get(light)),
                )]
            }
            Self::Dashed {
                color,
                length,
                spacing,
                width,
            } => Shape::dashed_line(
                points,
                Stroke::new(width, color.get(light)),
                length,
                spacing,
            ),
            Self::Dotted {
                color,
                radius,
                spacing,
            } => Shape::dotted_line(points, color.get(light), radius, spacing),
        }
    }
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self::Filled {
            color: DisplayColor::default(),
            width: 1.0,
        }
    }
}

#[derive(Debug, Component)]
#[require(Name)]
pub struct VehicleClass {
    pub stroke: StrokeStyle,
}

impl Default for VehicleClass {
    fn default() -> Self {
        Self {
            stroke: StrokeStyle::default(),
        }
    }
}
