use crate::units::time::{Duration, TimetableTime};
use bevy::prelude::*;

#[derive(Debug, Clone, Copy)]
pub enum TravelMode {
    At(TimetableTime),
    For(Duration),
    Flexible,
}

impl TravelMode {
    pub fn adjust_time(&mut self, adjustment: Duration) {
        match self {
            TravelMode::At(t) => {
                t.0 += adjustment.0;
            }
            TravelMode::For(dur) => {
                dur.0 += adjustment.0;
            }
            TravelMode::Flexible => (),
        }
    }
}

impl std::fmt::Display for TravelMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::At(t) => write!(f, "{}", t),
            Self::For(dur) => write!(f, "{}", dur),
            Self::Flexible => write!(f, ">>"),
        }
    }
}

/// An entry in the timetable
#[derive(Debug, Component)]
pub struct TimetableEntry {
    pub arrival: TravelMode,
    pub departure: Option<TravelMode>,
    pub arrival_estimate: Option<TimetableTime>,
    pub departure_estimate: Option<TimetableTime>,
    pub station: Entity,
    pub service: Option<Entity>,
    pub track: Option<Entity>,
}

#[derive(Debug, Component, Default)]
pub struct VehicleSchedule {
    pub start: TimetableTime,
    pub repeat: Option<Duration>,
    pub times: Vec<Duration>,
    pub entities: Vec<Entity>,
}
