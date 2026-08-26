use egui::{Frame, Margin, Rangef, ScrollArea, Ui};
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
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            StripBuilder::new(ui)
                .size(Size::remainder())
                .size(Size::exact(available.min(500.0)))
                .size(Size::remainder())
                .horizontal(|mut strip| {
                    strip.cell(|_ui| {});
                    strip.cell(|ui| {
                        Frame::new().inner_margin(Margin::symmetric(6, 24)).show(ui, |ui| {
                            ui.heading("Settings");
                            ui.add(&mut app.settings);
                            ui.add_space(30.0);
                            ui.heading("Preferences");
                            ui.add(&mut app.preferences);
                        });
                    });
                    strip.cell(|_ui| {});
                });
        });
    }
    fn title(&self) -> egui::WidgetText {
        tr!("tab-config").into()
    }
}
