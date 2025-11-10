use crate::vehicles::{ArrivalType, DepartureType, Schedule, TimetableEntry};
use bevy::prelude::*;
use egui::{Color32, Style, TextBuffer, Ui};
use egui_table::{CellInfo, HeaderCellInfo, Table, TableDelegate, columns::Column};

const COLUMN_NAMES: &[&str] = &[
    "Station",
    "Arri.",
    "Dept.",
    "Service",
    "Track",
    "Parent",
];

pub struct TableCache {
    station_names: Vec<Option<String>>,
    arrivals: Vec<ArrivalType>,
    departures: Vec<DepartureType>,
    service_names: Vec<Option<String>>,
    track_names: Vec<Option<String>>,
    parent_names: Vec<Option<String>>,
}

impl TableCache {
    fn new(
        vehicle_schedule: &Schedule,
        timetable_entries: &Query<(&TimetableEntry, &ChildOf)>,
        names: &Query<&Name>,
    ) -> Self {
        let schedule_length = vehicle_schedule.1.len();
        let mut station_names = Vec::with_capacity(schedule_length);
        let mut arrivals = Vec::with_capacity(schedule_length);
        let mut departures = Vec::with_capacity(schedule_length);
        let mut service_names = Vec::with_capacity(schedule_length);
        let mut track_names = Vec::with_capacity(schedule_length);
        let mut parent_names = Vec::with_capacity(schedule_length);
        for timetable_entry_entity in vehicle_schedule.1.iter() {
            let Ok((entry, parent)) = timetable_entries.get(*timetable_entry_entity) else {
                continue;
            };
            let station_name = names
                .get(entry.station)
                .and_then(|s| Ok(s.to_string()))
                .ok();
            let parent_name = names.get(parent.0).and_then(|s| Ok(s.to_string())).ok();
            let service_name = entry
                .service
                .and_then(|e| names.get(e).and_then(|s| Ok(s.to_string())).ok());
            let track_name = entry
                .track
                .and_then(|e| names.get(e).and_then(|s| Ok(s.to_string())).ok());
            let arrival = entry.arrival;
            let departure = entry.departure;
            station_names.push(station_name);
            arrivals.push(arrival);
            departures.push(departure);
            service_names.push(service_name);
            track_names.push(track_name);
            parent_names.push(parent_name);
        }
        Self {
            station_names,
            arrivals,
            departures,
            service_names,
            track_names,
            parent_names,
        }
    }
}

impl TableDelegate for TableCache {
    fn header_cell_ui(&mut self, ui: &mut Ui, cell: &HeaderCellInfo) {
        ui.add_space(4.0);
        ui.style_mut().spacing.item_spacing.x = 4.0;
        ui.label(COLUMN_NAMES[cell.group_index]);
        ui.add_space(4.0);
    }
    fn cell_ui(&mut self, ui: &mut Ui, cell: &CellInfo) {
        let i = cell.row_nr as usize;
        ui.add_space(4.0);
        ui.style_mut().spacing.item_spacing.x = 4.0;
        match cell.col_nr {
            0 => {
                if ui.button("☰").clicked() {
                    info!("123");
                }
                if ui.button("ℹ").clicked() {
                    info!("456");
                }
                ui.label(
                    self.station_names[i]
                        .as_ref()
                        .and_then(|v| Some(v.as_str()))
                        .unwrap_or("---"),
                );
            }
            1 => {
                ui.monospace(format!("{}", self.arrivals[i]));
            }
            2 => {
                ui.monospace(format!("{}", self.departures[i]));
            }
            3 => {
                ui.label(
                    self.service_names[i]
                        .as_ref()
                        .and_then(|v| Some(v.as_str()))
                        .unwrap_or("---"),
                );
            }
            4 => {
                ui.label(
                    self.track_names[i]
                        .as_ref()
                        .and_then(|v| Some(v.as_str()))
                        .unwrap_or("---"),
                );
            }
            5 => {
                ui.label(
                    self.parent_names[i]
                        .as_ref()
                        .and_then(|v| Some(v.as_str()))
                        .unwrap_or("---"),
                );
            }
            _ => (),
        }
        ui.add_space(4.0);
    }
}

pub fn show_vehicle<'a>(
    (InMut(ui), In(entity)): (InMut<egui::Ui>, In<Entity>),
    time: Res<Time>,
    schedules: Query<&Schedule>,
    timetable_entries: Query<(&TimetableEntry, &ChildOf)>,
    names: Query<&Name>,
) {
    let Ok(vehicle_schedule) = schedules.get(entity) else {
        ui.label("The vehicle does not exist.");
        return;
    };
    let current_table_cache = &mut TableCache::new(vehicle_schedule, &timetable_entries, &names);
    Table::new()
        .num_rows(vehicle_schedule.1.len() as u64)
        .columns(
            (0..COLUMN_NAMES.len())
                .map(|v| Column::new(1.0))
                .collect::<Vec<_>>(),
        )
        .num_sticky_cols(1)
        .show(ui, current_table_cache);
}
