// SPDX-License-Identifier: MPL-2.0
//! The settings for the app

use paiagram_core::time::TDuration;

use crate::widgets::DurationDragValue;

#[derive(Default, PartialEq)]
pub(crate) enum LevelOfDetailMode {
    #[default]
    X1,
    X2,
    X4,
}

/// Create the language definitions. See the call site in the source code for details.
macro_rules! make_lang {
    {
        $($lang_code:expr, $native_name:expr;)*
    } => { paste::paste! {
        /// The language of the application.
        #[derive(PartialEq, Clone, Copy)]
        pub(crate) enum AppLanguage {
            $( [<$lang_code:camel>], )*
        }
        impl std::fmt::Display for AppLanguage {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.lang_code())
            }
        }
        impl AppLanguage {
            fn lang_code(&self) -> &str {
                match *self {
                    $(Self::[<$lang_code:camel>] => $lang_code, )*
                }
            }
            fn native_name(&self) -> &str {
                match *self {
                    $(Self::[<$lang_code:camel>] => $native_name, )*
                }
            }
            const ALL: &[AppLanguage] = &[
                $( AppLanguage::[<$lang_code:camel>], )*
            ];
        }
    }};
}

make_lang! {
    "en-CA", "English (Canada)";
    "zh-Hans", "简体中文";
    "zh-Hans-x-en-CA-transliteration", "森普勒费艾德柴尼斯（喀内迪安英格利什特兰斯勒特勒申）";
    "zh-Hans-x-japanese-railway", "简体中文（日本交通术语）";
}

impl Default for AppLanguage {
    fn default() -> Self {
        Self::EnCa
    }
}

pub(crate) struct Preferences {
    pub dev_mode: bool,
    pub aa: bool,
    pub lod_mode: LevelOfDetailMode,
    pub language: AppLanguage,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            dev_mode: false,
            aa: true,
            lod_mode: LevelOfDetailMode::default(),
            language: AppLanguage::default(),
        }
    }
}

pub(crate) struct Settings {
    pub repeat_frequency: TDuration,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            repeat_frequency: TDuration::from_hms(24, 0, 0),
        }
    }
}

impl egui::Widget for &mut Preferences {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        egui::Grid::new("preferences grid")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Language");
                ui.horizontal_wrapped(|ui| {
                    for lang in AppLanguage::ALL {
                        ui.radio_value(&mut self.language, *lang, lang.native_name());
                    }
                });
                ui.end_row();
                ui.label("Current language code");
                ui.label(self.language.lang_code());
                ui.end_row();
                ui.label("Level of Detail");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.lod_mode, LevelOfDetailMode::X1, "1x");
                    ui.radio_value(&mut self.lod_mode, LevelOfDetailMode::X2, "2x");
                    ui.radio_value(&mut self.lod_mode, LevelOfDetailMode::X4, "4x");
                });
                ui.end_row();
                ui.label("Antialiasing");
                ui.checkbox(&mut self.aa, "");
                ui.end_row();
                ui.label("Developer Mode");
                ui.checkbox(&mut self.dev_mode, "");
                ui.end_row();
            })
            .response
    }
}

impl egui::Widget for &mut Settings {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        egui::Grid::new("settings grid")
            .num_columns(2)
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());
                ui.label("Repeat Frequency");
                ui.add(DurationDragValue(&mut self.repeat_frequency));
                ui.end_row();
            })
            .response
    }
}
