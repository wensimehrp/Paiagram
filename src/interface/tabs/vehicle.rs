use crate::interface::widgets::timetable_popup;
use crate::interface::{
    AppTab, UiCommand,
    tabs::{Tab, station_timetable::StationTimetableTab},
};
use crate::units::time::{Duration, TimetableTime};
use crate::vehicles::entries::{ActualRouteEntry, TimetableEntryCache, VehicleScheduleCache};
use crate::vehicles::{
    AdjustTimetableEntry, TimetableAdjustment,
    entries::{TimetableEntry, TravelMode, VehicleSchedule},
    services::VehicleService,
};
use bevy::prelude::*;
use egui::{Color32, Label, Popup, Sense, Separator, Stroke, Ui, Vec2};
use egui_table::{CellInfo, HeaderCellInfo, Table, TableDelegate, columns::Column};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VehicleTab(pub Entity);

impl Tab for VehicleTab {
    const NAME: &'static str = "Vehicle";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        if let Err(e) = world.run_system_cached_with(show_vehicle, (ui, self.0)) {
            error!("UI Error while displaying vehicle page: {}", e)
        }
    }
    fn title(&self) -> egui::WidgetText {
        "Vehicle".into()
    }
    fn id(&self) -> egui::Id {
        egui::Id::new(format!("VehicleTab_{:?}", self.0))
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false; 2]
    }
}

const COLUMN_NAMES: &[&str] = &["Station", "Arri.", "Dept.", "Service", "Track", "Parent"];

struct TimetableInfo<'a> {
    station_name: Option<&'a str>,
    service_name: Option<&'a str>,
    track_name: Option<&'a str>,
    parent_name: Option<&'a str>,
    entry: &'a TimetableEntry,
    entry_cache: &'a TimetableEntryCache,
}

struct TableCache<'a> {
    entries: Vec<TimetableInfo<'a>>,
    timetable_entities: Option<&'a [ActualRouteEntry]>,
    msg_sender: MessageWriter<'a, AdjustTimetableEntry>,
    msg_open_ui: MessageWriter<'a, UiCommand>,
    vehicle_set: Entity,
}

