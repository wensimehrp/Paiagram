// SPDX-License-Identifier: MPL-2.0
//! The settings for the app

use std::sync::Arc;

use egui::ScrollArea;
use log::info;
use paiagram_core::time::TDuration;
use parking_lot::Mutex;

use crate::font::{FONT_DATABASE, load_default_font, load_font_to_egui};
use crate::widgets::DurationDragValue;
use crate::widgets::search::search;

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
        pub enum AppLanguage { $(
            #[doc = $native_name]
            [<$lang_code:camel>],
        )* }
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
            pub fn init() {
                $(
                    let content = include_str!(concat!(
                        "../assets/locales/",
                        $lang_code,
                        ".ftl",
                    ));
                    egui_i18n::load_translations_from_text($lang_code, content).unwrap_or_else(|e| panic!("{} {:?}", $lang_code, e));
                )*
                egui_i18n::set_language(Self::EnCa.lang_code());
                egui_i18n::set_fallback(Self::EnCa.lang_code());
                info!("Loaded translations");
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
    // "zh-Hans-x-japanese-railway", "简体中文（日本交通术语）";
    // "zh-Hans-x-en-CA-transliteration", "森普勒费艾德柴尼斯（喀内迪安英格利什特兰斯勒特勒申）";
    "zh-Hant", "繁體中文";
    // "zh-Latn-x-pinyin", "Zhongwen (Pinyin)";
    "ja-JP", "日本語";
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
    pub font_name: Arc<Mutex<String>>,
}

impl Preferences {
    pub fn new(ctx: &egui::Context) -> Self {
        AppLanguage::init();
        let font_name = Arc::new(Mutex::new(String::new()));
        let ret = Self {
            dev_mode: false,
            aa: true,
            lod_mode: LevelOfDetailMode::default(),
            language: AppLanguage::default(),
            font_name: font_name.clone(),
        };
        load_default_font(ctx.clone(), font_name.clone());
        ret
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
            .min_row_height(24.0)
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Language");
                egui::ComboBox::from_id_salt("language selection")
                    .selected_text(self.language.native_name())
                    .truncate()
                    .show_ui(ui, |ui| {
                        for lang in AppLanguage::ALL {
                            if ui
                                .selectable_value(&mut self.language, *lang, lang.native_name())
                                .changed()
                            {
                                egui_i18n::set_language(lang.lang_code());
                            }
                        }
                    });
                ui.end_row();
                ui.weak("(Language code)");
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
                ui.label("Font");
                egui::ComboBox::from_id_salt("font selection")
                    .selected_text(self.font_name.lock().as_str())
                    .height(300.0)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show_ui(ui, |ui| {
                        let query_id = ui.id().with("query text");
                        let candidate_id = ui.id().with("query candidates");
                        let mut query: String =
                            ui.data_mut(|w| w.remove_temp(query_id)).unwrap_or_default();
                        let mut matches: Vec<usize> =
                            ui.data_mut(|w| w.remove_temp(candidate_id)).unwrap_or_default();
                        if ui.text_edit_singleline(&mut query).changed() {
                            matches.clear();
                            matches.extend(search(
                                &query,
                                FONT_DATABASE
                                    .read()
                                    .faces()
                                    .map(|face| face.post_script_name.as_str()),
                                100,
                            ));
                        }
                        ScrollArea::vertical().show(ui, |ui| {
                            for (idx, face) in FONT_DATABASE.read().faces().enumerate() {
                                if let Err(_) = matches.binary_search(&idx) {
                                    continue;
                                }
                                let face_name = face.post_script_name.as_str();
                                if ui.button(face_name).clicked() {
                                    load_font_to_egui(
                                        face.id,
                                        ui.ctx().clone(),
                                        self.font_name.clone(),
                                        ui.fonts(|r| r.definitions().clone()),
                                    );
                                }
                            }
                        });
                        ui.data_mut(|w| w.insert_temp(query_id, query));
                        ui.data_mut(|w| w.insert_temp(candidate_id, matches));
                    });
            })
            .response
    }
}

impl egui::Widget for &mut Settings {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        egui::Grid::new("settings grid")
            .min_row_height(24.0)
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
