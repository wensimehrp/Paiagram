use bevy::color::palettes::tailwind::*;
use egui::Color32;

#[derive(Debug, Clone, Copy)]
pub enum DisplayColor {
    Predefined(PredefinedColor),
    Custom(Color32),
}

impl Default for DisplayColor {
    fn default() -> Self {
        Self::Predefined(PredefinedColor::Red)
    }
}

impl DisplayColor {
    fn get(self, light: bool) -> Color32 {
        match self {
            Self::Predefined(p) => p.get(light),
            Self::Custom(c) => c,
        }
    }
}

#[derive(Debug, Clone, Copy)]
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
    pub const fn get(self, light: bool) -> Color32 {
        #[rustfmt::skip]
        let c = match (self, light) {
            (Self::Red, true)       => RED_700,
            (Self::Red, false)      => RED_400,
            (Self::Orange, true)    => ORANGE_700,
            (Self::Orange, false)   => ORANGE_400,
            (Self::Amber, true)     => AMBER_700,
            (Self::Amber, false)    => AMBER_400,
            (Self::Yellow, true)    => YELLOW_700,
            (Self::Yellow, false)   => YELLOW_400,
            (Self::Lime, true)      => LIME_700,
            (Self::Lime, false)     => LIME_400,
            (Self::Green, true)     => GREEN_700,
            (Self::Green, false)    => GREEN_400,
            (Self::Emerald, true)   => EMERALD_700,
            (Self::Emerald, false)  => EMERALD_400,
            (Self::Teal, true)      => TEAL_700,
            (Self::Teal, false)     => TEAL_400,
            (Self::Cyan, true)      => CYAN_700,
            (Self::Cyan, false)     => CYAN_400,
            (Self::Sky, true)       => SKY_700,
            (Self::Sky, false)      => SKY_400,
            (Self::Blue, true)      => BLUE_700,
            (Self::Blue, false)     => BLUE_400,
            (Self::Indigo, true)    => INDIGO_700,
            (Self::Indigo, false)   => INDIGO_400,
            (Self::Violet, true)    => VIOLET_700,
            (Self::Violet, false)   => VIOLET_400,
            (Self::Purple, true)    => PURPLE_700,
            (Self::Purple, false)   => PURPLE_400,
            (Self::Fuchsia, true)   => FUCHSIA_700,
            (Self::Fuchsia, false)  => FUCHSIA_400,
            (Self::Pink, true)      => PINK_700,
            (Self::Pink, false)     => PINK_400,
            (Self::Rose, true)      => ROSE_700,
            (Self::Rose, false)     => ROSE_400,
            (Self::Slate, true)     => SLATE_700,
            (Self::Slate, false)    => SLATE_400,
            (Self::Gray, true)      => GRAY_700,
            (Self::Gray, false)     => GRAY_400,
            (Self::Zinc, true)      => ZINC_700,
            (Self::Zinc, false)     => ZINC_400,
            (Self::Neutral, true)   => NEUTRAL_700,
            (Self::Neutral, false)  => NEUTRAL_400,
            (Self::Stone, true)     => STONE_700,
            (Self::Stone, false)    => STONE_400,
        };
        Color32::from_rgba_unmultiplied_const(
            (c.red * 256.0) as u8,
            (c.green * 256.0) as u8,
            (c.blue * 256.0) as u8,
            (c.alpha * 256.0) as u8,
        )
    }
}
