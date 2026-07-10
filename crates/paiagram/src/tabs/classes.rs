use egui::{Button, Layout, Panel, ScrollArea, Ui, WidgetText, vec2};
use egui_i18n::tr;
use paiagram_core::ClassKey;
use serde::{Deserialize, Serialize};

use super::{AppState, Tab};
use crate::MainTab;
use crate::tabs::trip::TripTab;

#[derive(Default, PartialEq, Clone, Serialize, Deserialize)]
pub(crate) struct ClassesTab {
    #[serde(skip)]
    selected_class: Option<ClassKey>,
}

impl Tab for ClassesTab {
    const NAME: &'static str = "Classes";
    fn title(&self) -> WidgetText {
        tr!("tab-classes").into()
    }
    fn main_display(&mut self, app: &mut AppState, ui: &mut Ui) {
        Panel::right(ui.id().with("first"))
            .default_size(ui.available_width() / 3.0)
            .resizable(true)
            .show_inside(ui, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.label("Trips");
                    let Some(class_key) = self.selected_class else {
                        return;
                    };
                    let class_view = app.source.classes.get_view(class_key);
                    let Some(_view) = class_view else {
                        return;
                    };
                    ui.with_layout(Layout::default().with_cross_justify(true), |ui| {
                        // Show trips with this class
                        for (tk, name) in app.source.trips_iter() {
                            let has_class = app.source.trips.query(tk, |b| *b.class == Some(class_key)).unwrap_or(false);
                            if !has_class { continue; }
                            let res = ui.button(name.as_str());
                            if res.clicked() {
                                app.pending_tabs.push_back(crate::PendingTabOp::Open(MainTab::Trip(TripTab::new(tk))));
                            }
                        }
                    });
                });
            });

        let mut itoa_buffer = itoa::Buffer::new();
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            egui::Grid::new("class grid")
                .num_columns(3)
                .striped(true)
                .show(ui, |ui| {
                    ui.label(tr!("classes-name"));
                    ui.label(tr!("classes-count"));
                    ui.label(tr!("classes-color"));
                    ui.end_row();
                    for ck in app.source.classes.keys() {
                        let view = app.source.classes.get_view(*ck);
                        let Some(ref view) = view else { continue; };
                        ui.allocate_ui_with_layout(
                            vec2(200.0, 24.0),
                            Layout::default().with_cross_justify(true),
                            |ui| {
                                let mut is_sel = self.selected_class == Some(*ck);
                                if ui.selectable_value(&mut is_sel, true, view.name.as_str()).clicked() {
                                    self.selected_class = Some(*ck);
                                }
                                if !is_sel && self.selected_class == Some(*ck) {
                                    self.selected_class = None;
                                }
                            },
                        );
                        let printed = itoa_buffer.format(count_trips_with_class(app, *ck));
                        ui.label(printed);
                        let mut displayed_color: paiagram_core::colors::DisplayedColor = 
                            paiagram_core::colors::DisplayedColor::Custom(view.style.color);
                        ui.add(&mut displayed_color);
                        ui.end_row();
                    }
                });
        });
    }
}

fn count_trips_with_class(app: &AppState, class_key: ClassKey) -> usize {
    app.source.trips.keys().filter(|tk| {
        app.source.trips.query(**tk, |b| *b.class == Some(class_key)).unwrap_or(false)
    }).count()
}
