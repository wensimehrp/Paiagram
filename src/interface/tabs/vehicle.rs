use crate::interface::{AppTab, UiCommand};
use crate::units::time::{Duration, TimetableTime};
use crate::vehicles::{
    AdjustTimetableEntry, TimetableAdjustment,
    entries::{TimetableEntry, TravelMode, VehicleSchedule},
    services::VehicleService,
};
use bevy::prelude::*;
use egui::{Color32, Label, Sense, Separator, Stroke, Ui, Vec2};
use egui_table::{CellInfo, HeaderCellInfo, Table, TableDelegate, columns::Column};

const COLUMN_NAMES: &[&str] = &["Station", "Arri.", "Dept.", "Service", "Track", "Parent"];

struct TimetableInfo<'a> {
    station_name: Option<&'a str>,
    service_name: Option<&'a str>,
    track_name: Option<&'a str>,
    parent_name: Option<&'a str>,
    entry: &'a TimetableEntry,
}

struct TableCache<'a> {
    entries: Vec<TimetableInfo<'a>>,
    timetable_entities: &'a [Entity],
    msg_sender: MessageWriter<'a, AdjustTimetableEntry>,
    msg_open_ui: MessageWriter<'a, UiCommand>,
    vehicle_set: Entity,
}

impl<'a> TableCache<'a> {
    fn new(
        vehicle_schedule: &'a VehicleSchedule,
        timetable_entries: &'a Query<(&TimetableEntry, &ChildOf)>,
        names: &'a Query<(Entity, &Name)>,
        msg_sender: MessageWriter<'a, AdjustTimetableEntry>,
        msg_open_ui: MessageWriter<'a, UiCommand>,
        vehicle_set: Entity,
    ) -> Self {
        let schedule_length = vehicle_schedule.entities.len();
        let mut entries = Vec::with_capacity(schedule_length);
        for timetable_entry_entity in vehicle_schedule.entities.iter() {
            let Ok((entry, parent)) = timetable_entries.get(*timetable_entry_entity) else {
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
            };
            entries.push(info);
        }
        Self {
            entries,
            msg_sender,
            msg_open_ui,
            vehicle_set,
            timetable_entities: &vehicle_schedule.entities,
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
                            self.entries[i].entry.station,
                        )));
                }
                ui.label(self.entries[i].station_name.unwrap_or("---"));
            }
            1 => {
                use crate::interface::widgets::scrollable_time::time_widget;
                time_widget(
                    ui,
                    self.entries[i].entry.arrival,
                    self.entries[i].entry.arrival_estimate,
                    if i == 0 {
                        None
                    } else {
                        self.entries[i - 1].entry.departure_estimate
                    },
                    self.timetable_entities[i],
                    &mut None,
                    &mut self.msg_sender,
                );
            }
            2 => {
                if ui
                    .monospace(match self.entries[i].entry.departure {
                        Some(v) => format!("{}", v),
                        None => "..".to_string(),
                    })
                    .clicked()
                {
                    self.msg_sender.write(AdjustTimetableEntry {
                        entity: self.timetable_entities[i],
                        adjustment: TimetableAdjustment::SetDepartureType(Some(TravelMode::For(
                            Duration(100),
                        ))),
                    });
                };
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
    schedules: Query<(&VehicleSchedule, &ChildOf)>,
    timetable_entries: Query<(&TimetableEntry, &ChildOf)>,
    names: Query<(Entity, &Name)>,
    mut msg_sender: MessageWriter<AdjustTimetableEntry>,
    msg_open_ui: MessageWriter<UiCommand>,
) {
    let Ok((vehicle_schedule, parent)) = schedules.get(entity) else {
        ui.label("The vehicle does not exist.");
        return;
    };
    let stroke_width = 16.0;
    let (rect, response) =
        ui.allocate_at_least(Vec2::new(ui.available_width(), stroke_width), Sense::hover());
    let painter = ui.painter();
    let stroke = Stroke {
        width: stroke_width,
        color: Color32::LIGHT_RED,
    };
    if ui.is_rect_visible(response.rect) {
        painter.hline(rect.left()..=rect.right(), rect.center().y, stroke);
    }
    if ui.button("Refresh").clicked() {
        for schedule in schedules.iter() {
            let Some(entity) = schedule.0.entities.get(0) else {
                continue;
            };
            msg_sender.write(AdjustTimetableEntry {
                entity: *entity,
                adjustment: TimetableAdjustment::PassThrough,
            });
        }
    }
    let mut current_table_cache = TableCache::new(
        vehicle_schedule,
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
