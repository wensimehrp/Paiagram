use egui::Ui;
use egui_i18n::tr;
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub(crate) struct StartTab;

impl Tab for StartTab {
    const NAME: &'static str = "Start";
    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        ui.heading(tr!("program-name"));
        egui::Grid::new("start info grid")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label(tr!("tab-start-amount-vehicles"));
                ui.label(app.vehicles.len().to_string());
                ui.end_row();
                ui.label(tr!("tab-start-amount-trips"));
                ui.label(app.trips.len().to_string());
                ui.end_row();
                ui.label(tr!("tab-start-amount-stations"));
                ui.label(app.stations.len().to_string());
                ui.end_row();
                ui.label(tr!("tab-start-amount-intervals"));
                ui.label(app.intervals.len().to_string());
                ui.end_row();
            });
    }
    fn title(&self) -> egui::WidgetText {
        tr!("tab-start").into()
    }
    fn scroll_bars(&self) -> [bool; 2] {
        [false, true]
    }
}
