use bevy::prelude::*;
use egui_i18n::*;
use strum_macros::EnumIter;

/// Languages
/// Sorted alphabetically
#[derive(Reflect, Clone, Copy, Debug, EnumIter, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    EnCA,
    JaJP,
    ZhHans,
}

impl Language {
    /// The native name of the language.
    pub fn name(self) -> &'static str {
        match self {
            Self::EnCA => "English (Canada)",
            Self::JaJP => "Japanese",
            Self::ZhHans => "中文（简体）",
        }
    }
    /// The identifier of the language.
    pub fn identifier(self) -> &'static str {
        match self {
            Self::EnCA => "en-CA",
            Self::JaJP => "ja-JP",
            Self::ZhHans => "zh-Hans",
        }
    }
}

pub fn init(default_language: Option<&str>) {
    let default_identifier = Language::default().identifier();
    load_translations_from_text(
        default_identifier,
        include_str!("../assets/locales/en-CA.ftl"),
    )
    .unwrap();
    load_translations_from_text("zh-Hans", include_str!("../assets/locales/zh-Hans.ftl")).unwrap();
    set_language(default_language.unwrap_or(default_identifier));
    set_fallback(default_identifier);
}
