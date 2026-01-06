pub use crate::i18n::Language;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

pub struct SettingsPlugin;
impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ApplicationSettings::default());
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, EnumIter, PartialEq, Eq)]
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug, EnumIter, PartialEq, Eq)]
pub enum PinyinScheme {
    Sogou,
    Microsoft,
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
