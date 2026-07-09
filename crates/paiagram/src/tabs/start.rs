use egui::Ui;
use egui_i18n::tr;
use serde::{Deserialize, Serialize};

use super::{AppState, Tab};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub(crate) struct StartTab;

impl Tab for StartTab {
    const NAME: &'static str = "Start";
    fn main_display(&mut self, app: &mut AppState, ui: &mut Ui) {
        ui.heading(tr!("program-name"));
        egui::Grid::new("start info grid")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label(tr!("tab-start-amount-vehicles"));
                ui.label(app.source.vehicles.len().to_string());
                ui.end_row();
                ui.label(tr!("tab-start-amount-trips"));
                ui.label(app.source.trips.len().to_string());
                ui.end_row();
                ui.label(tr!("tab-start-amount-stations"));
                ui.label(app.source.stations.len().to_string());
                ui.end_row();
                ui.label(tr!("tab-start-amount-routes"));
                ui.label(app.source.routes.len().to_string());
                ui.end_row();
                ui.label(tr!("tab-start-amount-intervals"));
                ui.label(app.source.intervals.len().to_string());
            });
        if ui.button(tr!("tab-start-merge-stations-by-name")).clicked() {
            // TODO: implement station merge
        }
    }
    fn title(&self) -> egui::WidgetText {
        tr!("tab-start").into()
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false, true]
    }
}
