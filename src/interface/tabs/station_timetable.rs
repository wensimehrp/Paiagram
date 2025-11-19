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
use egui::{Button, Frame, Label, Rect, RichText, Sense, Separator, UiBuilder, Widget, vec2};
use egui_table::{Column, Table, TableDelegate};

struct TableCache<'a> {
    msg_open_ui: MessageWriter<'a, UiCommand>,
    times: &'a [Vec<(&'a str, &'a str, TimetableTime, Entity)>],
}

impl TableDelegate for TableCache<'_> {
    fn default_row_height(&self) -> f32 {
        60.0 + 6.0
    }
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {}
    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
        let i = cell.row_nr as usize;
        match cell.col_nr {
            0 => {
                ui.style_mut().spacing.item_spacing.x = 0.0;
                ui.set_width(ui.available_width() - 1.0);
                ui.centered_and_justified(|ui| {
                    ui.label(format!("{:02}", i));
                });
                ui.add(Separator::default().spacing(0.0).vertical());
            }
            1 => {
                let entries = &self.times[i];
                ui.style_mut().spacing.item_spacing.x = 0.0;
                for (station_name, service_name, time, entity) in entries {
                    ui.add_space(6.0);
                    let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 60.0), Sense::click());
                    let response = ui
                        .scope_builder(
                            UiBuilder::new().sense(Sense::click()).max_rect(rect),
                            |ui| {
                                let response = ui.response();
                                let visuals = ui.style().interact(&response);
                                let mut stroke = visuals.bg_stroke;
                                stroke.width = 1.5;
                                Frame::canvas(ui.style())
                                    .fill(visuals.bg_fill.gamma_multiply(0.5))
                                    .stroke(stroke)
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.vertical_centered_justified(|ui| {
                                            ui.label(*service_name);
                                            ui.label(time.to_hmsd().1.to_string());
                                            ui.label(*station_name);
                                        });
                                    });
                            },
                        )
                        .response;
                    if response.clicked() {
                        self.msg_open_ui
                            .write(UiCommand::OpenOrFocusTab(AppTab::Vehicle(*entity)));
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
    vehicle_sets: Query<(Entity, &Children, &Name), With<VehicleSet>>,
    vehicles: Query<(Entity, &Name, &VehicleSchedule), With<Vehicle>>,
    timetable_entries: Query<(&TimetableEntry, &ChildOf)>,
    msg_open_ui: MessageWriter<UiCommand>,
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
    if selected_line_info.is_none() {
        ui.label("No vehicle set selected.");
        return;
    }
    let mut times: Vec<Vec<(&str, &str, TimetableTime, Entity)>> = vec![Vec::new(); 24];
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
        times[index].push(("北京西", "快车", departure_time, parent.0))
    }
    for time in &mut times {
        time.sort_by_key(|k| k.2.to_hmsd().1);
    }
    let mut table_cache = TableCache {
        msg_open_ui: msg_open_ui,
        times: &times,
    };
    Table::new()
        .num_rows(24u64)
        .headers(vec![])
        .columns(vec![
            Column::new(30.0).resizable(false),
            Column::new(500.0).resizable(true),
        ])
        .num_sticky_cols(1)
        .show(ui, &mut table_cache);
}
