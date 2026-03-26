use bevy::prelude::*;
use egui::{Color32, RichText, Stroke};
use moonshine_core::prelude::MapEntities;
use paiagram_core::{
    class::ClassQuery,
    entry::EntryQuery,
    station::{ParentStationOrStation, PlatformEntries, StationQuery},
    trip::TripQuery,
    units::time::TimetableTime,
};
use serde::{Deserialize, Serialize};

use crate::GlobalTimer;

#[derive(MapEntities, Serialize, Deserialize, Clone, PartialEq)]
pub struct StationTab {
    #[entities]
    station_entity: Entity,
}

impl StationTab {
    pub fn new(station_entity: Entity) -> Self {
        Self { station_entity }
    }
}

impl super::Tab for StationTab {
    const NAME: &'static str = "Station";
    fn main_display(&mut self, world: &mut World, ui: &mut egui::Ui) {
        let station_name = world.get::<Name>(self.station_entity).unwrap().as_str();
        ui.heading(station_name);
        egui::ScrollArea::both().show(ui, |ui| {
            world
                .run_system_cached_with(display_time_grid, (ui, self.station_entity))
                .unwrap();
        });
    }
}

fn display_time_grid(
    (InMut(ui), In(station_entity)): (InMut<egui::Ui>, In<Entity>),
    station_q: Query<StationQuery>,
    platform_entry_q: Query<&PlatformEntries>,
    entry_q: Query<EntryQuery>,
    class_q: Query<ClassQuery>,
    trip_q: Query<TripQuery>,
    parent_station_or_station: Query<ParentStationOrStation>,
    global_timer: Res<GlobalTimer>,
) {
    struct DisplayedEntry<'a> {
        time: TimetableTime,
        color: Color32,
        trip_name: &'a str,
        last_station_abbrev: &'a str,
    }
    let mut entry_bucket: [Vec<DisplayedEntry>; 24] = [const { Vec::new() }; 24];
    let station_info = station_q.get(station_entity).unwrap();
    let station_entry_iter = station_info
        .passing_entries(&platform_entry_q)
        .map(|e| entry_q.get(e).unwrap())
        .filter(|it| it.mode.arr.is_some());
    for e in station_entry_iter {
        let Some(estimate) = e.estimate else {
            continue;
        };
        let trip = trip_q.get(e.parent_schedule.parent()).unwrap();
        let last_entry_entity = *trip.schedule.last().unwrap();
        let last_stop_entity = entry_q.get(last_entry_entity).unwrap().stop();
        let last_station_entity = parent_station_or_station
            .get(last_stop_entity)
            .unwrap()
            .parent();
        let last_station_name = station_q.get(last_station_entity).unwrap().name.as_str();
        let class = class_q.get(trip.class.entity()).unwrap();
        entry_bucket[estimate.dep.hour() as usize].push(DisplayedEntry {
            time: estimate.dep,
            color: class.stroke.color.get(ui.visuals().dark_mode),
            trip_name: trip.name.as_str(),
            last_station_abbrev: last_station_name,
        });
    }
    for line in entry_bucket.iter_mut() {
        line.sort_by_key(|it| it.time.minute() * 60 + it.time.second());
    }
    let mut heights: [f32; 25] = [0.0; 25];
    let font_id = egui::FontId::new(16.0, egui::FontFamily::Name("dia_pro".into()));
    let (current_h, current_min, current_secs, _) =
        global_timer.read_ticks().to_timetable_time().to_hmsd();
    let current_h = current_h as usize;
    let mut widths_seconds: Vec<(f32, i32)> = Vec::with_capacity(entry_bucket[current_h].len() + 2);
    widths_seconds.push((ui.clip_rect().left(), 0));
    egui::Grid::new("station grid")
        .striped(true)
        .num_columns(entry_bucket.iter().map(|it| it.len()).max().unwrap() + 1)
        .show(ui, |ui| {
            for (line_idx, entries) in entry_bucket.into_iter().enumerate() {
                ui.heading(line_idx.to_string());
                let display_entry = |entry: DisplayedEntry, ui: &mut egui::Ui| {
                    ui.vertical_centered(|ui| {
                        ui.small(entry.last_station_abbrev);
                        ui.label(
                            RichText::new(entry.time.minute().to_string())
                                .color(entry.color)
                                .font(font_id.clone()),
                        );
                        ui.small(entry.trip_name);
                    });
                };
                if line_idx == current_h {
                    let mut push_widths_seconds = |minutes: i32, ui: &egui::Ui| {
                        if widths_seconds.last().unwrap().1 / 60 == minutes {
                            return;
                        }
                        widths_seconds.push((ui.cursor().left(), minutes * 60));
                    };
                    for entry in entries {
                        push_widths_seconds(entry.time.minute(), ui);
                        display_entry(entry, ui);
                    }
                    push_widths_seconds(60, ui);
                } else {
                    for entry in entries {
                        display_entry(entry, ui);
                    }
                }
                heights[line_idx] = ui.cursor().top();
                ui.end_row();
            }
        });
    heights[24] = ui.cursor().top();
    let current_seconds = current_min * 60 + current_secs;
    let hour_progress = current_seconds as f32 / 3600.0;
    let base_y = heights[current_h];
    let next_y = heights[current_h + 1];
    let block_height = next_y - base_y;
    let mut hour_line_y = base_y + block_height * hour_progress;
    let (base_x_idx, (base_x, base_seconds)) = widths_seconds
        .iter()
        .enumerate()
        .rev()
        .find(|(_, (_, secs))| current_seconds >= *secs)
        .unwrap();
    let (next_x, next_seconds) = widths_seconds[base_x_idx + 1];
    let width = next_x - base_x;
    let width_seconds = next_seconds - base_seconds;
    let minute_progress = (current_seconds - base_seconds) as f32 / (width_seconds) as f32;
    let mut minute_line_x = base_x + width * minute_progress;
    let line_stroke = Stroke::new(1.5, Color32::RED);
    line_stroke.round_center_to_pixel(ui.pixels_per_point(), &mut hour_line_y);
    line_stroke.round_center_to_pixel(ui.pixels_per_point(), &mut minute_line_x);
    ui.painter().hline(
        ui.clip_rect().left()..=ui.clip_rect().right(),
        hour_line_y,
        line_stroke,
    );
    ui.painter()
        .vline(minute_line_x, base_y..=next_y, line_stroke);
}
