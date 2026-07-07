use egui::{Ui, WidgetText};
use egui_i18n::tr;
use paiagram_core::{TEntry, TripKey};
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct TripTab {
    trip: TripKey,
}

impl Tab for TripTab {
    const NAME: &'static str = "Trip";
    fn title(&self) -> WidgetText {
        tr!("tab-trip").into()
    }
    fn main_display(&mut self, app: &mut App, ui: &mut egui::Ui) {
        show_trip(self, app, ui);
    }
}

impl TripTab {
    pub(crate) fn new(trip: TripKey) -> Self {
        Self { trip }
    }
}

fn show_trip(tab: &mut TripTab, app: &mut App, ui: &mut Ui) {
    let Some(handle) = app.trips.get_handle(tab.trip) else {
        ui.label("Error!");
        return;
    };
    let name = app.trips.get_name(handle);
    let schedule = app.trips.get_entries(handle);
    ui.heading(name.as_str());
    ui.label(schedule.len().to_string());
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new(ui.id().with("trip_entries"))
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                ui.label(tr!("trip-table-station"));
                ui.label(tr!("trip-table-arrival"));
                ui.label(tr!("trip-table-departure"));
                ui.end_row();
                for entry in &schedule {
                    entry_row_ui(*entry, app, ui);
                    ui.end_row();
                }
            });
    });
}

fn entry_row_ui(entry: TEntry, app: &mut App, ui: &mut Ui) {
    match entry {
        TEntry::Pinned { stn, arr, dep, .. } => {
            // Station name
            if let Some(stn_handle) = app.stations.get_handle(stn) {
                ui.label(app.stations.get_name(stn_handle).as_str());
            } else {
                ui.label("???");
            }

            // Arrival
            match arr {
                paiagram_core::trip::TravelMode::At(t) => {
                    ui.label(t.to_string());
                }
                paiagram_core::trip::TravelMode::For(d) => {
                    ui.label(d.to_string());
                }
                paiagram_core::trip::TravelMode::Flexible => {
                    ui.label("〇");
                }
            }

            // Departure
            match dep {
                paiagram_core::trip::TravelMode::At(t) => {
                    ui.label(t.to_string());
                }
                paiagram_core::trip::TravelMode::For(d) => {
                    ui.label(d.to_string());
                }
                paiagram_core::trip::TravelMode::Flexible => {
                    ui.label("〇");
                }
            }
        }
        TEntry::PinnedNonStop { stn, pass, .. } => {
            if let Some(stn_handle) = app.stations.get_handle(stn) {
                ui.label(app.stations.get_name(stn_handle).as_str());
            } else {
                ui.label("???");
            }
            match pass {
                paiagram_core::trip::TravelMode::At(t) => {
                    ui.label(t.to_string());
                }
                paiagram_core::trip::TravelMode::For(d) => {
                    ui.label(d.to_string());
                }
                paiagram_core::trip::TravelMode::Flexible => {
                    ui.label("〇");
                }
            }
            ui.label("");
        }
        TEntry::PinnedExternalNonStop { stn, pass, .. } => {
            if let Some(stn_handle) = app.stations.get_handle(stn) {
                ui.label(app.stations.get_name(stn_handle).as_str());
            } else {
                ui.label("???");
            }
            match pass {
                paiagram_core::trip::TravelMode::At(t) => {
                    ui.label(t.to_string());
                }
                paiagram_core::trip::TravelMode::For(d) => {
                    ui.label(d.to_string());
                }
                paiagram_core::trip::TravelMode::Flexible => {
                    ui.label("〇");
                }
            }
            ui.label("");
        }
        TEntry::PinnedExternal { .. } => {
            ui.label(tr!("trip-table-exit"));
            ui.label("");
            ui.label("");
        }
        TEntry::Derived(_) => {
            ui.label("derived");
            ui.label("");
            ui.label("");
        }
    }
}
