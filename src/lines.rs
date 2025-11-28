use crate::{
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
    pub children: Option<Vec<Entity>>,
    pub scale_mode: ScaleMode,
}

#[derive(Component, Debug)]
#[require(Name)]
pub struct RulerLine(pub RulerLineType);

pub struct LinesPlugin;

impl Plugin for LinesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedPostUpdate, prepare_displayed_line);
    }
}

fn prepare_displayed_line(
    mut lines: Populated<&mut DisplayedLine>,
    mut msg_changed: MessageReader<AdjustTimetableEntry>,
    timetable_entries: Query<&TimetableEntry>,
    vehicles: Populated<(Entity, &VehicleSchedule)>,
) {
    for mut line in lines.iter_mut().filter(|p| p.children.is_none()) {
        let mut stations = line.stations.iter().map(|(e, _)| *e).collect::<Vec<_>>();
        stations.sort_unstable();
        let mut children = Vec::new();
        for (vehicle_entity, vehicle) in vehicles.iter() {
            for (entry, entity) in vehicle.into_entries(&timetable_entries) {
                if let Ok(_) = stations.binary_search(&entry.station) {
                    children.push(vehicle_entity);
                    break;
                }
            }
        }
        line.children = Some(children);
    }
}
