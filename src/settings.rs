use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub struct SettingsPlugin;
impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ApplicationSettings::default());
    }
}

#[derive(Serialize, Deserialize)]
pub enum TerminologyScheme {
    Paiagram,
    ChineseRailway,
    JapaneseRailway,
}

/// Languages
/// Sorted alphabetically
#[derive(Serialize, Deserialize)]
pub enum Language {
    EnCA,
    ZhCN,
}

#[derive(Resource, Serialize, Deserialize)]
pub struct ApplicationSettings {
    pub enable_romaji_search: bool,
    pub pinyin_scheme: Vec<String>,
    pub terminology_scheme: TerminologyScheme,
    pub language: Language,
}

impl Default for ApplicationSettings {
    fn default() -> Self {
        Self {
            enable_romaji_search: false,
            pinyin_scheme: vec!["quanpin".into(), "diletter_microsoft".into()],
            terminology_scheme: TerminologyScheme::Paiagram,
            language: Language::EnCA,
        }
    }
}
