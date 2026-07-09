// SPDX-License-Identifier: MPL-2.0
//! Internationalization support.

use egui_i18n::{load_translations_from_text, set_fallback, set_language};

/// Languages
/// Sorted alphabetically
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    EnCA,
    JaJP,
    ZhHans,
}

impl Language {
    pub const ALL: &[Self] = &[Self::EnCA, Self::JaJP, Self::ZhHans];

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

/// Initialize the i18n system. Must be called once at startup.
pub fn init() {
    let default = Language::default();
    load_translations_from_text(
        default.identifier(),
        include_str!("../../paiagram/assets/locales/en-CA.ftl"),
    )
    .unwrap();
    load_translations_from_text(
        "zh-Hans",
        include_str!("../../paiagram/assets/locales/zh-Hans.ftl"),
    )
    .unwrap();
    set_language(default.identifier());
    set_fallback(default.identifier());
}
