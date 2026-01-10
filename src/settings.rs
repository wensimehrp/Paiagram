pub use crate::i18n::Language;
use bevy::prelude::*;
use egui_i18n::tr;
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Author {
    Unknown,
    OpenStreetMapContributors,
    Lead(String),
    Contributor(String),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AuthorList(pub Vec<Author>);

#[derive(Resource, Serialize, Deserialize)]
pub struct ApplicationSettings {
    pub enable_romaji_search: bool,
    pub show_performance_stats: bool,
    pub pinyin_scheme: Vec<String>,
    pub terminology_scheme: TerminologyScheme,
    pub language: Language,
    pub authors: AuthorList,
    pub remarks: String,
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
        }
    }
}
