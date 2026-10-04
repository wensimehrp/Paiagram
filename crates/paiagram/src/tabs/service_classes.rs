use egui::*;
use paiagram_core::{Key, ServiceClassKey};
use serde::{Deserialize, Serialize};

use super::Tab;
use crate::App;

#[derive(Default, Clone, Serialize, Deserialize)]
pub(crate) struct ServiceClassesTab {
    #[serde(skip)]
    selected_class: Option<ServiceClassKey>,
}

impl PartialEq for ServiceClassesTab {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Tab for ServiceClassesTab {
    const NAME: &'static str = "Service Classes";
    fn title(&self) -> WidgetText {
        Self::NAME.into()
    }
    fn main_display(&mut self, app: &mut App, ui: &mut Ui) {
        Panel::left(ui.id().with("service classes left panel")).frame(Frame::NONE).show(ui, |ui| {
            ScrollArea::vertical().auto_shrink(false).show_rows(
                ui,
                ui.spacing().interact_size.y,
                app.source.service_classes.len(),
                |ui, row_range| {
                    Frame::new().inner_margin(6.0).show(ui, |ui| {
                        ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                            for (&service_class_key, service_class) in &app
                                .source
                                .service_classes
                                .skip(row_range.start)
                                .take(row_range.count())
                            {
                                let service_class_str = service_class.name.as_str();
                                let atom_id = IdSalt::new(service_class_str);
                                let atom = Atom::custom(atom_id, Vec2::new(35.0, 15.0));
                                let button = egui::Button::selectable(
                                    self.selected_class == Some(service_class_key),
                                    (atom, service_class_str),
                                )
                                .truncate()
                                .atom_ui(ui);
                                if let Some(rect) = button.rect(atom_id) {
                                    let stroke = Stroke::new(
                                        service_class.style.width as f32 * 1.0,
                                        service_class.style.color,
                                    );
                                    ui.painter().line_segment(
                                        [rect.left_center(), rect.right_center()],
                                        stroke,
                                    );
                                    let stroke = Stroke::new(2.0, service_class.style.color);
                                    ui.painter().line_segment(
                                        [rect.left_top(), rect.left_bottom()],
                                        stroke,
                                    );
                                };
                                if button.clicked() {
                                    self.selected_class = Some(service_class_key);
                                }
                            }
                        });
                    });
                },
            );
        });
        CentralPanel::default_margins().show(ui, |ui| {
            let Some(selected_class_key) = self.selected_class else {
                ui.centered_and_justified(|ui| ui.heading("Nothing focused"));
                return;
            };
            let Some(selected_class) = app.source.service_classes.get(&selected_class_key) else {
                ui.centered_and_justified(|ui| ui.heading("Service class does not exist"));
                return;
            };
            ui.heading(selected_class.name.as_str());
            Grid::new(ui.id().with(selected_class.name.as_str())).num_columns(2).show(ui, |ui| {
                ui.label("Creation time");
                let time_string = jiff::Zoned::try_from(selected_class_key.creation_time())
                    .map_or("Unknown".into(), |zoned| {
                        format!("{}", zoned.strftime("%F %T %:z"))
                    });
                ui.label(time_string);
                ui.end_row();
            });
        });
    }
}
