use crate::{
    graph::{Depot, Station, StationEntries},
    interface::{
        AppTab, UiCommand,
        tabs::{Tab, vehicle},
    },
    units::time::TimetableTime,
    vehicles::{
        AdjustTimetableEntry, TimetableAdjustment, Vehicle,
        entries::{
            ActualRouteEntry, TimetableEntry, TimetableEntryCache, TravelMode, VehicleSchedule,
            VehicleScheduleCache,
        },
        services::VehicleService,
        vehicle_set::VehicleSet,
    },
};
use bevy::{
    ecs::{
        entity::Entity,
        entity::{EntityMapper, MapEntities},
        hierarchy::{ChildOf, Children},
        message::{MessageReader, MessageWriter},
        name::Name,
        query::With,
        system::{In, InMut, Local, Query},
    },
    log::{error, info},
};
use egui::{Button, Frame, Label, Rect, RichText, Sense, Separator, UiBuilder, Widget, vec2};
use egui_table::{Column, Table, TableDelegate};
use moonshine_core::kind::Instance;
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct StationTimetableTab {
    pub station_entity: Instance<Station>,
}

impl Serialize for StationTimetableTab {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.station_entity.entity().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StationTimetableTab {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let entity = Entity::deserialize(deserializer)?;
        Ok(Self {
            // SAFETY: entity is expected to be a valid Station when loaded.
            station_entity: unsafe { Instance::from_entity_unchecked(entity) },
        })
    }
}

impl MapEntities for StationTimetableTab {
    fn map_entities<M: EntityMapper>(&mut self, entity_mapper: &mut M) {
        self.station_entity.map_entities(entity_mapper);
    }
}

impl Tab for StationTimetableTab {
    const NAME: &'static str = "Station Timetable";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut egui::Ui) {
        if let Err(e) =
            world.run_system_cached_with(show_station_timetable, (ui, self.station_entity))
        {
            error!("UI Error while displaying station timetable page: {}", e)
        }
    }
    fn id(&self) -> egui::Id {
        egui::Id::new(self.station_entity.entity())
    }
}

struct TableCache<'a> {
    msg_open_ui: MessageWriter<'a, UiCommand>,
    times: &'a [Vec<(&'a str, &'a str, TimetableTime, Entity)>],
    page_settings: &'a PageSettings,
}

impl TableDelegate for TableCache<'_> {
    fn default_row_height(&self) -> f32 {
        let mut height = 20.0 + 7.0;
        if self.page_settings.show_service_name {
            height += 20.0;
        }
        if self.page_settings.show_terminal_station {
            height += 20.0;
        }
        height
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
                    let (rect, resp) = ui.allocate_exact_size(
                        vec2(
                            if self.page_settings.show_service_name
                                || self.page_settings.show_terminal_station
                            {
                                67.0
                            } else {
                                20.0
                            },
                            ui.available_height(),
                        ),
                        Sense::click(),
                    );
                    let response = ui
                        .scope_builder(
                            UiBuilder::new().sense(Sense::click()).max_rect(rect),
                            |ui| {
                                let response = ui.response();
                                let visuals = ui.style().interact(&response);
                                let mut stroke = visuals.bg_stroke;
                                stroke.width = 1.5;
                                Frame::canvas(ui.style())
                                    .fill(visuals.bg_fill.gamma_multiply(0.2))
                                    .stroke(stroke)
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.vertical_centered_justified(|ui| {
                                            if self.page_settings.show_service_name {
                                                ui.add(Label::new(*service_name).truncate());
                                            }
                                            ui.monospace(time.to_hmsd().1.to_string());
                                            if self.page_settings.show_terminal_station {
                                                ui.add(Label::new(*station_name).truncate());
                                            }
                                        });
                                    });
                            },
                        )
                        .response;
                    if response.clicked() {
                        self.msg_open_ui
                            .write(UiCommand::OpenOrFocusTab(AppTab::Vehicle(
                                vehicle::VehicleTab(*entity),
                            )));
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

pub struct PageSettings {
    show_service_name: bool,
    show_terminal_station: bool,
}
impl Default for PageSettings {
    fn default() -> Self {
        Self {
            show_service_name: true,
            show_terminal_station: true,
        }
    }
}

/// Display station times in Japanese style timetable
pub fn show_station_timetable(
    (InMut(ui), In(station)): (InMut<egui::Ui>, In<Instance<Station>>),
    vehicle_sets: Query<(Entity, &Children, &Name), With<VehicleSet>>,
    vehicles: Query<(Entity, &Name, &VehicleScheduleCache, &VehicleSchedule), With<Vehicle>>,
    station_names: Query<&Name, With<Station>>,
    station_caches: Query<&StationEntries>,
    service_names: Query<&Name, With<VehicleService>>,
    timetable_entries: Query<(&TimetableEntry, &TimetableEntryCache, &ChildOf)>,
    msg_open_ui: MessageWriter<UiCommand>,
    mut page_settings: Local<PageSettings>,
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
    ui.checkbox(&mut page_settings.show_service_name, "Show service name");
    ui.checkbox(
        &mut page_settings.show_terminal_station,
        "Show terminus station",
    );
    let mut times: Vec<Vec<(&str, &str, TimetableTime, Entity)>> = vec![Vec::new(); 24];
    if let Ok(station_cache) = station_caches.get(station.entity()) {
        for (entry, entry_cache, parent) in station_cache
            .entries()
            .iter()
            .filter_map(|e| timetable_entries.get(*e).ok())
        {
            if !selected_line_cache.children.contains(&parent.0) {
                continue;
            }
            if entry.departure.is_none() {
                continue;
            }
            let Some(estimate) = &entry_cache.estimate else {
                continue;
            };
            let (hour, ..) = estimate.departure.to_hmsd();
            let index = hour.rem_euclid(24) as usize;
            let mut terminal_name = "---";
            let mut service_name = "---";
            if let Ok((_, _, schedule_cache, _schedule)) = vehicles.get(parent.0)
                && let Some(entry_service) = entry.service
                && let Some(last_entry_entity) = schedule_cache
                    .get_service_last_entry(entry_service)
                    .map(ActualRouteEntry::inner)
                && let Ok((last_entry, _, _)) = timetable_entries.get(last_entry_entity)
                && let Ok(name) = station_names.get(last_entry.station)
            {
                terminal_name = name
            }
            if let Some(entry_service) = entry.service
                && let Ok(name) = service_names.get(entry_service.entity())
            {
                service_name = name;
            }
            times[index].push((terminal_name, service_name, estimate.departure, parent.0))
        }
    }
    for time in &mut times {
        time.sort_by_key(|k| k.2.to_hmsd().1);
    }
    let mut table_cache = TableCache {
        msg_open_ui: msg_open_ui,
        times: &times,
        page_settings: &page_settings,
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
