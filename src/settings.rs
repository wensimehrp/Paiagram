use crate::i18n::Language;
use bevy::prelude::*;
use egui_i18n::{set_language, tr};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

pub struct SettingsPlugin;
impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ApplicationSettings::default())
            .add_systems(
                Update,
                update_language_setting.run_if(resource_changed_or_removed::<ApplicationSettings>),
            );
    }
}

/// Update the application's language when the language setting changes
/// This function should only be called when the ApplicationSettings resource changes
/// using Bevy's [`resource_changed_or_removed`] condition.
fn update_language_setting(settings: Res<ApplicationSettings>) {
    set_language(settings.language.identifier());
}

/// Different terminology schemes for railway terms
#[derive(Reflect, Clone, Copy, Debug, EnumIter, PartialEq, Eq)]
pub enum TerminologyScheme {
    Paiagram,
    ChineseRailway,
    JapaneseRailway,
}

impl TerminologyScheme {
    pub fn name(self) -> &'static str {
        match self {
            Self::Paiagram => "Paiagram",
            Self::ChineseRailway => "Chinese Railway",
            Self::JapaneseRailway => "Japanese Railway",
        }
    }
}

#[derive(Reflect, Clone, Copy, Debug, EnumIter, PartialEq, Eq)]
pub enum PinyinScheme {
    Sogou,
    Microsoft,
}

#[derive(Reflect, Clone, Debug)]
pub enum Author {
    Unknown,
    OpenStreetMapContributors,
    Lead(String),
    Contributor(String),
}

#[derive(Reflect, Clone, Debug)]
pub struct AuthorList(pub Vec<Author>);

/// TODO: Extract user preferences from this list
#[derive(Reflect, Resource)]
#[reflect(Resource)]
pub struct ApplicationSettings {
    pub enable_romaji_search: bool,
    pub show_performance_stats: bool,
    pub pinyin_scheme: Vec<String>,
    pub terminology_scheme: TerminologyScheme,
    pub language: Language,
    pub authors: AuthorList,
    pub remarks: String,
    pub autosave_enabled: bool,
    pub autosave_interval_minutes: u32,
}

impl Default for ApplicationSettings {
    fn default() -> Self {
        Self {
            enable_romaji_search: false,
            show_performance_stats: false,
            pinyin_scheme: vec!["quanpin".into(), "diletter_microsoft".into()],
            terminology_scheme: TerminologyScheme::Paiagram,
            language: Language::EnCA,
            authors: AuthorList(if cfg!(target_arch = "wasm32") {
                vec![Author::Unknown]
            } else {
                vec![
                    std::env::var("USER")
                        .or_else(|_| std::env::var("USERNAME"))
                        .map(Author::Lead)
                        .unwrap_or(Author::Unknown),
                ]
            }),
            remarks: String::new(),
            autosave_enabled: true,
            autosave_interval_minutes: 5,
        }
    }
}

impl egui::Widget for &mut ApplicationSettings {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.checkbox(
            &mut self.enable_romaji_search,
            tr!("settings-enable-romaji-search"),
        );
        ui.checkbox(
            &mut self.show_performance_stats,
            tr!("settings-show-performance-stats"),
        );
        egui::ComboBox::from_id_salt("Language")
            .selected_text(self.language.name())
            .show_ui(ui, |ui| {
                for lang in Language::iter() {
                    ui.selectable_value(&mut self.language, lang, lang.name());
                }
            });
        egui::ComboBox::from_id_salt("Terminology scheme")
            .selected_text(self.terminology_scheme.name())
            .show_ui(ui, |ui| {
                for scheme in TerminologyScheme::iter() {
                    ui.selectable_value(&mut self.terminology_scheme, scheme, scheme.name());
                }
            });
        ui.text_edit_multiline(&mut self.remarks);
        ui.checkbox(&mut self.autosave_enabled, tr!("settings-enable-autosave"));
        ui.add(
            egui::Slider::new(&mut self.autosave_interval_minutes, 1..=10)
                .text(tr!("settings-autosave-interval")),
        )
    }
}
