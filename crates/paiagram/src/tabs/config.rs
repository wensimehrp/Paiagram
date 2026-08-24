use egui::{Frame, Margin, Rangef, Ui};
use egui_extras::{Size, StripBuilder};
use egui_i18n::tr;
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub(crate) struct ConfigTab;

impl Tab for ConfigTab {
    const NAME: &'static str = "Configuration";
    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        let available = ui.available_width();
        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::exact(available.min(400.0)))
            .size(Size::remainder())
            .horizontal(|mut strip| {
                strip.cell(|_ui| {});
                strip.cell(|ui| {
                    Frame::new().inner_margin(Margin::symmetric(6, 24)).show(ui, |ui| {
                        ui.heading("Settings");
                        ui.add(&mut app.settings);
                        ui.separator();
                        ui.heading("Preferences");
                        ui.add(&mut app.preferences);
                    });
                });
                strip.cell(|_ui| {});
            });
    }
    fn title(&self) -> egui::WidgetText {
        tr!("tab-config").into()
    }
}
