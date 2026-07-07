use egui::{ScrollArea, Ui, WidgetText};
use egui_i18n::tr;
use paiagram_core::colors::PredefinedColor;
use paiagram_core::{ClassKey, TripKey};
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;

#[derive(Default, PartialEq, Clone, Serialize, Deserialize)]
pub(crate) struct ClassesTab {
    #[serde(skip)]
    selected_class: Option<ClassKey>,
    #[serde(skip)]
    hovered_trip: Option<TripKey>,
}

impl Tab for ClassesTab {
    const NAME: &'static str = "Classes";
    fn title(&self) -> WidgetText {
        tr!("tab-classes").into()
    }
    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        let classes: Vec<ClassKey> = app.classes.keys().copied().collect();

        if classes.is_empty() {
            ui.label("No classes defined");
            return;
        }

        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                egui::Grid::new("class grid")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(tr!("classes-name"));
                        ui.label(tr!("classes-count"));
                        ui.label(tr!("classes-color"));
                        ui.end_row();

                        for &class_key in &classes {
                            let Some(handle) = app.classes.get_handle(class_key) else {
                                continue;
                            };
                            let name = app.classes.get_name(handle);
                            let view = app
                                .classes
                                .get_view(class_key)
                                .expect("class exists");

                            // Count trips using this class
                            let count = app
                                .trips_iter()
                                .into_iter()
                                .filter(|(tk, _)| app.trips.get_view(*tk).map_or(false, |v| v.class == Some(class_key)))
                                .count();

                            ui.selectable_value(
                                &mut self.selected_class,
                                Some(class_key),
                                name.as_str(),
                            );
                            ui.label(count.to_string());

                            let color = egui::Color32::from_gray(
                                if matches!(self.selected_class, Some(c) if c == class_key) {
                                    200
                                } else {
                                    128
                                },
                            );
                            ui.label(
                                egui::RichText::new("■■■").color(color),
                            );
                            ui.end_row();
                        }
                    });
            });
    }
}
