use crate::basic::TimetableTime;
use crate::vehicles::{
    AdjustTimetableEntry, ArrivalType, DepartureType, Schedule, TimetableAdjustment, TimetableEntry,
};
use bevy::prelude::*;
use egui::{Color32, Ui};
use egui_table::{CellInfo, HeaderCellInfo, Table, TableDelegate, columns::Column};

const COLUMN_NAMES: &[&str] = &[
    "Station", "Arri.", "Dept.", "Service", "Track", "Parent", "A.E.", "D.E.",
];

pub struct TableCache<'a> {
    station_names: Vec<Option<String>>,
    arrivals: Vec<(Entity, ArrivalType)>,
    departures: Vec<(Entity, DepartureType)>,
    arrival_estimates: Vec<Option<TimetableTime>>,
    departure_estimates: Vec<Option<TimetableTime>>,
    service_names: Vec<Option<String>>,
    track_names: Vec<Option<String>>,
    parent_names: Vec<Option<String>>,
    msg_sender: MessageWriter<'a, AdjustTimetableEntry>,
}

impl<'a> TableCache<'a> {
    fn new(
        vehicle_schedule: &Schedule,
        timetable_entries: &Query<(&TimetableEntry, &ChildOf)>,
        names: &Query<&Name>,
        msg_sender: MessageWriter<'a, AdjustTimetableEntry>,
    ) -> Self {
        let schedule_length = vehicle_schedule.1.len();
        let mut station_names = Vec::with_capacity(schedule_length);
        let mut arrivals = Vec::with_capacity(schedule_length);
        let mut departures = Vec::with_capacity(schedule_length);
        let mut service_names = Vec::with_capacity(schedule_length);
        let mut track_names = Vec::with_capacity(schedule_length);
        let mut parent_names = Vec::with_capacity(schedule_length);
        let mut arrival_estimates = Vec::with_capacity(schedule_length);
        let mut departure_estimates = Vec::with_capacity(schedule_length);
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
            let arrival_estimate = entry.arrival_estimate;
            let departure_estimate = entry.departure_estimate;
            station_names.push(station_name);
            arrivals.push((*timetable_entry_entity, arrival));
            departures.push((*timetable_entry_entity, departure));
            service_names.push(service_name);
            track_names.push(track_name);
            parent_names.push(parent_name);
            arrival_estimates.push(arrival_estimate);
            departure_estimates.push(departure_estimate);
        }
        Self {
            station_names,
            arrivals,
            departures,
            service_names,
            track_names,
            parent_names,
            arrival_estimates,
            departure_estimates,
            msg_sender,
        }
    }
}

impl<'a> TableDelegate for TableCache<'a> {
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
        if ui.rect_contains_pointer(ui.max_rect().expand2(ui.style().spacing.item_spacing)) {}
        egui::Frame::new().show(ui, |ui| match cell.col_nr {
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
                if ui.monospace(format!("{}", self.arrivals[i].1)).clicked() {
                    self.msg_sender.write(AdjustTimetableEntry {
                        entity: self.arrivals[i].0,
                        adjustment: TimetableAdjustment::SetArrivalType(ArrivalType::Flexible),
                    });
                };
            }
            2 => {
                if ui.monospace(format!("{}", self.departures[i].1)).clicked() {
                    self.msg_sender.write(AdjustTimetableEntry {
                        entity: self.arrivals[i].0,
                        adjustment: TimetableAdjustment::SetDepartureType(DepartureType::Duration(
                            TimetableTime(100),
                        )),
                    });
                };
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
            6 => {
                ui.monospace(
                    self.arrival_estimates[i]
                        .and_then(|v| Some(format!("{}", v)))
                        .unwrap_or("---".into()),
                );
            }
            7 => {
                ui.monospace(
                    self.departure_estimates[i]
                        .and_then(|v| Some(format!("{}", v)))
                        .unwrap_or("---".into()),
                );
            }
            _ => (),
        });
        ui.add_space(4.0);
    }
}

pub fn show_vehicle(
    (InMut(ui), In(entity)): (InMut<egui::Ui>, In<Entity>),
    schedules: Query<&Schedule>,
    timetable_entries: Query<(&TimetableEntry, &ChildOf)>,
    names: Query<&Name>,
    msg_sender: MessageWriter<AdjustTimetableEntry>,
) {
    let Ok(vehicle_schedule) = schedules.get(entity) else {
        ui.label("The vehicle does not exist.");
        return;
    };
    let mut current_table_cache =
        TableCache::new(vehicle_schedule, &timetable_entries, &names, msg_sender);
    Table::new()
        .num_rows(vehicle_schedule.1.len() as u64)
        .columns(
            (0..COLUMN_NAMES.len())
                .map(|v| match v {
                    0 => Column::new(100.0).resizable(true),
                    1 => Column::new(90.0).resizable(false),
                    2 => Column::new(90.0).resizable(false),
                    3 => Column::new(100.0).resizable(true),
                    4 => Column::new(100.0).resizable(true),
                    5 => Column::new(100.0).resizable(true),
                    6 => Column::new(90.0).resizable(false),
                    7 => Column::new(90.0).resizable(false),
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>(),
        )
        .num_sticky_cols(1)
        .show(ui, &mut current_table_cache);
}
