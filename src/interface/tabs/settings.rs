use bevy::ecs::system::{InMut, ResMut};
use egui::Ui;
use strum::IntoEnumIterator;

use crate::settings::{ApplicationSettings, Language, TerminologyScheme};
use super::Tab;

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct SettingsTab;

impl Tab for SettingsTab {
    const NAME: &'static str = "Settings";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        if let Err(e) = world.run_system_cached_with(show_settings, ui) {
            bevy::log::error!("UI Error while displaying settings page: {}", e)
        }
    }
}

fn show_settings(InMut(ui): InMut<Ui>, mut settings: ResMut<ApplicationSettings>) {
    ui.checkbox(&mut settings.enable_romaji_search, "Enable Romaji search");
    ui.checkbox(
        &mut settings.show_performance_stats,
        "Show performance analytics",
    );
    egui::ComboBox::from_label("Language")
        .selected_text(settings.language.name())
        .show_ui(ui, |ui| {
            for lang in Language::iter() {
                ui.selectable_value(&mut settings.language, lang, lang.name());
            }
        });
    egui::ComboBox::from_label("Terminology scheme")
        .selected_text(settings.terminology_scheme.name())
        .show_ui(ui, |ui| {
            for scheme in TerminologyScheme::iter() {
                ui.selectable_value(&mut settings.terminology_scheme, scheme, scheme.name());
            }
        });
}
