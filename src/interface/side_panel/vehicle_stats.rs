use bevy::ecs::{
    entity::Entity,
    name::Name,
    query::With,
    system::{In, InMut, Query},
};
use egui::Ui;

use crate::vehicles::{
    Vehicle,
    entries::{TimetableEntry, TimetableEntryCache, VehicleSchedule, VehicleScheduleCache},
};

pub fn show_vehicle_stats(
    (InMut(ui), In(vehicle_entity)): (InMut<Ui>, In<Entity>),
    vehicle_name: Query<&Name, With<Vehicle>>,
    schedule: Query<(&VehicleSchedule, &VehicleScheduleCache)>,
    timetable_entries: Query<(&TimetableEntry, &TimetableEntryCache)>,
) {
    // Display basic statistics and edit functions
    ui.heading(
        vehicle_name
            .get(vehicle_entity)
            .map_or("Unknown", Name::as_str),
    );
    // calculate the duration
    egui::Grid::new("vehicle_info_grid")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            ui.label("Duration:");
            let maybe_schedule = schedule.get(vehicle_entity).ok();
            let duration_text = {
                if let Some((_, cache)) = maybe_schedule
                    && let Some(actual_route) = cache.actual_route.as_ref()
                    && let Some(first) = actual_route.first()
                    && let Some(last) = actual_route.last()
                    && let Ok((_, first_cache)) = timetable_entries.get(first.inner())
                    && let Ok((_, last_cache)) = timetable_entries.get(last.inner())
                    && let Some(arrival) = first_cache.estimate.as_ref().map(|e| e.arrival)
                    && let Some(departure) = last_cache.estimate.as_ref().map(|e| e.departure)
                {
                    (departure - arrival).to_string()
                } else {
                    "Incomplete".to_string()
                }
            };
            ui.monospace(duration_text);
            ui.end_row();
        });
}