impl<'a> TableCache<'a> {
    fn new(
        vehicle_schedule_cache: &'a VehicleScheduleCache,
        timetable_entries: &'a Query<(&TimetableEntry, &TimetableEntryCache, &ChildOf)>,
        names: &'a Query<(Entity, &Name)>,
        msg_sender: MessageWriter<'a, AdjustTimetableEntry>,
        msg_open_ui: MessageWriter<'a, UiCommand>,
        vehicle_set: Entity,
    ) -> Self {
        let schedule_length = vehicle_schedule_cache
            .actual_route
            .as_ref()
            .map_or(0, |r| r.len());
        let mut entries = Vec::with_capacity(schedule_length);
        for timetable_entry_entity in vehicle_schedule_cache.actual_route.iter().flatten() {
            let Ok((entry, entry_cache, parent)) =
                timetable_entries.get(timetable_entry_entity.inner())
            else {
                continue;
            };
            let station_name = names
                .get(entry.station)
                .ok()
                .and_then(|(_, name)| Some(name.as_str()));
            let service_name = match entry.service {
                Some(service_entity) => names
                    .get(service_entity)
                    .ok()
                    .and_then(|(_, name)| Some(name.as_str())),
                None => None,
            };
            let track_name = match entry.track {
                Some(track_entity) => names
                    .get(track_entity)
                    .ok()
                    .and_then(|(_, name)| Some(name.as_str())),
                None => None,
            };
            let parent_name = names
                .get(parent.0)
                .ok()
                .and_then(|(_, name)| Some(name.as_str()));
            let info = TimetableInfo {
                station_name,
                service_name,
                track_name,
                parent_name,
                entry,
                entry_cache,
            };
            entries.push(info);
        }
        Self {
            entries,
            msg_sender,
            msg_open_ui,
            vehicle_set,
            timetable_entities: vehicle_schedule_cache.actual_route.as_deref(),
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
        egui::Frame::new().show(ui, |ui| match cell.col_nr {
            0 => {
                if ui.button("☰").clicked() {
                    info!("123");
                }
                if ui.button("ℹ").clicked() {
                    self.msg_open_ui
                        .write(UiCommand::OpenOrFocusTab(AppTab::StationTimetable(
                            StationTimetableTab {
                                station_entity: self.entries[i].entry.station,
                            },
                        )));
                }
                ui.label(self.entries[i].station_name.unwrap_or("---"));
            }
            1 => {
                let response = ui.button(self.entries[i].entry.arrival.to_string());
                let Some(timetable_entities) = self.timetable_entities else {
                    return;
                };
                Popup::menu(&response)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        timetable_popup::popup(
                            timetable_entities[i].inner(),
                            (self.entries[i].entry, self.entries[i].entry_cache),
                            if i == 0 {
                                None
                            } else {
                                Some((self.entries[i - 1].entry, self.entries[i - 1].entry_cache))
                            },
                            &mut self.msg_sender,
                            ui,
                            true,
                        )
                    });
            }
            2 => {
                let response = ui.button(
                    self.entries[i]
                        .entry
                        .departure
                        .map(|t| t.to_string())
                        .unwrap_or("||".to_string()),
                );
                let Some(timetable_entities) = self.timetable_entities else {
                    return;
                };
                Popup::menu(&response)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        timetable_popup::popup(
                            timetable_entities[i].inner(),
                            (self.entries[i].entry, self.entries[i].entry_cache),
                            if i == 0 {
                                None
                            } else {
                                Some((self.entries[i - 1].entry, self.entries[i - 1].entry_cache))
                            },
                            &mut self.msg_sender,
                            ui,
                            false,
                        )
                    });
            }
            3 => {
                ui.label(self.entries[i].service_name.unwrap_or("---"));
            }
            4 => {
                ui.label(self.entries[i].track_name.unwrap_or("---"));
            }
            5 => {
                ui.label(self.entries[i].parent_name.unwrap_or("---"));
            }
            _ => unreachable!(),
        });
        ui.add_space(4.0);
    }
}

pub fn show_vehicle(
    (InMut(ui), In(entity)): (InMut<egui::Ui>, In<Entity>),
    schedules: Query<(&VehicleSchedule, &VehicleScheduleCache, &ChildOf)>,
    timetable_entries: Query<(&TimetableEntry, &TimetableEntryCache, &ChildOf)>,
    names: Query<(Entity, &Name)>,
    mut msg_sender: MessageWriter<AdjustTimetableEntry>,
    msg_open_ui: MessageWriter<UiCommand>,
    mut show: Local<bool>,
) {
    let Ok((vehicle_schedule, vehicle_schedule_cache, parent)) = schedules.get(entity) else {
        ui.label("The vehicle does not exist.");
        return;
    };
    if ui.button("Refresh").clicked() {
        for (schedule, _, _) in schedules {
            for entity in schedule.entities.iter().cloned() {
                msg_sender.write(AdjustTimetableEntry {
                    entity,
                    adjustment: TimetableAdjustment::PassThrough,
                });
            }
        }
    }
    let old_show = *show;
    ui.selectable_value(&mut *show, !old_show, "show");
    if !*show {
        return;
    }
    let mut current_table_cache = TableCache::new(
        vehicle_schedule_cache,
        &timetable_entries,
        &names,
        msg_sender,
        msg_open_ui,
        parent.0,
    );
    Table::new()
        .num_rows(vehicle_schedule.entities.len() as u64)
        .columns(
            (0..COLUMN_NAMES.len())
                .map(|v| match v {
                    0 => Column::new(100.0).resizable(true),
                    1 => Column::new(90.0).resizable(false),
                    2 => Column::new(90.0).resizable(false),
                    3 => Column::new(100.0).resizable(true),
                    4 => Column::new(100.0).resizable(true),
                    5 => Column::new(100.0).resizable(true),
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>(),
        )
        .num_sticky_cols(1)
        .show(ui, &mut current_table_cache);
}
