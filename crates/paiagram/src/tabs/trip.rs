use egui::{Color32, RectAlign, Ui, Vec2, WidgetText, vec2};
use egui_i18n::tr;
use paiagram_core::trip::{TEntry, TravelMode};
use paiagram_core::units::time::{Duration, TimetableTime};
use paiagram_core::TripKey;
use serde::{Deserialize, Serialize};

use super::{AppState, Tab};
use crate::widgets::timetable_popup::{
    arrival_popup_inner, departure_popup_inner, shift_at_value, shift_for_value,
};

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct TripTab {
    trip_key: TripKey,
}

impl TripTab {
    pub(crate) fn new(trip_key: TripKey) -> Self {
        Self { trip_key }
    }
}

impl Tab for TripTab {
    const NAME: &'static str = "Trip";
    fn title(&self) -> WidgetText {
        tr!("tab-trip").into()
    }
    fn main_display(&mut self, app: &mut AppState, ui: &mut egui::Ui) {
        let view = app.source.trips.get_view(self.trip_key);
        let Some(ref view) = view else {
            ui.label("Trip not found");
            return;
        };

        ui.heading(view.name.as_str());
        let entry_count = view.schedule.entries().len();
        ui.label(entry_count.to_string());

        let entries: Vec<&TEntry> = view.schedule.entries().iter().collect();
        let trip_key = self.trip_key;

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new(ui.id().with("trip_grid"))
                .num_columns(3)
                .striped(true)
                .show(ui, |ui| {
                    ui.label(tr!("trip-table-station"));
                    ui.label(tr!("trip-table-arrival"));
                    ui.label(tr!("trip-table-departure"));
                    ui.end_row();
                    for (idx, entry) in entries.iter().enumerate() {
                        let stn_name = get_station_name(app, entry);
                        ui.label(stn_name);

                        // Display arrival button with mode-specific rendering
                        ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                        let arr_res = match entry {
                            TEntry::Pinned { arr, .. } => match arr {
                                TravelMode::At(t) => shift_at_value(*t, trip_key, idx, app, ui, vec2(70.0, 18.0), true),
                                TravelMode::For(d) => shift_for_value(*d, trip_key, idx, app, ui, vec2(70.0, 18.0), true),
                                TravelMode::Flexible => ui.add_sized(vec2(70.0, 18.0), egui::Button::new("〇")),
                            },
                            _ => ui.add_sized(vec2(70.0, 18.0), egui::Button::new("↓")),
                        };
                        // Arrival popup (opens on click, handles DragValue conflict)
                        egui::Popup::menu(&arr_res)
                            .align(RectAlign::LEFT)
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .show(|ui| arrival_popup_inner(ui, idx, trip_key, app));

                        // Display departure button
                        let (dep_res, is_nonstop) = match &entry {
                            TEntry::PinnedNonStop { .. } => {
                                (ui.add_sized(vec2(70.0, 18.0), egui::Button::new("〇")), true)
                            }
                            TEntry::Pinned { dep, .. } => match dep {
                                TravelMode::At(t) => (shift_at_value(*t, trip_key, idx, app, ui, vec2(70.0, 18.0), false), false),
                                TravelMode::For(d) => (shift_for_value(*d, trip_key, idx, app, ui, vec2(70.0, 18.0), false), false),
                                TravelMode::Flexible => (ui.add_sized(vec2(70.0, 18.0), egui::Button::new("〇")), false),
                            },
                            _ => (ui.add_sized(vec2(70.0, 18.0), egui::Button::new("⇂")), false),
                        };
                        if !is_nonstop {
                            egui::Popup::menu(&dep_res)
                                .align(RectAlign::RIGHT)
                                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                                .show(|ui| departure_popup_inner(ui, idx, trip_key, app));
                        }

                        ui.end_row();
                    }
                });
        });
    }
}

fn get_station_name(app: &AppState, entry: &TEntry) -> String {
    let sk = match entry {
        TEntry::Derived(s) => *s,
        TEntry::Pinned { stn: s, .. } => *s,
        TEntry::PinnedNonStop { stn: s, .. } => *s,
        TEntry::PinnedExternalNonStop { stn: s, .. } => *s,
        TEntry::PinnedExternal { .. } => return "External".into(),
    };
    app.source.stations.query(sk, |b| b.name.clone())
        .unwrap_or_default()
        .to_string()
}
