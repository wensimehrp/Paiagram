use egui::{Color32, Ui, Vec2, WidgetText, vec2};
use egui_i18n::tr;
use paiagram_core::TripKey;
use paiagram_core::trip::TEntry;
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;
// use crate::widgets::timetable_popup::{
//     arrival_popup, departure_popup, shift_at_value, shift_for_value,
// };

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
    let Some((name, schedule)) =
        app.trips.query(tab.trip, |view| (view.name.clone(), view.schedule.clone()))
    else {
        return;
    };
    egui::Frame::new().inner_margin(6.0).show(ui, |ui| {
        ui.heading(name.as_str());
        ui.label(schedule.entries().len().to_string());
    });
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Frame::new().inner_margin(6.0).show(ui, |ui| {
            egui::Grid::new(ui.id().with("trip ui")).num_columns(3).striped(true).show(ui, |ui| {
                ui.label(tr!("trip-table-station"));
                ui.label(tr!("trip-table-arrival"));
                ui.label(tr!("trip-table-departure"));
                ui.end_row();
                // Remove button background
                ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                for entry in schedule.entries() {
                    row_ui(*entry, app, ui);
                    ui.end_row();
                }
            });
        });
    });
}

fn row_ui(entry: TEntry, app: &mut App, ui: &mut Ui) {
    const BUTTON_SIZE: Vec2 = vec2(70.0, 18.0);
    let Some(station) = app.nodes.query(entry.node_key(), |view| *view.parent) else {
        ui.label("No station");
        return;
    };
    app.stations.query(station, |view| {
        ui.label(view.name.as_str());
    });
    ui.add_sized(BUTTON_SIZE, egui::Button::new("↓"));
    ui.add_sized(BUTTON_SIZE, egui::Button::new("↓"));
    // // display arrival button
    // let arr_res = match it.mode.arr {
    //     None => ui.add_sized(BUTTON_SIZE, egui::Button::new("↓")),
    //     Some(TravelMode::Flexible) => ui.add_sized(BUTTON_SIZE, egui::Button::new("〇")),
    //     Some(TravelMode::At(t)) => shift_at_value(t, it.entity, ui, commands, BUTTON_SIZE, true),
    //     Some(TravelMode::For(d)) => shift_for_value(d, it.entity, ui, commands, BUTTON_SIZE,
    // true), };
    // arrival_popup(
    //     &arr_res,
    //     &it,
    //     &trip,
    //     &entry_mode_q,
    //     RectAlign::LEFT,
    //     &mut commands,
    // );

    // // display departure button
    // let dep_res = match it.mode.dep {
    //     TravelMode::Flexible => ui.add_sized(BUTTON_SIZE, egui::Button::new("〇")),
    //     TravelMode::At(t) => shift_at_value(t, it.entity, ui, commands, BUTTON_SIZE, false),
    //     TravelMode::For(d) => shift_for_value(d, it.entity, ui, commands, BUTTON_SIZE, false),
    // };
    // departure_popup(&dep_res, &it, RectAlign::RIGHT, &mut commands);
}
