// SPDX-License-Identifier: MPL-2.0

use crate::colors::{DisplayedColor, PredefinedColor};

/// The stroke style used for rendering a trip class.
#[derive(Debug, Clone, Copy)]
pub struct DisplayedStroke {
    pub color: DisplayedColor,
    pub width: f32,
}

impl Default for DisplayedStroke {
    fn default() -> Self {
        Self {
            color: DisplayedColor::Predefined(PredefinedColor::Emerald),
            width: 1.0,
        }
    }
}

impl DisplayedStroke {
    pub fn from_seed(data: impl AsRef<[u8]>) -> Self {
        Self {
            color: DisplayedColor::from_seed(data),
            width: 1.0,
        }
    }

    pub fn egui_stroke(&self, is_dark: bool) -> egui::Stroke {
        egui::Stroke {
            color: self.color.into_color32(is_dark),
            width: self.width,
        }
    }

    pub fn neutral(is_dark: bool) -> egui::Stroke {
        egui::Stroke {
            color: DisplayedColor::Predefined(PredefinedColor::Neutral).into_color32(is_dark),
            width: 1.0,
        }
    }
}
