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
        match (self, is_dark) {
            (Self::Red, true)       => Color32::from_rgb(248, 113, 113),
            (Self::Red, false)      => Color32::from_rgb(190, 18, 60),
            (Self::Orange, true)    => Color32::from_rgb(251, 146, 60),
            (Self::Orange, false)   => Color32::from_rgb(194, 65, 12),
            (Self::Amber, true)     => Color32::from_rgb(251, 191, 36),
            (Self::Amber, false)    => Color32::from_rgb(180, 83, 9),
            (Self::Yellow, true)    => Color32::from_rgb(250, 204, 21),
            (Self::Yellow, false)   => Color32::from_rgb(161, 98, 7),
            (Self::Lime, true)      => Color32::from_rgb(163, 230, 53),
            (Self::Lime, false)     => Color32::from_rgb(77, 124, 15),
            (Self::Green, true)     => Color32::from_rgb(74, 222, 128),
            (Self::Green, false)    => Color32::from_rgb(21, 128, 61),
            (Self::Emerald, true)   => Color32::from_rgb(52, 211, 153),
            (Self::Emerald, false)  => Color32::from_rgb(4, 120, 87),
            (Self::Teal, true)      => Color32::from_rgb(45, 212, 191),
            (Self::Teal, false)     => Color32::from_rgb(15, 118, 110),
            (Self::Cyan, true)      => Color32::from_rgb(34, 211, 238),
            (Self::Cyan, false)     => Color32::from_rgb(14, 116, 144),
            (Self::Sky, true)       => Color32::from_rgb(56, 189, 248),
            (Self::Sky, false)      => Color32::from_rgb(3, 105, 161),
            (Self::Blue, true)      => Color32::from_rgb(96, 165, 250),
            (Self::Blue, false)     => Color32::from_rgb(29, 78, 216),
            (Self::Indigo, true)    => Color32::from_rgb(129, 140, 248),
            (Self::Indigo, false)   => Color32::from_rgb(67, 56, 202),
            (Self::Violet, true)    => Color32::from_rgb(167, 139, 250),
            (Self::Violet, false)   => Color32::from_rgb(109, 40, 217),
            (Self::Purple, true)    => Color32::from_rgb(192, 132, 252),
            (Self::Purple, false)   => Color32::from_rgb(126, 34, 206),
            (Self::Fuchsia, true)   => Color32::from_rgb(232, 121, 249),
            (Self::Fuchsia, false)  => Color32::from_rgb(162, 28, 175),
            (Self::Pink, true)      => Color32::from_rgb(244, 114, 182),
            (Self::Pink, false)     => Color32::from_rgb(190, 24, 93),
            (Self::Rose, true)      => Color32::from_rgb(251, 113, 133),
            (Self::Rose, false)     => Color32::from_rgb(190, 18, 60),
            (Self::Slate, true)     => Color32::from_rgb(148, 163, 184),
            (Self::Slate, false)    => Color32::from_rgb(51, 65, 85),
            (Self::Gray, true)      => Color32::from_rgb(156, 163, 175),
            (Self::Gray, false)     => Color32::from_rgb(55, 65, 81),
            (Self::Zinc, true)      => Color32::from_rgb(161, 161, 170),
            (Self::Zinc, false)     => Color32::from_rgb(63, 63, 70),
            (Self::Neutral, true)   => Color32::from_rgb(163, 163, 163),
            (Self::Neutral, false)  => Color32::from_rgb(64, 64, 64),
            (Self::Stone, true)     => Color32::from_rgb(168, 162, 158),
            (Self::Stone, false)    => Color32::from_rgb(68, 64, 60),
        }
    }
}
