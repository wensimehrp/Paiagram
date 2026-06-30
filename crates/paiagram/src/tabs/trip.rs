use egui::{Color32, RectAlign, Ui, Vec2, WidgetText, vec2};
use egui_i18n::tr;
use paiagram_core::{TEntry, TripKey};
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;
use crate::widgets::timetable_popup::{
    arrival_popup, departure_popup, shift_at_value, shift_for_value,
};

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
    ui.heading(name);
    ui.label(schedule.len().to_string());
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new(ui.id().with("lskdfjlsdkjflkdsjf"))
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                ui.label(tr!("trip-table-station"));
                ui.label(tr!("trip-table-arrival"));
                ui.label(tr!("trip-table-departure"));
                ui.end_row();
                for entry in schedule {
                    // row_ui(*entry, app, ui);
                    ui.label("123");
                    ui.end_row();
                }
            });
    });
}

// fn row_ui(entry: TEntry, app: &mut App, ui: &mut Ui) {
//     const BUTTON_SIZE: Vec2 = vec2(70.0, 18.0);
//     let platform = platform_q.get(it.stop()).unwrap();
//     let station = platform.station(&station_q);

//     // display station label
//     ui.label(station.name.as_str());

//     // Remove button background
//     ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
//     // display arrival button
//     let arr_res = match it.mode.arr {
//         None => ui.add_sized(BUTTON_SIZE, egui::Button::new("↓")),
//         Some(TravelMode::Flexible) => ui.add_sized(BUTTON_SIZE, egui::Button::new("〇")),
//         Some(TravelMode::At(t)) => shift_at_value(t, it.entity, ui, commands, BUTTON_SIZE, true),
//         Some(TravelMode::For(d)) => shift_for_value(d, it.entity, ui, commands, BUTTON_SIZE,
// true),     };
//     arrival_popup(
//         &arr_res,
//         &it,
//         &trip,
//         &entry_mode_q,
//         RectAlign::LEFT,
//         &mut commands,
//     );

//     // display departure button
//     let dep_res = match it.mode.dep {
//         TravelMode::Flexible => ui.add_sized(BUTTON_SIZE, egui::Button::new("〇")),
//         TravelMode::At(t) => shift_at_value(t, it.entity, ui, commands, BUTTON_SIZE, false),
//         TravelMode::For(d) => shift_for_value(d, it.entity, ui, commands, BUTTON_SIZE, false),
//     };
//     departure_popup(&dep_res, &it, RectAlign::RIGHT, &mut commands);
// }
