use std::fmt::format;

use crate::{
    interface::{AppTab, UiCommand, tabs::vehicle},
    intervals::{Depot, Station},
    units::time::TimetableTime,
    vehicles::{
        AdjustTimetableEntry, TimetableAdjustment, Vehicle,
        entries::{TimetableEntry, TravelMode, VehicleSchedule},
        vehicle_set::VehicleSet,
    },
};
use bevy::{
    ecs::{
        entity::Entity,
        hierarchy::{ChildOf, Children},
        message::{MessageReader, MessageWriter},
        name::Name,
        query::With,
        system::{In, InMut, Local, Query},
    },
    log::info,
};
use egui::RichText;
use egui_table::{Column, Table, TableDelegate};

struct TableCache<'a> {
    msg_open_ui: MessageWriter<'a, UiCommand>,
    times: Vec<Vec<(String, String, TimetableTime, Entity)>>,
}

impl TableDelegate for TableCache<'_> {
    fn default_row_height(&self) -> f32 {
        20.0
    }
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
        ui.label(["Time", "Details"][cell.group_index]);
    }
    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
        let i = cell.row_nr as usize;
        match cell.col_nr {
            0 => {
                ui.monospace(format!("{:02}:00", i));
            }
            1 => {
                let entries = &self.times[i];
                for (terminal_name, class, time, entry_entity) in entries {
                    if ui
                        .button(RichText::new(format!("{:02}", time.to_hmsd().1)).monospace())
                        .clicked()
                    {
                        self.msg_open_ui
                            .write(UiCommand::OpenOrFocusTab(AppTab::Vehicle(*entry_entity)));
                    }
                }
            }
            _ => unreachable!(),
        };
    }
}

#[derive(Default)]
pub struct SelectedLineCache {
    vehicle_set: Option<Entity>,
    children: Vec<Entity>,
    name: String,
}

/// Display station times in Japanese style timetable
pub fn show_station_timetable(
    (InMut(ui), In(station)): (InMut<egui::Ui>, In<Entity>),
    station_names: Query<(&Name, Option<&Depot>), With<Station>>,
    vehicle_sets: Query<(Entity, &Children, &Name), With<VehicleSet>>,
    vehicles: Query<(Entity, &Name, &VehicleSchedule), With<Vehicle>>,
    timetable_entries: Query<(&TimetableEntry, &ChildOf)>,
    mut msg_passthrough: MessageWriter<AdjustTimetableEntry>,
    mut msg_open_ui: MessageWriter<UiCommand>,
    mut selected_line_cache: Local<SelectedLineCache>,
) {
    let mut selected_line_info: Option<Entity> = selected_line_cache.vehicle_set;
    egui::ComboBox::from_label("Vehicle set")
        .selected_text(selected_line_cache.name.as_str())
        .show_ui(ui, |ui| {
            for (vehicle_set_entity, children, name) in vehicle_sets.iter() {
                if ui
                    .selectable_value(
                        &mut selected_line_info,
                        Some(vehicle_set_entity),
                        name.as_str(),
                    )
                    .clicked()
                {
                    selected_line_cache.name = name.to_string();
                    selected_line_cache.vehicle_set = Some(vehicle_set_entity);
                    selected_line_cache.children = children.to_vec();
                }
            }
        });
    if ui.button("Refresh").clicked() {
        for vehicle in vehicles {
            let Some(entity) = vehicle.2.entities.get(0) else {
                continue;
            };
            msg_passthrough.write(AdjustTimetableEntry {
                entity: *entity,
                adjustment: TimetableAdjustment::PassThrough,
            });
        }
    }
    if selected_line_info.is_none() {
        ui.label("No vehicle set selected.");
        return;
    }
    let mut times: Vec<Vec<(String, String, TimetableTime, Entity)>> = vec![Vec::new(); 24];
    for entry in selected_line_cache
        .children
        .iter()
        .filter_map(|c| vehicles.get(*c).ok().and_then(|v| Some(&v.2.entities)))
        .flatten()
        .filter_map(|e| timetable_entries.get(*e).ok())
    {
        let (entry, parent) = entry;
        let Some(departure_time) = entry.departure_estimate else {
            continue;
        };
        if entry.departure.is_none() || entry.station != station {
            continue;
        };
        let (hour, ..) = departure_time.to_hmsd();
        let index = hour.rem_euclid(24) as usize;
        times[index].push((String::new(), String::new(), departure_time, parent.0))
    }
    for time in &mut times {
        time.sort_by_key(|k| k.2.to_hmsd().1);
    }
    let mut table_cache = TableCache {
        msg_open_ui: msg_open_ui,
        times,
    };
    Table::new()
        .num_rows(24u64)
        .columns(vec![Column::new(100.0).resizable(true); 2])
        .num_sticky_cols(1)
        .show(ui, &mut table_cache);
}
