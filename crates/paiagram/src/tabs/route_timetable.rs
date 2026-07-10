use egui::{FontId, Layout, Rect, RichText, Ui, Vec2, WidgetText, vec2};
use egui_i18n::tr;
use egui_table::{Column, Table, TableDelegate};
use paiagram_core::trip::{TEntry, TravelMode};
use paiagram_core::units::time::TimetableTime;
use paiagram_core::RouteKey;
use serde::{Deserialize, Serialize};

use super::{AppState, Tab};
use crate::widgets::TimeDragValueOud;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct StationDisplayMode {
    pub arrival: bool,
    pub departure: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct RouteTimetableTab {
    route_key: RouteKey,
    #[serde(skip)]
    route_display_modes: Vec<StationDisplayMode>,
}

impl RouteTimetableTab {
    pub(crate) fn new(rk: RouteKey) -> Self {
        Self { route_key: rk, route_display_modes: Vec::new() }
    }
}

impl PartialEq for RouteTimetableTab {
    fn eq(&self, other: &Self) -> bool {
        self.route_key == other.route_key
    }
}

impl Tab for RouteTimetableTab {
    const NAME: &'static str = "Route Timetable";
    fn title(&self) -> WidgetText {
        tr!("tab-route-timetable").into()
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false; 2]
    }
    fn edit_display(&mut self, _app: &mut AppState, ui: &mut Ui) {
        if ui.button(tr!("route-timetable-sort-entries")).clicked() {}
    }
    fn main_display(&mut self, app: &mut AppState, ui: &mut egui::Ui) {
        let route_view = app.source.routes.get_view(self.route_key);
        let Some(ref route) = route_view else { ui.label("Route not found"); return; };

        let station_keys: Vec<_> = route.stations.iter().copied().collect();

        // Init display modes
        if self.route_display_modes.len() != station_keys.len() {
            self.route_display_modes = vec![StationDisplayMode { arrival: true, departure: true }; station_keys.len()];
        }

        // Collect trips that reference stations on this route
        let mut route_trips: Vec<paiagram_core::TripKey> = Vec::new();
        for tk in app.source.trips.keys() {
            if let Some(view) = app.source.trips.get_view(*tk) {
                if view.schedule.entries().iter().any(|entry| {
                    let sk = match entry {
                        TEntry::Derived(s) => *s,
                        TEntry::Pinned { stn: s, .. } => *s,
                        TEntry::PinnedNonStop { stn: s, .. } => *s,
                        TEntry::PinnedExternalNonStop { stn: s, .. } => *s,
                        TEntry::PinnedExternal { .. } => return false,
                    };
                    station_keys.contains(&sk)
                }) {
                    route_trips.push(*tk);
                }
            }
        }

        let table = Table::new()
            .id_salt(ui.id().with(("route", self.route_key)))
            .num_rows(station_keys.len() as u64)
            .num_sticky_cols(2);

        let mut displayer = RouteTimetableDisplayer {
            route_display_modes: &mut self.route_display_modes,
            station_keys: &station_keys,
            trips: Vec::new(),
            available_trips: &route_trips,
            column_offset: 0,
            app,
        };

        let number_style = egui::TextStyle::Name("number".into());
        ui.style_mut().text_styles.insert(
            number_style.clone(),
            egui::FontId::new(15.0, egui::FontFamily::Name("dia_pro".into())),
        );
        ui.style_mut().drag_value_text_style = number_style;
        ui.style_mut().spacing.interact_size = Vec2::ZERO;
        ui.style_mut().spacing.button_padding = Vec2::ZERO;
        ui.style_mut().visuals.button_frame = false;

        table
            .columns(
                std::iter::once(Column::new(80.0).resizable(true))
                    .chain(std::iter::once(Column::new(20.0).resizable(false)))
                    .chain((0..route_trips.len()).map(|_| Column::new(36.0).resizable(false)))
                    .collect::<Vec<_>>(),
            )
            .show(ui, &mut displayer);
    }
}

