use crate::{
    interface::{AppTab, UiCommand},
    intervals::{Depot, Station},
    units::time::TimetableTime,
    vehicles::{
        AdjustTimetableEntry, TimetableAdjustment, Vehicle,
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
    mut msg_passthrough: MessageWriter<AdjustTimetableEntry>,
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
    let mut rows: Vec<Vec<(TimetableTime, &TimetableEntry, Entity)>> = vec![Vec::new(); 24];
    for (entry, entity) in stopping_vehicles {
        let Some(departure_estimate) = entry.departure_estimate else {
            continue;
        };
        let (hour, ..) = departure_estimate.to_hmsd();
        rows[hour as usize].push((departure_estimate, entry, entity));
    }
    for row in &mut rows {
        row.sort_by_key(|(time, _, _)| *time);
    }
    if ui.button("refresh").clicked() {
        for (_, _, schedule) in &vehicles {
            msg_passthrough.write(AdjustTimetableEntry {
                entity: schedule.entities[0],
                adjustment: TimetableAdjustment::PassThrough,
            });
        }
    }
    for (hour, row) in rows.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("{:02}:00", hour));
            for (time, entry, parent_vehicle) in row {
                if ui.button("ℹ").clicked() {
                    msg_open_tab.write(UiCommand::OpenOrFocusTab(AppTab::Vehicle(*parent_vehicle)));
                }
                ui.monospace({
                    if let Some(departure_estimate) = entry.departure_estimate {
                        let (h, m, ..) = departure_estimate.to_hmsd();
                        format!("{:02}{:02}", h, m)
                    } else {
                        "----".to_string()
                    }
                });
            }
        });
    }
}
