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
#[derive(Component, Debug)]
#[require(Name)]
pub struct DisplayedLine {
    stations: Vec<(Entity, f32)>,
    pub scale_mode: ScaleMode,
}

pub enum DisplayedLineError {
    InvalidIndex,
    SameStationAsNeighbor,
    AdjacentIntervals((Entity, Entity)),
}

impl DisplayedLine {
    pub fn new(stations: DisplayedLineType) -> Self {
        Self {
            stations,
            scale_mode: ScaleMode::default(),
        }
    }
    pub fn stations(&self) -> &DisplayedLineType {
        &self.stations
    }
    pub unsafe fn stations_mut(&mut self) -> &mut DisplayedLineType {
        &mut self.stations
    }
    pub fn insert(&mut self, index: usize, (station, height): (Entity, f32)) -> Result<(), DisplayedLineError> {
        // Two same intervals cannot be neighbours
        // an interval is defined by (prev_entity, this_entity)
        if index > self.stations.len() {
            return Err(DisplayedLineError::InvalidIndex);
        };
        let prev_prev = if index >= 2 {
            Some(self.stations[index - 2].0)
        } else {
            None
        };
        let prev = if index >= 1 {
            Some(self.stations[index - 1].0)
        } else {
            None
        };
        let next = self.stations.get(index).map(|(e, _)| *e);
        let next_next = self.stations.get(index + 1).map(|(e, _)| *e);
        if let Some(prev_prev) = prev_prev && prev_prev == station {
            return Err(DisplayedLineError::AdjacentIntervals((prev_prev, prev.unwrap())))
        };
        if let Some(next_next) = next_next && next_next == station {
            return Err(DisplayedLineError::AdjacentIntervals((next.unwrap(), next_next)))
        };
        if let Some(prev) = prev && prev == station {
            return Err(DisplayedLineError::SameStationAsNeighbor);
        };
        if let Some(next) = next && next == station {
            return Err(DisplayedLineError::SameStationAsNeighbor);
        };
        self.stations.insert(index, (station, height));
        Ok(())
    }
    pub fn push(&mut self, station: (Entity, f32)) -> Result<(), DisplayedLineError>{
        self.insert(self.stations.len(), station)
    }
}

#[derive(Component, Debug)]
#[require(Name)]
pub struct RulerLine(pub RulerLineType);

pub struct LinesPlugin;

impl Plugin for LinesPlugin {
    fn build(&self, app: &mut App) {}
}