struct EntryData {
    arr_mode: Option<TravelMode>,
    arr_time: Option<TimetableTime>,
    dep_mode: TravelMode,
    dep_time: TimetableTime,
}

#[derive(Clone)]
enum CellKind {
    Skipped,
    NoOperation,
    Terminated,
    Stop(usize), // index into current trip's entries
}

struct PreparedTrip {
    name: String,
    cells: Vec<CellKind>,
    entry_data: Vec<EntryData>,
}

struct RouteTimetableDisplayer<'a> {
    route_display_modes: &'a mut [StationDisplayMode],
    station_keys: &'a [paiagram_core::StationKey],
    trips: Vec<PreparedTrip>,
    available_trips: &'a [paiagram_core::TripKey],
    column_offset: usize,
    app: &'a mut AppState,
}

impl RouteTimetableDisplayer<'_> {
    fn table_cell_width() -> f32 { 36.0 }
    fn cell_size() -> Vec2 { vec2(36.0, 16.0) }
}

impl TableDelegate for RouteTimetableDisplayer<'_> {
    fn prepare(&mut self, info: &egui_table::PrefetchInfo) {
        self.trips.clear();
        let visible_start = info.visible_columns.start.max(2);
        let visible_end = info.visible_columns.end;
        if visible_start >= visible_end { return; }

        let trip_start = visible_start - 2;
        let trip_end = visible_end - 2;
        self.column_offset = trip_start;

        for i in trip_start..trip_end.min(self.available_trips.len()) {
            let tk = self.available_trips[i];
            let Some(view) = self.app.source.trips.get_view(tk) else { continue; };
            let entries = view.schedule.entries();
            let mut cells: Vec<CellKind> = vec![CellKind::Skipped; self.station_keys.len()];
            let mut entry_data: Vec<EntryData> = Vec::new();

            let mut next_abs_idx = 0usize;
            let mut si = 0usize;
            for entry in entries.iter() {
                let sk = match entry {
                    TEntry::Derived(s) => *s,
                    TEntry::Pinned { stn: s, .. } => *s,
                    TEntry::PinnedNonStop { stn: s, .. } => *s,
                    TEntry::PinnedExternalNonStop { stn: s, .. } => *s,
                    TEntry::PinnedExternal { .. } => continue,
                };
                // Advance stations iterator
                while si < self.station_keys.len() {
                    if self.station_keys[si] == sk {
                        let abs_idx = next_abs_idx;
                        if abs_idx < cells.len() {
                            cells[abs_idx] = CellKind::Stop(entry_data.len());
                            let (arr_mode, arr_time, dep_mode, dep_time) = match entry {
                                TEntry::Pinned { arr, dep, .. } => {
                                    let at = match arr { TravelMode::At(t) => Some(*t), _ => None };
                                    let dt = match dep {
                                        TravelMode::At(t) => *t,
                                        TravelMode::For(d) => at.map(|a| a + *d).unwrap_or(TimetableTime(0)),
                                        TravelMode::Flexible => TimetableTime(0),
                                    };
                                    (Some(*arr), at, *dep, dt)
                                }
                                TEntry::PinnedNonStop { pass, .. } => {
                                    let t = match pass { TravelMode::At(t) => *t, _ => TimetableTime(0) };
                                    (None, None, *pass, t)
                                }
                                _ => (None, None, TravelMode::Flexible, TimetableTime(0)),
                            };
                            entry_data.push(EntryData { arr_mode, arr_time, dep_mode, dep_time });
                        }
                        next_abs_idx = abs_idx + 1;
                        si += 1;
                        break;
                    } else {
                        si += 1;
                    }
                }
            }
            // Leading Skipped → NoOperation
            for c in cells.iter_mut().take_while(|c| matches!(c, CellKind::Skipped)) {
                *c = CellKind::NoOperation;
            }
            // Trailing Skipped → NoOperation, last → Terminated
            let mut last = None;
            for (i, c) in cells.iter_mut().enumerate().rev().take_while(|(_, c)| matches!(c, CellKind::Skipped)) {
                *c = CellKind::NoOperation;
                last = Some(i);
            }
            if let Some(idx) = last {
                cells[idx] = CellKind::Terminated;
            }
            self.trips.push(PreparedTrip { name: view.name.to_string(), cells, entry_data });
        }
    }
    fn row_top_offset(&self, _ctx: &egui::Context, _table_id: egui::Id, row_nr: u64) -> f32 {
        let offset_count: usize = self.route_display_modes[0..(row_nr as usize)]
            .iter()
            .map(|mode| if mode.arrival && mode.departure { 2 } else if mode.arrival || mode.departure { 1 } else { 0 })
            .sum();
        (offset_count as f32) * self.default_row_height()
    }
    fn default_row_height(&self) -> f32 { 16.0 }
    fn header_cell_ui(&mut self, ui: &mut Ui, cell: &egui_table::HeaderCellInfo) {
        let stroke = ui.visuals().window_stroke();
        let dy = vec2(0.0, -stroke.width);
        ui.painter().line_segment(
            [ui.max_rect().left_bottom() + dy, ui.max_rect().right_bottom() + dy],
            stroke,
        );
        if cell.group_index == 0 { ui.label(tr!("route-timetable-stations")); return; }
        let dx = vec2(-stroke.width, 0.0);
        ui.painter().line_segment(
            [ui.max_rect().right_top() + dx, ui.max_rect().right_bottom() + dx],
            stroke,
        );
        if cell.group_index == 1 { return; }
        let ti = cell.group_index - 2;
        let end = self.column_offset + self.trips.len();
        if ti < self.column_offset || ti >= end { return; }
        let local = ti - self.column_offset;
        ui.label(&self.trips[local].name);
    }
    fn cell_ui(&mut self, ui: &mut Ui, cell: &egui_table::CellInfo) {
        let row = cell.row_nr as usize;

        // Column 0: station name
        if cell.col_nr == 0 {
            let name = self.station_keys.get(row)
                .and_then(|sk| self.app.source.stations.query(*sk, |b| b.name.clone()))
                .unwrap_or_default();
            ui.allocate_ui_with_layout(
                ui.available_size(),
                Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(name.as_str()); },
            );
            return;
        }

        let dx = vec2(-ui.visuals().window_stroke.width, 0.0);
        ui.painter().line_segment(
            [ui.max_rect().right_top() + dx, ui.max_rect().right_bottom() + dx],
            ui.visuals().window_stroke(),
        );

        // Column 1: Arrival/Departure toggle
        if cell.col_nr == 1 {
            let prev_arr = row.checked_sub(1).map_or(false, |idx| self.route_display_modes[idx].arrival);
            let has_arr = self.route_display_modes[row].arrival;
            let has_dep = self.route_display_modes[row].departure;
            let prev_dep = !has_arr
                && row.checked_sub(1).map_or(false, |idx| self.route_display_modes[idx].departure);
            ui.vertical(|ui| {
                if has_arr {
                    let s = if prev_arr { "〃" } else { "Ａ" };
                    let res = ui.button(s);
                    egui::Popup::menu(&res).show(|ui| {
                        ui.add_enabled(!self.route_display_modes[row].arrival || self.route_display_modes[row].departure,
                            egui::Checkbox::new(&mut self.route_display_modes[row].arrival, "Arrival"));
                        ui.add_enabled(!self.route_display_modes[row].departure || self.route_display_modes[row].arrival,
                            egui::Checkbox::new(&mut self.route_display_modes[row].departure, "Departure"));
                    });
                }
                if has_dep {
                    let s = if prev_dep { "〃" } else { "Ｄ" };
                    let res = ui.button(s);
                    egui::Popup::menu(&res).show(|ui| {
                        ui.add_enabled(!self.route_display_modes[row].arrival || self.route_display_modes[row].departure,
                            egui::Checkbox::new(&mut self.route_display_modes[row].arrival, "Arrival"));
                        ui.add_enabled(!self.route_display_modes[row].departure || self.route_display_modes[row].arrival,
                            egui::Checkbox::new(&mut self.route_display_modes[row].departure, "Departure"));
                    });
                }
            });
            return;
        }

        // Columns 2+: trip times
        let display_mode = &self.route_display_modes[row];

        // Draw separation lines
        if display_mode.arrival {
            if display_mode.departure {
                ui.painter().line_segment(
                    [ui.max_rect().left_center(), ui.max_rect().right_center()],
                    ui.visuals().window_stroke(),
                );
            } else {
                let dy = vec2(0.0, -ui.visuals().window_stroke.width);
                ui.painter().line_segment(
                    [ui.max_rect().left_bottom() + dy, ui.max_rect().right_bottom() + dy],
                    ui.visuals().window_stroke(),
                );
            }
        }

        let ti = cell.col_nr - 2;
        let end = self.column_offset + self.trips.len();
        if ti < self.column_offset || ti >= end { return; }
        let local = ti - self.column_offset;
        let trip = &self.trips[local];
        if row >= trip.cells.len() { return; }
        let cell_kind = &trip.cells[row];

        ui.vertical(|ui| {
            if display_mode.arrival {
                let font = FontId::new(15.0, egui::FontFamily::Name("dia_pro".into()));
                let res = ui.put(
                    Rect::from_min_size(
                        ui.max_rect().left_top(),
                        Self::cell_size(),
                    ),
                    |ui: &mut Ui| match cell_kind {
                        CellKind::Skipped => ui.button(RichText::new("║").font(font)),
                        CellKind::NoOperation => ui.button(RichText::new("…").font(font)),
                        CellKind::Terminated => ui.button(RichText::new("▔").font(font)),
                        CellKind::Stop(ei) => {
                            let ed = &trip.entry_data[*ei];
                            match ed.arr_mode {
                                Some(TravelMode::At(t)) => {
                                    let mut new_t = t;
                                    ui.add(TimeDragValueOud(&mut new_t, false))
                                }
                                Some(TravelMode::Flexible) => ui.button(RichText::new("〇").font(font)),
                                Some(TravelMode::For(_)) => ui.button(RichText::new("For")),
                                None => ui.label(RichText::new("⇂").font(font)),
                            }
                        }
                    },
                );
                egui::Popup::menu(&res).show(|ui| { ui.label("Hi!"); });
            }
            if display_mode.departure {
                let font = FontId::new(15.0, egui::FontFamily::Name("dia_pro".into()));
                let res = ui.put(
                    Rect::from_min_size(
                        if display_mode.arrival { ui.max_rect().left_center() } else { ui.max_rect().left_top() },
                        Self::cell_size(),
                    ),
                    |ui: &mut Ui| match cell_kind {
                        CellKind::Skipped => ui.button(RichText::new("║").font(font)),
                        CellKind::NoOperation => ui.button(RichText::new("…").font(font)),
                        CellKind::Terminated => ui.button(RichText::new("▔").font(font)),
                        CellKind::Stop(ei) => {
                            let ed = &trip.entry_data[*ei];
                            match ed.dep_mode {
                                TravelMode::At(t) => {
                                    let mut new_t = t;
                                    ui.add(TimeDragValueOud(&mut new_t, false))
                                }
                                TravelMode::Flexible => ui.button(RichText::new("⇂").font(font)),
                                _ => ui.label(RichText::new("⇂").font(font)),
                            }
                        }
                    },
                );
                egui::Popup::menu(&res).show(|ui| { ui.label("Hi!"); });
            }
        });
    }
}
