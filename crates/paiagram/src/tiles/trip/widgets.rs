use std::str::FromStr;

use egui::{DragValue, Response, Ui, Widget, emath};
use paiagram_core::time::TDuration;

use crate::font::TIMETABLTE_TEXT_STYLE;

/// A [`DragValue`] with a separate displayed value
pub struct SeparateDisplayDragValue<'a, T: emath::Numeric + ToString + FromStr> {
    /// The value being modified
    pub modified: &'a mut TDuration,
    /// The value being displayed
    pub displayed: T,
}

impl<'a, T: emath::Numeric + ToString + FromStr> Widget for SeparateDisplayDragValue<'a, T> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.style_mut().drag_value_text_style = TIMETABLTE_TEXT_STYLE.clone();
        let widget = DragValue::new(self.modified)
            .update_while_editing(false)
            .custom_formatter(|_, _| self.displayed.to_string())
            .custom_parser(|s| s.parse::<T>().ok().map(T::to_f64));
        ui.add(widget)
    }
}
