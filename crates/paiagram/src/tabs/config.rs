use egui::{Frame, Ui};
use egui_i18n::tr;
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub(crate) struct ConfigTab;

impl Tab for ConfigTab {
    const NAME: &'static str = "Configuration";
    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        Frame::new().inner_margin(6.0).show(ui, |ui| {
            ui.heading("Settings");
            ui.add(&mut app.settings);
            ui.separator();
            ui.heading("Preferences");
            ui.add(&mut app.preferences);
        });
    }
    fn title(&self) -> egui::WidgetText {
        tr!("tab-config").into()
    }
}
