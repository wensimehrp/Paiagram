use bevy::ecs::system::{InMut, ResMut};
use egui::Ui;
use strum::IntoEnumIterator;

use crate::settings::{ApplicationSettings, Language, TerminologyScheme};

pub fn show_settings(InMut(ui): InMut<Ui>, mut settings: ResMut<ApplicationSettings>) {
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
