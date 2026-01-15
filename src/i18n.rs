use bevy::prelude::*;
use egui_i18n::*;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

/// Languages
/// Sorted alphabetically
#[derive(Reflect, Clone, Copy, Debug, EnumIter, PartialEq, Eq)]
pub enum Language {
    EnCA,
    ZhHans,
    JaJP,
}

impl Language {
    pub fn name(self) -> &'static str {
        match self {
            Self::EnCA => "English (Canada)",
            Self::ZhHans => "中文（简体）",
            Self::JaJP => "Japanese",
        }
    }
    pub fn identifier(self) -> &'static str {
        match self {
            Self::EnCA => "en-CA",
            Self::ZhHans => "zh-Hans",
            Self::JaJP => "ja-JP",
        }
    }
}

pub fn init(default_language: Option<&str>) {
    load_translations_from_text("en-CA", include_str!("../assets/locales/en-CA.ftl")).unwrap();
    load_translations_from_text("zh-Hans", include_str!("../assets/locales/zh-Hans.ftl")).unwrap();
    set_language(default_language.unwrap_or("en-CA"));
    set_fallback("en-CA");
}
