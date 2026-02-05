use crate::{
    i18n::Language,
    settings::{ProjectSettings, UserPreferences},
};

use super::Tab;
use bevy::prelude::*;
use egui::Ui;
use egui_i18n::tr;
use moonshine_core::prelude::MapEntities;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

#[derive(Serialize, Deserialize, Clone, Default, MapEntities)]
pub struct SettingsTab;

impl PartialEq for SettingsTab {
    fn eq(&self, other: &Self) -> bool {
        true
    }
}

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
    ui.heading("Preferences");
    ui.checkbox(&mut preferences.dark_mode, "Dark Mode");
    egui::ComboBox::new(ui.id().with("settings box"), "Language").show_ui(ui, |ui| {
        for lang in Language::iter() {
            ui.selectable_value(&mut preferences.lang, lang, lang.name());
        }
    });
    ui.heading("Project Settings");
    ui.text_edit_multiline(&mut settings.remarks);
}
