use bevy::{
    ecs::system::{InMut, Local},
    log::info,
};
use egui::{Rect, Ui, UiBuilder};

pub fn display_start(InMut(ui): InMut<Ui>, mut widget_info: Local<Option<Rect>>) {
    ui.vertical_centered(|ui| {
        ui.scope_builder(
            UiBuilder {
                sizing_pass: widget_info.is_none(),
                ..Default::default()
            },
            |ui| {
                ui.set_max_width(600.0f32.min(ui.available_width()));
                if !ui.is_sizing_pass() {
                    let amnt =
                        (ui.max_rect().height() / 2.0) - (widget_info.unwrap().height() / 2.0);
                    ui.add_space(amnt);
                }
                ui.columns(2, |columns| {
                    columns[0].vertical(|ui| {
                        ui.heading("Paiagram");
                        ui.label("Dispatching Journeys");
                        ui.add_space(10.0);
                        ui.heading("Start");
                        if ui.link("New Diagram...").clicked() {};
                        if ui.link("Open Diagram...").clicked() {};
                        if ui.link("Import Diagram...").clicked() {};
                        ui.add_space(10.0);
                        ui.heading("Recent");
                    });
                    columns[1].vertical(|ui| {
                        ui.heading("External Resources");
                        ui.label("Looking for external resources? Checkout these groups!");
                        ui.add_space(10.0);
                        if ui.link("Matrix Chat Room").clicked() {};
                        if ui.link("GitHub Repository").clicked() {};
                        if ui.link("QQ").clicked() {};
                    });
                });
                if ui.is_sizing_pass() {
                    *widget_info = Some(ui.min_rect());
                }
            },
        )
    });
}
