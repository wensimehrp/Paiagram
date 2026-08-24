use egui::{Button, Color32, Popup, RectAlign, RichText, Ui, Vec2, WidgetText, vec2};
use egui_i18n::tr;
use paiagram_core::TripKey;
use paiagram_core::trip::TravelMode::Flexible;
use paiagram_core::trip::{TEntry, TravelMode};
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;
use crate::widgets::{DurationDragValue, TimeDragValue};
// use crate::widgets::timetable_popup::{
//     arrival_popup, departure_popup, shift_at_value, shift_for_value,
// };

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub(crate) struct TripTab {
    trip: TripKey,
    show_derived: bool,
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
        Self {
            trip,
            show_derived: false,
        }
    }
}

fn show_trip(tab: &mut TripTab, app: &mut App, ui: &mut Ui) {
    let Some((name, schedule)) =
        app.trips.query(tab.trip, |view| (view.name.clone(), view.schedule.clone()))
    else {
        return;
    };
    egui::Frame::new().inner_margin(6.0).show(ui, |ui| {
        ui.heading(name.as_str());
        ui.label(schedule.entries().len().to_string());
    });
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        egui::Frame::new().inner_margin(6.0).show(ui, |ui| {
            egui::Grid::new(ui.id().with("trip ui")).num_columns(2).striped(true).show(ui, |ui| {
                ui.label(tr!("trip-table-station"));
                ui.label(tr!("trip-table-arrival"));
                ui.end_row();
                // Remove button background
                ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                for (idx, entry) in schedule.entries().iter().enumerate() {
                    row_ui(*entry, app, ui, idx);
                    ui.end_row();
                }
            });
        });
    });
}

fn row_ui(entry: TEntry, app: &mut App, ui: &mut Ui, idx: usize) {
    const BUTTON_SIZE: Vec2 = vec2(70.0, 18.0);
    let Some(station) = app.nodes.query(entry.node_key(), |view| *view.parent) else {
        ui.label("No station");
        return;
    };
    app.stations.query(station, |view| {
        ui.label(view.name.as_str());
    });
    let mut wide_size = BUTTON_SIZE;
    wide_size.x *= 2.0;
    wide_size.x += ui.spacing().item_spacing.x;
    let (first_res, second_res) = ui
        .horizontal(|ui| match entry {
            TEntry::Derived { .. } => (
                None,
                ui.add_sized(wide_size, Button::new(RichText::new("(↓)").weak())),
            ),
            TEntry::Pinned { arr, dep, .. } => (
                Some(match arr {
                    TravelMode::For(mut d) => ui.add_sized(BUTTON_SIZE, DurationDragValue(&mut d)),
                    TravelMode::At(mut t) => ui.add_sized(BUTTON_SIZE, TimeDragValue(&mut t)),
                    Flexible => ui.add_sized(BUTTON_SIZE, Button::new("〇")),
                }),
                match dep {
                    TravelMode::For(mut d) => ui.add_sized(BUTTON_SIZE, DurationDragValue(&mut d)),
                    TravelMode::At(mut t) => ui.add_sized(BUTTON_SIZE, TimeDragValue(&mut t)),
                    Flexible => ui.add_sized(BUTTON_SIZE, Button::new("〇")),
                },
            ),
            TEntry::PinnedNonStop { pass, .. } => (
                None,
                match pass {
                    TravelMode::For(mut d) => ui.add_sized(wide_size, DurationDragValue(&mut d)),
                    TravelMode::At(mut t) => ui.add_sized(wide_size, TimeDragValue(&mut t)),
                    Flexible => ui.add_sized(wide_size, Button::new("↓")),
                },
            ),
        })
        .inner;
    Popup::menu(&second_res).align(RectAlign::RIGHT).show(|ui| {});
    let Some(first_res) = first_res else {
        return;
    };
    Popup::menu(&first_res).align(RectAlign::LEFT).show(|ui| {
        ui.button("Flexible");
    });
}
