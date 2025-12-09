use crate::{
    intervals::StationCache,
    units::{canvas::CanvasLength, time::TimetableTime},
    vehicles::{
        AdjustTimetableEntry, TimetableAdjustment,
        entries::{TimetableEntry, VehicleSchedule},
    },
};
use bevy::prelude::*;

/// Displayed line type:
/// A list of (station entity, size of the interval on canvas in mm)
/// The first entry is the starting station, where the canvas distance is simply omitted.
/// Each entry afterwards represents the interval from the previous station to this station.
pub type DisplayedLineType = Vec<(Entity, f32)>;

pub type RulerLineType = Vec<(Entity, TimetableTime)>;

#[derive(Debug, Default)]
pub enum ScaleMode {
    Linear,
    #[default]
    Logarithmic,
    Uniform,
}

/// An imaginary (railway) line on the canvas, consisting of multiple segments.
#[derive(Component, Debug, Default)]
#[require(Name)]
pub struct DisplayedLine {
    pub stations: Vec<(Entity, f32)>,
    pub scale_mode: ScaleMode,
}

#[derive(Component, Debug)]
#[require(Name)]
pub struct RulerLine(pub RulerLineType);

pub struct LinesPlugin;

impl Plugin for LinesPlugin {
    fn build(&self, app: &mut App) {}
}
