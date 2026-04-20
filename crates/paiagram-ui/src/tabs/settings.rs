use paiagram_core::{
    i18n::Language,
    settings::{AntialiasingMode, LevelOfDetailMode, ProjectSettings, UserPreferences},
};

use super::Tab;
use bevy::prelude::*;
use egui::Ui;
use egui_i18n::tr;
use bevy::ecs::entity::MapEntities;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

#[derive(Serialize, Deserialize, Clone, Default, MapEntities, PartialEq)]
pub struct SettingsTab;

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
                for lang in Language::iter() {
                    ui.selectable_value(&mut preferences.lang, lang, lang.name());
                }
            });
        ui.end_row();

        ui.label("Developer Mode");
        ui.checkbox(&mut preferences.developer_mode, "");
        ui.end_row();

        ui.label("Antialising Options");
        egui::ComboBox::new("antialiasing", "")
            .selected_text(match preferences.antialiasing_mode {
                AntialiasingMode::Off => "Off",
                AntialiasingMode::On => "On",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut preferences.antialiasing_mode,
                    AntialiasingMode::Off,
                    "Off",
                );
                ui.selectable_value(
                    &mut preferences.antialiasing_mode,
                    AntialiasingMode::On,
                    "On",
                );
            });
        ui.end_row();

        ui.label("LOD Mode");
        egui::ComboBox::new("lod", "")
            .selected_text(match preferences.level_of_detail_mode {
                LevelOfDetailMode::Off => "Off",
                LevelOfDetailMode::Lod2 => "2×",
                LevelOfDetailMode::Lod4 => "4×",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut preferences.level_of_detail_mode,
                    LevelOfDetailMode::Off,
                    "Off",
                );
                ui.selectable_value(
                    &mut preferences.level_of_detail_mode,
                    LevelOfDetailMode::Lod2,
                    "2×",
                );
                ui.selectable_value(
                    &mut preferences.level_of_detail_mode,
                    LevelOfDetailMode::Lod4,
                    "4×",
                );
            });
        ui.end_row();
    });
    ui.heading(tr!("settings-project-settings"));
    ui.text_edit_multiline(&mut settings.remarks);
}
