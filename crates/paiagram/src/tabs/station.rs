//! Station tab - shows station timetable in a 24-hour grid

use egui::{Color32, Rect, RichText, Stroke, WidgetText};
use egui_i18n::tr;
use paiagram_core::trip::TEntry;
use paiagram_core::units::time::TimetableTime;
use paiagram_core::StationKey;
use serde::{Deserialize, Serialize};

use crate::widgets::indicators::{
    display_time_indicator_indicator_horizontal, display_time_indicator_indicator_vertical,
};
use crate::App;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct StationTab {
    station: StationKey,
    include_nonstop: bool,
}

impl StationTab {
    pub(crate) fn new(station: StationKey) -> Self {
        Self {
            station,
            include_nonstop: false,
        }
    }
}

struct DisplayedEntry {
    time: TimetableTime,
    color: Color32,
    trip_name: String,
    last_station_abbrev: String,
}

impl super::Tab for StationTab {
    const NAME: &'static str = "Station";
    fn title(&self) -> WidgetText {
        tr!("tab-station").into()
    }
    fn main_display(&mut self, app: &mut App, ui: &mut egui::Ui) {
        let Some(stn_handle) = app.stations.get_handle(self.station) else {
            ui.label("???");
            return;
        };
        let name = app.stations.get_name(stn_handle);
        ui.heading(name.as_str());
        ui.checkbox(&mut self.include_nonstop, tr!("station-include-non-stop"));

        // Collect all entries passing through this station
        let mut entry_bucket: [Vec<DisplayedEntry>; 24] =
            [const { Vec::new() }; 24];

        for (trip_key, _) in app.trips_iter() {
            let Some(trip_handle) = app.trips.get_handle(trip_key) else {
                continue;
            };
            let trip_name = app.trips.get_name(trip_handle);
            let entries = app.trips.get_entries(trip_handle);

            // Find entries at this station
            for entry in &entries {
                let (stn_key, mode) = match *entry {
                    TEntry::Pinned { stn, dep, .. } => (stn, dep),
                    TEntry::PinnedNonStop { stn, pass, .. } => {
                        if !self.include_nonstop {
                            continue;
                        }
                        (stn, pass)
                    }
                    _ => continue,
                };
                if stn_key != self.station {
                    continue;
                }

                let time = match mode {
                    paiagram_core::trip::TravelMode::At(t) => t,
                    paiagram_core::trip::TravelMode::For(_) => continue,
                    paiagram_core::trip::TravelMode::Flexible => continue,
                };

                // Get trip class color
                let color = app.trips.get_view(trip_key).and_then(|v| v.class).and_then(|class_key| {
                    app.classes.get_view(class_key).map(|cv| cv.style.color)
                }).unwrap_or(Color32::GRAY);

                // Get last station abbreviation
                let last_station_name = app.stations.get_name(
                    app.stations.get_handle(
                        entries
                            .iter()
                            .rev()
                            .find_map(|e| match e {
                                TEntry::Pinned { stn, .. } | TEntry::PinnedNonStop { stn, .. } => {
                                    Some(*stn)
                                }
                                _ => None,
                            })
                            .unwrap_or(self.station),
                    )
                    .expect("last station not found"),
                );

                let abbrev: String = last_station_name
                    .chars()
                    .take(4)
                    .collect();

                entry_bucket[time.hour() as usize].push(DisplayedEntry {
                    time,
                    color,
                    trip_name: trip_name.to_string(),
                    last_station_abbrev: abbrev,
                });
            }
        }

        // Sort each hour bucket by time
        for line in entry_bucket.iter_mut() {
            line.sort_by_key(|it| it.time.minute() * 60 + it.time.second());
        }

        // Display in scrollable grid
        egui::ScrollArea::both().show(ui, |ui| {
            let mut heights: [f32; 25] = [0.0; 25];
            let current_time = app.timer.read_ticks().to_timetable_time();
            let (current_h, current_min, current_secs, _) = current_time.to_hmsd();
            let current_h = current_h as usize;
            let mut widths_seconds: Vec<(f32, i32)> =
                Vec::with_capacity(entry_bucket[current_h].len() + 2);
            widths_seconds.push((ui.clip_rect().left(), 0));

            egui::Grid::new("station grid")
                .striped(true)
                .show(ui, |ui| {
                    for (line_idx, entries) in entry_bucket.into_iter().enumerate() {
                        ui.horizontal_centered(|ui| {
                            ui.heading(line_idx.to_string());
                        });

                        fn display_entry(entry: &DisplayedEntry, ui: &mut egui::Ui) {
                            ui.vertical_centered(|ui| {
                                ui.small(&entry.last_station_abbrev);
                                ui.label(
                                    RichText::new(entry.time.minute().to_string())
                                        .color(entry.color)
                                        .font(egui::FontId::new(
                                            16.0,
                                            egui::FontFamily::Name("dia_pro".into()),
                                        )),
                                );
                                ui.small(&entry.trip_name);
                            });
                        }

                        if line_idx == current_h {
                            let mut push_widths_seconds =
                                |minutes: i32, ui: &egui::Ui| {
                                    if widths_seconds.last().unwrap().1 == minutes * 60 {
                                        return;
                                    }
                                    widths_seconds.push((ui.cursor().left(), minutes * 60));
                                };
                            for entry in &entries {
                                push_widths_seconds(entry.time.minute(), ui);
                                display_entry(entry, ui);
                            }
                            push_widths_seconds(60, ui);
                        } else {
                            for entry in &entries {
                                display_entry(entry, ui);
                            }
                        }
                        heights[line_idx] = ui.cursor().top();
                        ui.end_row();
                    }
                });
            heights[24] = ui.cursor().top();

            // Time indicators
            let current_seconds = current_min * 60 + current_secs;
            let hour_progress = current_seconds as f32 / 3600.0;
            let base_y = heights[current_h];
            let next_y = heights[current_h + 1];
            let block_height = next_y - base_y;
            let mut hour_line_y = base_y + block_height * hour_progress;
            let fallback = (ui.clip_rect().left(), 0);
            let (base_x_idx, (base_x, base_seconds)) = widths_seconds
                .iter()
                .enumerate()
                .rev()
                .find(|(_, (_, secs))| current_seconds >= *secs)
                .unwrap_or((0, &fallback));
            let (next_x, next_seconds) = if base_x_idx + 1 < widths_seconds.len() {
                widths_seconds[base_x_idx + 1]
            } else {
                (ui.clip_rect().right(), current_seconds + 60)
            };
            let width = next_x - base_x;
            let width_seconds = (next_seconds - base_seconds).max(1);
            let minute_progress = (current_seconds - base_seconds) as f32 / width_seconds as f32;
            let mut minute_line_x = base_x + width * minute_progress;
            let line_stroke = Stroke::new(1.5, Color32::RED);
            line_stroke.round_center_to_pixel(ui.pixels_per_point(), &mut hour_line_y);
            line_stroke.round_center_to_pixel(ui.pixels_per_point(), &mut minute_line_x);

            display_time_indicator_indicator_vertical(
                ui.id().with("hour indicator"),
                ui.clip_rect(),
                hour_line_y,
                line_stroke.color,
                ui.painter(),
            );
            ui.painter().hline(
                ui.clip_rect().left()..=ui.clip_rect().right(),
                hour_line_y,
                line_stroke,
            );
            display_time_indicator_indicator_horizontal(
                ui.id().with("minute indicator"),
                Rect::from_x_y_ranges(ui.clip_rect().x_range(), base_y..=next_y),
                minute_line_x,
                line_stroke.color,
                ui.painter(),
            );
            ui.painter()
                .vline(minute_line_x, base_y..=next_y, line_stroke);
        });
    }
}
