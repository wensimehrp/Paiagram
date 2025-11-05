use crate::{
    basic::TimetableTime,
    interface::UiCommand,
    intervals::{Depot, Station},
    vehicle_set::VehicleSet,
    vehicles::{DepartureType, Schedule, TimetableEntry, Vehicle},
};
use bevy::ecs::{
    entity::Entity,
    hierarchy::{ChildOf, Children},
    message::MessageWriter,
    name::Name,
    query::With,
    system::{In, InMut, Query},
};

/// Display station times in Japanese style timetable
pub fn show_station_timetable(
    (InMut(ui), In((vehicle_set, station))): (InMut<egui::Ui>, In<(Entity, Entity)>),
    station_names: Query<(&Name, Option<&Depot>), With<Station>>,
    vehicle_sets: Query<&Children, With<VehicleSet>>,
    vehicles: Query<(Entity, &Name, &Schedule), With<Vehicle>>,
    timetable_entries: Query<(&TimetableEntry, &ChildOf)>,
    mut msg_open_tab: MessageWriter<UiCommand>,
) {
    // find all timetable entries that stops at the given station
    let stopping_vehicles = timetable_entries
        .iter()
        .filter_map(|(entry, parent_vehicle)| {
            if entry.station == station && !matches!(entry.departure, DepartureType::NonStop) {
                Some((entry, parent_vehicle.0))
            } else {
                None
            }
        });
}
