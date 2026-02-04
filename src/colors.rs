use bevy::color::{Srgba, palettes::tailwind::*};
use bevy::prelude::*;
use egui::Color32;
use serde::{Deserialize, Serialize};
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

impl DisplayColor {
    /// get the color as [`Color32`]
    pub fn get(self, is_dark: bool) -> Color32 {
        match self {
            Self::Predefined(p) => p.get(is_dark),
            Self::Custom(c) => c,
        }
    }
}

#[derive(Debug, Clone, Copy, EnumIter, EnumCount, Serialize, Deserialize)]
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
