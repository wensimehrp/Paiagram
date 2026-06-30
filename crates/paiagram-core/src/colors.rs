//! The color definitions.

use egui::Color32;
use egui::color_picker::{Alpha, color_picker_color32, show_color_at};
use serde::{Deserialize, Serialize};

/// A color displayed in the application. This is used for stations, intervals, and trip classes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DisplayedColor {
    /// A predefined colour
    Predefined(PredefinedColor),
    /// A custom colour defined using egui's [`egui::Color32`]
    Custom(Color32),
}

impl DisplayedColor {
    /// Generate a displayed color from a seed. The process is not randomized. The seed could be
    /// anything that can be converted to [u8], e.g., a string.
    pub fn from_seed(data: impl AsRef<[u8]>) -> Self {
        let bytes = data.as_ref();
        let mut sum = 0u8;
        for byte in bytes.iter().copied() {
            sum = sum.wrapping_add(byte);
        }
        Self::Predefined(PredefinedColor::from_index(sum as usize))
    }
}

impl Default for DisplayedColor {
    fn default() -> Self {
        Self::Predefined(PredefinedColor::Neutral)
    }
}

// this is copied from egui
fn color_button(ui: &mut egui::Ui, color: Color32, open: bool) -> egui::Response {
    let size = ui.spacing().interact_size;
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::new(egui::WidgetType::ColorButton));

    if ui.is_rect_visible(rect) {
        let visuals = if open {
            &ui.visuals().widgets.open
        } else {
            ui.style().interact(&response)
        };
        let rect = rect.expand(visuals.expansion);

        let stroke_width = 1.0;
        show_color_at(ui.painter(), color, rect.shrink(stroke_width));

        let corner_radius = visuals.corner_radius.at_most(2); // Can't do more rounding because the background grid doesn't do any rounding
        ui.painter().rect_stroke(
            rect,
            corner_radius,
            (stroke_width, visuals.bg_fill), /* Using fill for stroke is intentional, because
                                              * default style has no
                                              * border */
            egui::StrokeKind::Inside,
        );
    }

    response
}

impl egui::Widget for &mut DisplayedColor {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let is_dark = ui.visuals().dark_mode;
        let button_res = color_button(ui, self.into_color32(is_dark), false);

        let current_predefined = match *self {
            DisplayedColor::Predefined(p) => Some(p),
            DisplayedColor::Custom(_) => None,
        };

        egui::Popup::menu(&button_res)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Predefined");
                        ui.set_max_width(200.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.style_mut().spacing.item_spacing = egui::Vec2::splat(4.0);
                            for predefined in PredefinedColor::ALL {
                                let color = predefined.into_color32(is_dark);
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
                                    *self = DisplayedColor::Predefined(predefined);
                                }
                            }
                        });
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.label("Custom");
                        let mut custom_color = match *self {
                            DisplayedColor::Custom(c) => c,
                            DisplayedColor::Predefined(p) => p.into_color32(is_dark),
                        };
                        if color_picker_color32(ui, &mut custom_color, Alpha::Opaque) {
                            *self = DisplayedColor::Custom(custom_color);
                        }
                    });
                })
            });
        button_res
    }
}

impl DisplayedColor {
    /// get the color as [`egui::Color32`]
    pub fn into_color32(self, is_dark: bool) -> Color32 {
        match self {
            Self::Predefined(p) => p.into_color32(is_dark),
            Self::Custom(c) => c,
        }
    }
}

/// Tailwind CSS predefined colors used in the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    pub const ALL: [Self; 22] = [
        Self::Red,
        Self::Orange,
        Self::Amber,
        Self::Yellow,
        Self::Lime,
        Self::Green,
        Self::Emerald,
        Self::Teal,
        Self::Cyan,
        Self::Sky,
        Self::Blue,
        Self::Indigo,
        Self::Violet,
        Self::Purple,
        Self::Fuchsia,
        Self::Pink,
        Self::Rose,
        Self::Slate,
        Self::Gray,
        Self::Zinc,
        Self::Neutral,
        Self::Stone,
    ];

    /// Select a color given the index. The index could be any number.
    pub fn from_index(i: usize) -> Self {
        Self::ALL[i % Self::ALL.len()]
    }

    /// Get the color given the current UI theme. Returns the lighter 400 varation if the theme is
    /// dark, and returns the (usually) darker 700 variation if the theme is light.
    pub const fn into_color32(self, is_dark: bool) -> Color32 {
        #[rustfmt::skip]
        let c = match (self, is_dark) {
            (Self::Red, true)       => todo!(), // RED_400,
            (Self::Red, false)      => todo!(), // RED_700,
            (Self::Orange, true)    => todo!(), // ORANGE_400,
            (Self::Orange, false)   => todo!(), // ORANGE_700,
            (Self::Amber, true)     => todo!(), // AMBER_400,
            (Self::Amber, false)    => todo!(), // AMBER_700,
            (Self::Yellow, true)    => todo!(), // YELLOW_400,
            (Self::Yellow, false)   => todo!(), // YELLOW_700,
            (Self::Lime, true)      => todo!(), // LIME_400,
            (Self::Lime, false)     => todo!(), // LIME_700,
            (Self::Green, true)     => todo!(), // GREEN_400,
            (Self::Green, false)    => todo!(), // GREEN_700,
            (Self::Emerald, true)   => todo!(), // EMERALD_400,
            (Self::Emerald, false)  => todo!(), // EMERALD_700,
            (Self::Teal, true)      => todo!(), // TEAL_400,
            (Self::Teal, false)     => todo!(), // TEAL_700,
            (Self::Cyan, true)      => todo!(), // CYAN_400,
            (Self::Cyan, false)     => todo!(), // CYAN_700,
            (Self::Sky, true)       => todo!(), // SKY_400,
            (Self::Sky, false)      => todo!(), // SKY_700,
            (Self::Blue, true)      => todo!(), // BLUE_400,
            (Self::Blue, false)     => todo!(), // BLUE_700,
            (Self::Indigo, true)    => todo!(), // INDIGO_400,
            (Self::Indigo, false)   => todo!(), // INDIGO_700,
            (Self::Violet, true)    => todo!(), // VIOLET_400,
            (Self::Violet, false)   => todo!(), // VIOLET_700,
            (Self::Purple, true)    => todo!(), // PURPLE_400,
            (Self::Purple, false)   => todo!(), // PURPLE_700,
            (Self::Fuchsia, true)   => todo!(), // FUCHSIA_400,
            (Self::Fuchsia, false)  => todo!(), // FUCHSIA_700,
            (Self::Pink, true)      => todo!(), // PINK_400,
            (Self::Pink, false)     => todo!(), // PINK_700,
            (Self::Rose, true)      => todo!(), // ROSE_400,
            (Self::Rose, false)     => todo!(), // ROSE_700,
            (Self::Slate, true)     => todo!(), // SLATE_400,
            (Self::Slate, false)    => todo!(), // SLATE_700,
            (Self::Gray, true)      => todo!(), // GRAY_400,
            (Self::Gray, false)     => todo!(), // GRAY_700,
            (Self::Zinc, true)      => todo!(), // ZINC_400,
            (Self::Zinc, false)     => todo!(), // ZINC_700,
            (Self::Neutral, true)   => todo!(), // NEUTRAL_400,
            (Self::Neutral, false)  => todo!(), // NEUTRAL_700,
            (Self::Stone, true)     => todo!(), // STONE_400,
            (Self::Stone, false)    => todo!(), // STONE_700,
        };
        c
    }
}
