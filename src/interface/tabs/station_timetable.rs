use crate::{
    interface::UiCommand,
    intervals::{Depot, Station},
    vehicles::{
        Vehicle,
        entries::{TimetableEntry, TravelMode, VehicleSchedule},
        vehicle_set::VehicleSet,
    },
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
    vehicles: Query<(Entity, &Name, &VehicleSchedule), With<Vehicle>>,
    timetable_entries: Query<(&TimetableEntry, &ChildOf)>,
    mut msg_open_tab: MessageWriter<UiCommand>,
) {
    // find all timetable entries that stops at the given station
    let stopping_vehicles = timetable_entries
        .iter()
        .filter_map(|(entry, parent_vehicle)| {
            if entry.station == station && !matches!(entry.departure, None) {
                Some((entry, parent_vehicle.0))
            } else {
                None
            }
        });
    let columns: Vec<(&TimetableEntry, Entity)> = Vec::with_capacity(24);
    for veh in stopping_vehicles {}
}
