use egui::{Color32, Rect, RichText, Stroke, WidgetText};
use egui_i18n::tr;
use paiagram_core::StationKey;
use paiagram_core::units::time::TimetableTime;
use serde::{Deserialize, Serialize};

use super::{AppState, Tab};
use crate::widgets::indicators::{
    display_time_indicator_indicator_horizontal, display_time_indicator_indicator_vertical,
};

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct StationTab {
    station_key: StationKey,
    include_nonstop: bool,
}

impl PartialEq for StationTab {
    fn eq(&self, other: &Self) -> bool {
        self.station_key == other.station_key
    }
}

impl StationTab {
    pub(crate) fn new(station_key: StationKey) -> Self {
        Self { station_key, include_nonstop: false }
    }
}

impl super::Tab for StationTab {
    const NAME: &'static str = "Station";
    fn title(&self) -> WidgetText {
        tr!("tab-station").into()
    }
    fn main_display(&mut self, app: &mut AppState, ui: &mut egui::Ui) {
        let station_name = app.source.stations.query(self.station_key, |b| b.name.clone())
            .unwrap_or_default();
        ui.heading(station_name.as_str());
        ui.checkbox(&mut self.include_nonstop, tr!("station-include-non-stop"));
        egui::ScrollArea::both().show(ui, |ui| {
            display_time_grid(self, app, ui);
        });
    }
}

fn display_time_grid(tab: &StationTab, app: &AppState, ui: &mut egui::Ui) {
    // Build list of passing trips/entries through this station
    #[derive(Clone)]
    struct DisplayedEntry {
        time: TimetableTime,
        color: Color32,
        trip_name: String,
        last_station_name: String,
    }

    let mut entry_bucket: [Vec<DisplayedEntry>; 24] = [const { Vec::new() }; 24];

    // Scan all trips for entries at this station
    for tk in app.source.trips.keys() {
        let trip_view = app.source.trips.get_view(*tk);
        let Some(ref view) = trip_view else { continue; };

        for (entry_idx, entry) in view.schedule.entries().iter().enumerate() {
            let stn_key = match entry {
                paiagram_core::trip::TEntry::Derived(s) => *s,
                paiagram_core::trip::TEntry::Pinned { stn: s, .. } => *s,
                paiagram_core::trip::TEntry::PinnedNonStop { stn: s, .. } => *s,
                paiagram_core::trip::TEntry::PinnedExternalNonStop { stn: s, .. } => *s,
                paiagram_core::trip::TEntry::PinnedExternal { .. } => continue,
            };

            if stn_key != tab.station_key { continue; }

            // Get departure estimate
            let dep_time = match entry {
                paiagram_core::trip::TEntry::Pinned { dep, .. } => match dep {
                    paiagram_core::trip::TravelMode::At(t) => *t,
                    paiagram_core::trip::TravelMode::For(d) => {
                        // For "For" duration, we need the arrival time + duration
                        // For simplicity, just use a rough estimate
                        TimetableTime(0)
                    }
                    paiagram_core::trip::TravelMode::Flexible => continue,
                },
                paiagram_core::trip::TEntry::Derived(_) => continue,
                _ => continue,
            };

            if !tab.include_nonstop {
                // Skip entries without an arrival mode (non-stop)
                let has_arr = matches!(entry, paiagram_core::trip::TEntry::Pinned { .. });
                if !has_arr { continue; }
            }

            // Find last station name
            let last_station_name = view.schedule.entries().last()
                .and_then(|last| match last {
                    paiagram_core::trip::TEntry::Derived(s) |
                        paiagram_core::trip::TEntry::Pinned { stn: s, .. } |
                        paiagram_core::trip::TEntry::PinnedNonStop { stn: s, .. } |
                        paiagram_core::trip::TEntry::PinnedExternalNonStop { stn: s, .. } => {
                        app.source.stations.query(*s, |b| b.name.clone())
                    }
                    _ => None,
                })
                .unwrap_or_default();

            // Get class color
            let color = view.class
                .and_then(|ck| app.source.classes.get_view(ck))
                .map(|cv| cv.style.color)
                .unwrap_or(Color32::GRAY);

            entry_bucket[dep_time.hour() as usize].push(DisplayedEntry {
                time: dep_time,
                color,
                trip_name: view.name.to_string(),
                last_station_name: last_station_name.to_string(),
            });
        }
    }

    for line in entry_bucket.iter_mut() {
        line.sort_by_key(|it| it.time.minute() * 60 + it.time.second());
    }

    let mut heights: [f32; 25] = [0.0; 25];
    let (current_h, current_min, current_secs, _) =
        app.timer.read_ticks().to_timetable_time().to_hmsd();
    let current_h = current_h as usize;
    let mut widths_seconds: Vec<(f32, i32)> = Vec::with_capacity(entry_bucket[current_h].len() + 2);
    widths_seconds.push((ui.clip_rect().left(), 0));

    egui::Grid::new("station grid")
        .striped(true)
        .show(ui, |ui| {
            for (line_idx, entries) in entry_bucket.into_iter().enumerate() {
                ui.horizontal_centered(|ui| {
                    ui.heading(line_idx.to_string());
                });
                fn display_entry(entry: DisplayedEntry, ui: &mut egui::Ui) {
                    ui.vertical_centered(|ui| {
                        ui.small(entry.last_station_name);
                        ui.label(
                            RichText::new(entry.time.minute().to_string())
                                .color(entry.color)
                                .font(egui::FontId::new(
                                    16.0,
                                    egui::FontFamily::Name("dia_pro".into()),
                                )),
                        );
                        ui.small(entry.trip_name);
                    });
                }
                if line_idx == current_h {
                    let mut push_widths_seconds = |minutes: i32, ui: &egui::Ui| {
                        if widths_seconds.last().unwrap().1 == minutes * 60 { return; }
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

    // Time indicators
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
        .map(|(i, (x, s))| (i, (*x, *s)))
        .unwrap_or((0, (0.0, 0)));
    let (next_x, next_seconds) = if base_x_idx + 1 < widths_seconds.len() {
        let (nx, ns) = widths_seconds[base_x_idx + 1];
        (nx, ns)
    } else {
        (base_x, base_seconds + 3600)
    };
    let width = next_x - base_x;
    let width_seconds = next_seconds - base_seconds;
    let minute_progress = if width_seconds > 0 {
        (current_seconds - base_seconds) as f32 / (width_seconds) as f32
    } else {
        0.0
    };
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
    ui.painter().vline(minute_line_x, base_y..=next_y, line_stroke);
}
