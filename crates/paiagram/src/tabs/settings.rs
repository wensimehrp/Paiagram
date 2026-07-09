use egui::Ui;
use egui_i18n::tr;
use paiagram_core::i18n::Language;
use paiagram_core::settings::{AntialiasingMode, LevelOfDetailMode};
use serde::{Deserialize, Serialize};

use super::{AppState, Tab};

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub(crate) struct SettingsTab;

impl Tab for SettingsTab {
    const NAME: &'static str = "Settings";
    fn main_display(&mut self, app: &mut AppState, ui: &mut Ui) {
        ui.heading(tr!("settings-preferences"));
        egui::Grid::new("settings grid 1").show(ui, |ui| {
            ui.label(tr!("settings-dark-mode"));
            ui.checkbox(&mut app.preferences.dark_mode, "");
            ui.end_row();

            ui.label(tr!("settings-language"));
            ui.label(tr!("settings-language"));
            let prev_lang = app.preferences.lang.clone();
            egui::ComboBox::new("language", "")
                .selected_text(app.preferences.lang.clone())
                .show_ui(ui, |ui| {
                    for lang in Language::ALL {
                        let id = lang.identifier().to_string();
                        let mut selected = app.preferences.lang == id;
                        if ui.selectable_value(&mut selected, true, lang.name()).clicked() {
                            app.preferences.lang = id.clone();
                        }
                    }
                });
            if app.preferences.lang != prev_lang {
                egui_i18n::set_language(&app.preferences.lang);
            }
            ui.end_row();

            ui.label(tr!("settings-developer-mode"));
            ui.checkbox(&mut app.preferences.developer_mode, "");
            ui.end_row();

            ui.label(tr!("settings-antialiasing-options"));
            egui::ComboBox::new("antialiasing", "")
                .selected_text(match app.preferences.antialiasing_mode {
                    AntialiasingMode::Off => tr!("settings-off"),
                    AntialiasingMode::On => tr!("settings-on"),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut app.preferences.antialiasing_mode,
                        AntialiasingMode::Off,
                        tr!("settings-off"),
                    );
                    ui.selectable_value(
                        &mut app.preferences.antialiasing_mode,
                        AntialiasingMode::On,
                        tr!("settings-on"),
                    );
                });
            ui.end_row();

            ui.label(tr!("settings-lod-mode"));
            egui::ComboBox::new("lod", "")
                .selected_text(match app.preferences.level_of_detail_mode {
                    LevelOfDetailMode::Off => tr!("settings-off"),
                    LevelOfDetailMode::Lod2 => tr!("settings-lod-2x"),
                    LevelOfDetailMode::Lod4 => tr!("settings-lod-4x"),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut app.preferences.level_of_detail_mode,
                        LevelOfDetailMode::Off,
                        tr!("settings-off"),
                    );
                    ui.selectable_value(
                        &mut app.preferences.level_of_detail_mode,
                        LevelOfDetailMode::Lod2,
                        tr!("settings-lod-2x"),
                    );
                    ui.selectable_value(
                        &mut app.preferences.level_of_detail_mode,
                        LevelOfDetailMode::Lod4,
                        tr!("settings-lod-4x"),
                    );
                });
            ui.end_row();
        });
        ui.heading(tr!("settings-project-settings"));
        ui.text_edit_multiline(&mut app.project_settings.remarks);
    }
    fn title(&self) -> egui::WidgetText {
        tr!("tab-settings").into()
    }
}
