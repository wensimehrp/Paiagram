use bevy::ecs::system::{InMut, ResMut};
use egui::Ui;
use egui_i18n::{set_language, tr};
use strum::IntoEnumIterator;

use super::Tab;
use crate::settings::{ApplicationSettings, Language, TerminologyScheme};

#[derive(PartialEq, Debug, Clone, Copy)]
pub struct SettingsTab;

impl Tab for SettingsTab {
    const NAME: &'static str = "Settings";
    fn main_display(&mut self, world: &mut bevy::ecs::world::World, ui: &mut Ui) {
        if let Err(e) = world.run_system_cached_with(show_settings, ui) {
            bevy::log::error!("UI Error while displaying settings page: {}", e)
        }
    }
    fn title(&self) -> egui::WidgetText {
        tr!("tab-settings").into()
    }
}

fn show_settings(InMut(ui): InMut<Ui>, mut settings: ResMut<ApplicationSettings>) {
    ui.checkbox(
        &mut settings.enable_romaji_search,
        tr!("settings-enable-romaji-search"),
    );
    ui.checkbox(
        &mut settings.show_performance_stats,
        tr!("settings-show-performance-stats"),
    );
    egui::ComboBox::from_id_salt("Language")
        .selected_text(settings.language.name())
        .show_ui(ui, |ui| {
            for lang in Language::iter() {
                if ui
                    .selectable_value(&mut settings.language, lang, lang.name())
                    .changed()
                {
                    set_language(lang.identifier());
                }
            }
        });
    egui::ComboBox::from_id_salt("Terminology scheme")
        .selected_text(settings.terminology_scheme.name())
        .show_ui(ui, |ui| {
            for scheme in TerminologyScheme::iter() {
                ui.selectable_value(&mut settings.terminology_scheme, scheme, scheme.name());
            }
        });
}
