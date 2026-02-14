use bevy::color::{Srgba, palettes::tailwind::*};
use bevy::prelude::*;
use egui::Color32;
use egui::color_picker::{Alpha, color_picker_color32};
use egui_i18n::tr;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::{EnumCount, EnumIter};

#[derive(Reflect, Debug, Clone, Copy, Serialize, Deserialize)]
#[reflect(opaque, Serialize, Deserialize)]
pub enum DisplayColor {
    Predefined(PredefinedColor),
    Custom(Color32),
}

impl Default for DisplayColor {
    fn default() -> Self {
        Self::Predefined(PredefinedColor::Neutral)
    }
}

impl egui::Widget for &mut DisplayColor {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let is_dark = ui.visuals().dark_mode;
        let current_predefined = match *self {
            DisplayColor::Predefined(p) => Some(p),
            DisplayColor::Custom(_) => None,
        };

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label("Predefined");
                ui.set_max_width(200.0);
                ui.horizontal_wrapped(|ui| {
                    ui.style_mut().spacing.item_spacing = egui::Vec2::splat(4.0);
                    for predefined in PredefinedColor::iter() {
                        let color = predefined.get(is_dark);
                        let is_selected = current_predefined == Some(predefined);
                        let button = egui::Button::new("")
                            .fill(color)
                            .min_size(egui::vec2(24.0, 24.0))
                            .stroke(if is_selected {
                                ui.visuals().selection.stroke
                            } else {
                                ui.visuals().widgets.inactive.bg_stroke
                            });

                        if ui.add(button).clicked() {
                            *self = DisplayColor::Predefined(predefined);
                        }
                    }
                });
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.label("Custom");
                let mut custom_color = match *self {
                    DisplayColor::Custom(c) => c,
                    DisplayColor::Predefined(p) => p.get(is_dark),
                };
                if color_picker_color32(ui, &mut custom_color, Alpha::Opaque) {
                    *self = DisplayColor::Custom(custom_color);
                }
            });
        })
        .response
    }
}

impl DisplayColor {
    /// get the color as [`Color32`]
    pub fn get(self, is_dark: bool) -> Color32 {
        match self {
            Self::Predefined(p) => p.get(is_dark),
            Self::Custom(c) => c,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, EnumCount, Serialize, Deserialize)]
pub enum PredefinedColor {
    Red,
    Orange,
    Amber,
    Yellow,
    Lime,
    Green,
    Emerald,
    Teal,
    Cyan,
    Sky,
    Blue,
    Indigo,
    Violet,
    Purple,
    Fuchsia,
    Pink,
    Rose,
    Slate,
    Gray,
    Zinc,
    Neutral,
    Stone,
}

impl PredefinedColor {
    #[rustfmt::skip]
    pub fn name(self) -> impl AsRef<str> {
        match self {
            Self::Red       => tr!("colour-red"),
            Self::Orange    => tr!("colour-orange"),
            Self::Amber     => tr!("colour-amber"),
            Self::Yellow    => tr!("colour-yellow"),
            Self::Lime      => tr!("colour-lime"),
            Self::Green     => tr!("colour-green"),
            Self::Emerald   => tr!("colour-emerald"),
            Self::Teal      => tr!("colour-teal"),
            Self::Cyan      => tr!("colour-cyan"),
            Self::Sky       => tr!("colour-sky"),
            Self::Blue      => tr!("colour-blue"),
            Self::Indigo    => tr!("colour-indigo"),
            Self::Violet    => tr!("colour-violet"),
            Self::Purple    => tr!("colour-purple"),
            Self::Fuchsia   => tr!("colour-fuchsia"),
            Self::Pink      => tr!("colour-pink"),
            Self::Rose      => tr!("colour-rose"),
            Self::Slate     => tr!("colour-slate"),
            Self::Gray      => tr!("colour-gray"),
            Self::Zinc      => tr!("colour-zinc"),
            Self::Neutral   => tr!("colour-neutral"),
            Self::Stone     => tr!("colour-stone"),
        }
    }

    // use 700 shade if light, otherwise use 400
    // neutral is special
    pub const fn get(self, is_dark: bool) -> Color32 {
        #[rustfmt::skip]
        let c = match (self, is_dark) {
            (Self::Red, true)       => RED_400,
            (Self::Red, false)      => RED_700,
            (Self::Orange, true)    => ORANGE_400,
            (Self::Orange, false)   => ORANGE_700,
            (Self::Amber, true)     => AMBER_400,
            (Self::Amber, false)    => AMBER_700,
            (Self::Yellow, true)    => YELLOW_400,
            (Self::Yellow, false)   => YELLOW_700,
            (Self::Lime, true)      => LIME_400,
            (Self::Lime, false)     => LIME_700,
            (Self::Green, true)     => GREEN_400,
            (Self::Green, false)    => GREEN_700,
            (Self::Emerald, true)   => EMERALD_400,
            (Self::Emerald, false)  => EMERALD_700,
            (Self::Teal, true)      => TEAL_400,
            (Self::Teal, false)     => TEAL_700,
            (Self::Cyan, true)      => CYAN_400,
            (Self::Cyan, false)     => CYAN_700,
            (Self::Sky, true)       => SKY_400,
            (Self::Sky, false)      => SKY_700,
            (Self::Blue, true)      => BLUE_400,
            (Self::Blue, false)     => BLUE_700,
            (Self::Indigo, true)    => INDIGO_400,
            (Self::Indigo, false)   => INDIGO_700,
            (Self::Violet, true)    => VIOLET_400,
            (Self::Violet, false)   => VIOLET_700,
            (Self::Purple, true)    => PURPLE_400,
            (Self::Purple, false)   => PURPLE_700,
            (Self::Fuchsia, true)   => FUCHSIA_400,
            (Self::Fuchsia, false)  => FUCHSIA_700,
            (Self::Pink, true)      => PINK_400,
            (Self::Pink, false)     => PINK_700,
            (Self::Rose, true)      => ROSE_400,
            (Self::Rose, false)     => ROSE_700,
            (Self::Slate, true)     => SLATE_400,
            (Self::Slate, false)    => SLATE_700,
            (Self::Gray, true)      => GRAY_400,
            (Self::Gray, false)     => GRAY_700,
            (Self::Zinc, true)      => ZINC_400,
            (Self::Zinc, false)     => ZINC_700,
            (Self::Neutral, true)   => NEUTRAL_400,
            (Self::Neutral, false)  => NEUTRAL_700,
            (Self::Stone, true)     => STONE_400,
            (Self::Stone, false)    => STONE_700,
        };
        translate_srgba_to_color32(c)
    }
}

pub const fn translate_srgba_to_color32(c: Srgba) -> Color32 {
    Color32::from_rgba_unmultiplied_const(
        (c.red * 256.0) as u8,
        (c.green * 256.0) as u8,
        (c.blue * 256.0) as u8,
        (c.alpha * 256.0) as u8,
    )
}

/// Give the text colour that is readable given some background colour.
fn readable_text_color(color: Color32) -> Color32 {
    let [r, g, b, _] = color.to_array();
    let luma = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0;
    if luma > 0.5 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}
