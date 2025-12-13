use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub struct SettingsPlugin;
impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ApplicationSettings::default());
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum TerminologyScheme {
    Paiagram,
    ChineseRailway,
    JapaneseRailway,
}

impl TerminologyScheme {
    pub const ALL: &[Self] = &[Self::Paiagram, Self::ChineseRailway, Self::JapaneseRailway];
    pub fn name(self) -> &'static str {
        match self {
            Self::Paiagram => "Paiagram",
            Self::ChineseRailway => "Chinese Railway",
            Self::JapaneseRailway => "Japanese Railway",
        }
    }
}

/// Languages
/// Sorted alphabetically
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum Language {
    EnCA,
    ZhCN,
}

impl Language {
    pub const ALL: &[Self] = &[Self::EnCA, Self::ZhCN];
    pub fn name(self) -> &'static str {
        match self {
            Self::EnCA => "English (Canada)",
            Self::ZhCN => "中文（简体）",
        }
    }
}

#[derive(Resource, Serialize, Deserialize)]
pub struct ApplicationSettings {
    pub enable_romaji_search: bool,
    pub show_performance_stats: bool,
    pub pinyin_scheme: Vec<String>,
    pub terminology_scheme: TerminologyScheme,
    pub language: Language,
}

impl Default for ApplicationSettings {
    fn default() -> Self {
        Self {
            enable_romaji_search: false,
            show_performance_stats: false,
            pinyin_scheme: vec!["quanpin".into(), "diletter_microsoft".into()],
            terminology_scheme: TerminologyScheme::Paiagram,
            language: Language::EnCA,
        }
    }
}
