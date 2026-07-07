use egui::{FontId, WidgetText};
use egui_i18n::tr;
use paiagram_core::trip::TEntry;
use paiagram_core::RouteKey;
use serde::{Deserialize, Serialize};

use crate::App;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct RouteTimetableTab {
    route: RouteKey,
}

impl RouteTimetableTab {
    pub(crate) fn new(route: RouteKey) -> Self {
        Self { route }
    }
}

impl super::Tab for RouteTimetableTab {
    const NAME: &'static str = "Route Timetable";
    fn title(&self) -> WidgetText {
        tr!("tab-route-timetable").into()
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false; 2]
    }
    fn main_display(&mut self, app: &mut App, ui: &mut egui::Ui) {
        let Some(route_handle) = app.routes.get_handle(self.route) else {
            ui.label("Route not found");
            return;
        };
        let name = app.routes.get_name(route_handle);
        ui.heading(name.as_str());
        let stations = app.routes.get_stations(route_handle);

        // Collect trips that visit these stations in order
        let mut trips_on_route: Vec<(String, Vec<Option<String>>)> = Vec::new();

        for (trip_key, _) in app.trips_iter() {
            let Some(trip_handle) = app.trips.get_handle(trip_key) else {
                continue;
            };
            let trip_name = app.trips.get_name(trip_handle);
            let entries = app.trips.get_entries(trip_handle);

            // Check if this trip visits stations along this route
            let mut times: Vec<Option<String>> = Vec::new();
            let mut entry_idx = 0;
            for stn_key in stations.iter() {
                // Scan forward through entries to find this station
                while entry_idx < entries.len() {
                    match &entries[entry_idx] {
                        TEntry::Pinned { stn, arr, dep, .. } if *stn == *stn_key => {
                            let arr_str = match arr {
                                paiagram_core::trip::TravelMode::At(t) => t.to_string(),
                                paiagram_core::trip::TravelMode::For(d) => d.to_string(),
                                paiagram_core::trip::TravelMode::Flexible => "〇".to_string(),
                            };
                            times.push(Some(arr_str));
                            entry_idx += 1;
                            break;
                        }
                        TEntry::PinnedNonStop { stn, .. } if *stn == *stn_key => {
                            times.push(None);
                            entry_idx += 1;
                            break;
                        }
                        TEntry::PinnedExternal { .. } => {
                            // Exiting route, no more stations match
                            times.push(None);
                            break;
                        }
                        _ => {
                            entry_idx += 1;
                        }
                    }
                }
                if entry_idx >= entries.len() {
                    break;
                }
            }

            if times.iter().any(|t| t.is_some()) {
                trips_on_route.push((trip_name.to_string(), times));
            }
        }

        // Display as a scrollable table
        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let font_id = FontId::proportional(14.0);

                // Header row: trip names
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Station");
                        for stn_key in stations.iter() {
                            let stn_handle = app.stations.get_handle(*stn_key);
                            let stn_name = stn_handle
                                .map(|h| app.stations.get_name(h))
                                .unwrap_or_default();
                            ui.label(
                                egui::RichText::new(stn_name.as_str())
                                    .font(font_id.clone()),
                            );
                        }
                    });
                    for (trip_name, times) in &trips_on_route {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(trip_name.as_str())
                                    .font(font_id.clone()),
                            );
                            for t in times {
                                match t {
                                    Some(s) => {
                                        ui.label(
                                            egui::RichText::new(s.as_str())
                                                .font(font_id.clone()),
                                        );
                                    }
                                    None => {
                                        ui.label("");
                                    }
                                }
                            }
                        });
                    }
                });
            });
    }
}
