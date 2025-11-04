use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub struct SettingsPlugin;
impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Settings::default());
    }
}

#[derive(Serialize, Deserialize)]
pub enum TerminologyScheme {
    Paiagram,
    Chinese,
    Japanese,
}

#[derive(Resource, Serialize, Deserialize)]
pub struct Settings {
    pub enable_romaji_search: bool,
    pub pinyin_scheme: Vec<String>,
    pub terminology_scheme: TerminologyScheme,
    pub language: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enable_romaji_search: false,
            pinyin_scheme: vec!["quanpin".into(), "diletter_microsoft".into()],
            terminology_scheme: TerminologyScheme::Paiagram,
            language: None,
        }
    }
}
