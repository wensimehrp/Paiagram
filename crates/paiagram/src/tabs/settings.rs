use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use egui::Ui;
use egui_i18n::tr;
use paiagram_core::i18n::Language;
use paiagram_core::settings::{
    AntialiasingMode, LevelOfDetailMode, ProjectSettings, UserPreferences,
};
use serde::{Deserialize, Serialize};

use super::Tab;

#[derive(Serialize, Deserialize, Clone, Default, MapEntities, PartialEq)]
pub(crate) struct SettingsTab;

impl Tab for SettingsTab {
    const NAME: &'static str = "Settings";
    fn main_display(&mut self, world: &mut World, ui: &mut Ui) {
        if let Err(e) = world.run_system_cached_with(show_settings, ui) {
            bevy::log::error!("UI Error while displaying settings page: {}", e)
        }
    }
    fn title(&self) -> egui::WidgetText {
        tr!("tab-settings").into()
    }
}

fn show_settings(
    InMut(ui): InMut<Ui>,
    mut preferences: ResMut<UserPreferences>,
    mut settings: ResMut<ProjectSettings>,
) {
    ui.heading(tr!("settings-preferences"));
    egui::Grid::new("settings grid 1").show(ui, |ui| {
        ui.label(tr!("settings-dark-mode"));
        ui.checkbox(&mut preferences.dark_mode, "");
        ui.end_row();

        ui.label(tr!("settings-language"));
        egui::ComboBox::new("language", "")
            .selected_text(preferences.lang.name())
            .show_ui(ui, |ui| {
                for lang in Language::ALL {
                    ui.selectable_value(&mut preferences.lang, *lang, lang.name());
                }
            });
        ui.end_row();

        ui.label(tr!("settings-developer-mode"));
        ui.checkbox(&mut preferences.developer_mode, "");
        ui.end_row();

        ui.label(tr!("settings-antialiasing-options"));
        egui::ComboBox::new("antialiasing", "")
            .selected_text(match preferences.antialiasing_mode {
                AntialiasingMode::Off => tr!("settings-off"),
                AntialiasingMode::On => tr!("settings-on"),
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut preferences.antialiasing_mode,
                    AntialiasingMode::Off,
                    tr!("settings-off"),
                );
                ui.selectable_value(
                    &mut preferences.antialiasing_mode,
                    AntialiasingMode::On,
                    tr!("settings-on"),
                );
            });
        ui.end_row();

        ui.label(tr!("settings-lod-mode"));
        egui::ComboBox::new("lod", "")
            .selected_text(match preferences.level_of_detail_mode {
                LevelOfDetailMode::Off => tr!("settings-off"),
                LevelOfDetailMode::Lod2 => tr!("settings-lod-2x"),
                LevelOfDetailMode::Lod4 => tr!("settings-lod-4x"),
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut preferences.level_of_detail_mode,
                    LevelOfDetailMode::Off,
                    tr!("settings-off"),
                );
                ui.selectable_value(
                    &mut preferences.level_of_detail_mode,
                    LevelOfDetailMode::Lod2,
                    tr!("settings-lod-2x"),
                );
                ui.selectable_value(
                    &mut preferences.level_of_detail_mode,
                    LevelOfDetailMode::Lod4,
                    tr!("settings-lod-4x"),
                );
            });
        ui.end_row();
    });
    ui.heading(tr!("settings-project-settings"));
    ui.text_edit_multiline(&mut settings.remarks);
}
