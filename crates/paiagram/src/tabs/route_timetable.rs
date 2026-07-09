use egui::{FontId, Layout, Rect, RichText, Ui, Vec2, WidgetText, vec2};
use egui_i18n::tr;
use egui_table::{Column, Table, TableDelegate};
use paiagram_core::trip::{TEntry, TravelMode};
use paiagram_core::RouteKey;
use serde::{Deserialize, Serialize};

use super::{AppState, Tab};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct RouteTimetableTab {
    route_key: RouteKey,
}

impl RouteTimetableTab {
    pub(crate) fn new(rk: RouteKey) -> Self {
        Self { route_key: rk }
    }
}

impl super::Tab for RouteTimetableTab {
    const NAME: &'static str = "Route Timetable";
    fn title(&self) -> WidgetText {
        tr!("tab-route-timetable").into()
    }
    fn scroll_bars(&self) -> [bool; 2] { [false; 2] }
    fn main_display(&mut self, app: &mut AppState, ui: &mut egui::Ui) {
        let route_view = app.source.routes.get_view(self.route_key);
        let Some(ref route) = route_view else {
            ui.label("Route not found");
            return;
        };

        let stations: Vec<_> = route.stations.iter().copied().collect();
        let station_names: Vec<String> = stations.iter()
            .map(|sk| app.source.stations.query(*sk, |b| b.name.clone()).unwrap_or_default().to_string())
            .collect();

        // Collect trips that reference stations on this route
        let mut route_trips: Vec<paiagram_core::TripKey> = Vec::new();
        for tk in app.source.trips.keys() {
            if let Some(view) = app.source.trips.get_view(*tk) {
                let uses_route_station = view.schedule.entries().iter().any(|entry| {
                    let sk = match entry {
                        TEntry::Derived(s) => *s,
                        TEntry::Pinned { stn: s, .. } => *s,
                        TEntry::PinnedNonStop { stn: s, .. } => *s,
                        TEntry::PinnedExternalNonStop { stn: s, .. } => *s,
                        TEntry::PinnedExternal { .. } => return false,
                    };
                    stations.contains(&sk)
                });
                if uses_route_station {
                    route_trips.push(*tk);
                }
            }
        }

        let table = egui_table::Table::new()
            .id_salt(ui.id().with("route_timetable"))
            .num_rows(stations.len() as u64)
            .num_sticky_cols(2);

        let mut displayer = RouteTimetableDisplayer {
            station_names: &station_names,
            station_keys: &stations,
            trip_keys: &route_trips,
            app,
        };
        let dia_pro_style = egui::TextStyle::Name("dia_pro".into());
        ui.style_mut().text_styles.insert(
            dia_pro_style.clone(),
            egui::FontId::new(15.0, egui::FontFamily::Name("dia_pro".into())),
        );
        ui.style_mut().drag_value_text_style = dia_pro_style;
        ui.spacing_mut().interact_size = Vec2::ZERO;
        ui.spacing_mut().button_padding = Vec2::ZERO;
        ui.style_mut().visuals.button_frame = false;
        table
            .columns(
                std::iter::once(Column::new(80.0).resizable(true))
                    .chain(std::iter::once(Column::new(20.0).resizable(false)))
                    .chain((0..route_trips.len()).map(|_| {
                        Column::new(36.0).resizable(false)
                    }))
                    .collect::<Vec<_>>(),
            )
            .show(ui, &mut displayer);
    }
}

struct RouteTimetableDisplayer<'a> {
    station_names: &'a [String],
    station_keys: &'a [paiagram_core::StationKey],
    trip_keys: &'a [paiagram_core::TripKey],
    app: &'a mut AppState,
}

impl RouteTimetableDisplayer<'_> {
    fn table_cell_width() -> f32 { 36.0 }
    fn cell_size() -> Vec2 { vec2(36.0, 16.0) }
}

impl TableDelegate for RouteTimetableDisplayer<'_> {
    fn default_row_height(&self) -> f32 { 16.0 }
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::HeaderCellInfo) {
        let dy = vec2(0.0, -ui.visuals().window_stroke.width);
        ui.painter().line_segment(
            [ui.max_rect().left_bottom() + dy, ui.max_rect().right_bottom() + dy],
            ui.visuals().window_stroke(),
        );
        if cell.group_index == 0 {
            ui.label(tr!("route-timetable-stations"));
            return;
        }
        if cell.group_index == 1 { return; }
        let trip_index = cell.group_index - 2;
        if trip_index < self.trip_keys.len() {
            if let Some(view) = self.app.source.trips.get_view(self.trip_keys[trip_index]) {
                ui.label(view.name.as_str());
            }
        }
    }
    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &egui_table::CellInfo) {
        let row_nr = cell.row_nr as usize;
        if cell.col_nr == 0 {
            if row_nr < self.station_names.len() {
                ui.allocate_ui_with_layout(
                    ui.available_size(),
                    Layout::left_to_right(egui::Align::Center),
                    |ui| { ui.label(&self.station_names[row_nr]); },
                );
            }
            return;
        }
        if cell.col_nr == 1 { return; }
        let trip_index = cell.col_nr - 2;
        if trip_index >= self.trip_keys.len() { return; }
        let tk = self.trip_keys[trip_index];
        let view = match self.app.source.trips.get_view(tk) {
            Some(v) => v,
            None => return,
        };
        let sk = self.station_keys[row_nr];
        // Find entry for this station
        for entry in view.schedule.entries() {
            let entry_sk = match entry {
                TEntry::Derived(s) => *s == sk,
                TEntry::Pinned { stn: s, .. } => *s == sk,
                TEntry::PinnedNonStop { stn: s, .. } => *s == sk,
                TEntry::PinnedExternalNonStop { stn: s, .. } => *s == sk,
                TEntry::PinnedExternal { .. } => false,
            };
            if !entry_sk { continue; }
            let font = FontId::new(15.0, egui::FontFamily::Name("dia_pro".into()));
            let time_str = match entry {
                TEntry::Pinned { arr: TravelMode::At(a), dep: TravelMode::At(d), .. } => {
                    format!("{}\n{}", a.to_oud2_str(false), d.to_oud2_str(false))
                }
                TEntry::Pinned { arr: TravelMode::At(a), .. } => a.to_oud2_str(false),
                TEntry::PinnedNonStop { pass: TravelMode::At(t), .. } => t.to_oud2_str(false),
                _ => "".into(),
            };
            ui.label(RichText::new(time_str).font(font));
            break;
        }
    }
}
